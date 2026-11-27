use crate::hass_mqtt::climate::mqtt_set_temperature;
use crate::hass_mqtt::command_routes::{
    device_id_segment, CommandTopic, ENERGY_COUNTER_ROUTE, FAN_COMMAND_ROUTE,
    FAN_SET_PERCENTAGE_ROUTE, HUMIDIFIER_SET_MODE_ROUTE, HUMIDIFIER_SET_TARGET_ROUTE,
    LIGHT_COMMAND_ROUTE, LIGHT_SEGMENT_COMMAND_ROUTE, MUSIC_SENSITIVITY_CLEAR_ROUTE,
    MUSIC_SENSITIVITY_COMMAND_ROUTE, NUMBER_COMMAND_ROUTE, ONECLICK_ROUTE, PURGE_CACHES_ROUTE,
    REQUEST_PLATFORM_DATA_ROUTE, SCENE_NEXT_ROUTE, SCENE_PREV_ROUTE, SET_MODE_SCENE_ROUTE,
    SET_MUSIC_PALETTE_ROUTE, SET_TEMPERATURE_ROUTE, SET_WORK_MODE_ROUTE, SWITCH_COMMAND_ROUTE,
};
use crate::hass_mqtt::enumerator::{enumerate_all_entites, enumerate_entities_for_device};
use crate::hass_mqtt::humidifier::{mqtt_device_set_work_mode, mqtt_humidifier_set_target};
use crate::hass_mqtt::instance::EntityList;
use crate::hass_mqtt::number::mqtt_number_command;
use crate::hass_mqtt::select::mqtt_set_mode_scene;
use crate::lan_api::DeviceColor;
use crate::opt_env_var;
use crate::platform_api::{from_json, DeviceType};
use crate::service::device::{Device as ServiceDevice, EnergyCounterState};
use crate::service::state::StateHandle;
use crate::temperature::TemperatureScale;
use anyhow::Context;
use async_channel::Receiver;
use mosquitto_rs::router::{MakeDispatcher, MqttRouter, Params, Payload, State};
use mosquitto_rs::{Client, Event, QoS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

const HASS_REGISTER_DELAY: tokio::time::Duration = tokio::time::Duration::from_secs(15);
const MUSIC_PALETTE_ENV_VAR: &str = "GOVEE_MUSIC_PALETTE";

/// Placeholder pattern for the Home Assistant birth/status topic.
/// Resolved at registration time with the configured discovery prefix.
const HASS_STATUS_ROUTE_PATTERN: &str = "{hass_disco_prefix}/status";

/// Resolve a table pattern to the topic template passed to `MqttRouter::route`.
fn resolve_mqtt_route_pattern(pattern: &str, disco_prefix: &str) -> String {
    if pattern == HASS_STATUS_ROUTE_PATTERN {
        format!("{disco_prefix}/status")
    } else {
        pattern.to_string()
    }
}

/// Single source of truth for MQTT route -> handler bindings.
///
/// Every expansion receives the same route and handler tokens. This keeps the
/// live subscriptions and the test inventory tied together, so a swapped
/// handler or a route hidden behind a runtime condition cannot pass unnoticed.
macro_rules! mqtt_routes {
    ($m:ident) => {
        $m! {
            HASS_STATUS_ROUTE_PATTERN => mqtt_homeassitant_status,
            LIGHT_COMMAND_ROUTE => mqtt_light_command,
            LIGHT_SEGMENT_COMMAND_ROUTE => mqtt_light_segment_command,
            SWITCH_COMMAND_ROUTE => mqtt_switch_command,
            ONECLICK_ROUTE => mqtt_oneclick,
            PURGE_CACHES_ROUTE => mqtt_purge_caches,
            REQUEST_PLATFORM_DATA_ROUTE => mqtt_request_platform_data,
            SCENE_NEXT_ROUTE => mqtt_scene_next,
            SCENE_PREV_ROUTE => mqtt_scene_prev,
            NUMBER_COMMAND_ROUTE => mqtt_number_command,
            HUMIDIFIER_SET_MODE_ROUTE => mqtt_device_set_work_mode,
            SET_WORK_MODE_ROUTE => mqtt_device_set_work_mode,
            MUSIC_SENSITIVITY_COMMAND_ROUTE =>
                crate::hass_mqtt::number::mqtt_music_sensitivity_command,
            MUSIC_SENSITIVITY_CLEAR_ROUTE =>
                crate::hass_mqtt::number::mqtt_clear_music_sensitivity_command,
            HUMIDIFIER_SET_TARGET_ROUTE => mqtt_humidifier_set_target,
            SET_TEMPERATURE_ROUTE => mqtt_set_temperature,
            SET_MODE_SCENE_ROUTE => mqtt_set_mode_scene,
            SET_MUSIC_PALETTE_ROUTE => mqtt_set_music_palette,
            FAN_SET_PERCENTAGE_ROUTE => crate::hass_mqtt::fan::mqtt_fan_set_percentage,
            FAN_COMMAND_ROUTE => crate::hass_mqtt::fan::mqtt_fan_command,
            ENERGY_COUNTER_ROUTE => mqtt_energy_counter,
        }
    };
}

#[cfg(test)]
macro_rules! emit_mqtt_route_pairings {
    ($($pattern:expr => $handler:path),+ $(,)?) => {
        /// `(pattern, handler_label)` for every registered MQTT route.
        /// Generated from the same table as the live `.route()` calls.
        pub(crate) const MQTT_ROUTE_PAIRINGS: &[(&str, &str)] = &[
            $(($pattern, stringify!($handler)),)+
        ];
    };
}

#[cfg(test)]
mqtt_routes!(emit_mqtt_route_pairings);

/// Production registers on a live `MqttRouter`; tests record patterns without
/// a broker. Both go through `bind_mqtt_command_routes`, so each route is
/// always present in the effective registration path.
enum MqttRouteBind<'a> {
    Live(&'a mut MqttRouter<StateHandle>),
    #[cfg(test)]
    Record(&'a mut Vec<String>),
}

async fn bind_one_mqtt_route<T, F>(
    bind: &mut MqttRouteBind<'_>,
    path: String,
    handler: F,
) -> anyhow::Result<()>
where
    F: MakeDispatcher<T, StateHandle>,
{
    match bind {
        MqttRouteBind::Live(router) => {
            router.route(path, handler).await?;
            Ok(())
        }
        #[cfg(test)]
        MqttRouteBind::Record(patterns) => {
            let _ = handler;
            patterns.push(path);
            Ok(())
        }
    }
}

/// Register every command topic. Feature flags belong in handlers, not here.
async fn bind_mqtt_command_routes(
    bind: &mut MqttRouteBind<'_>,
    disco_prefix: &str,
) -> anyhow::Result<()> {
    macro_rules! register_mqtt_routes {
        ($($pattern:expr => $handler:path),+ $(,)?) => {
            $(
                bind_one_mqtt_route(
                    bind,
                    resolve_mqtt_route_pattern($pattern, disco_prefix),
                    $handler,
                )
                .await?;
            )+
        };
    }
    mqtt_routes!(register_mqtt_routes);
    Ok(())
}

type MqttDispatchTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Reserve MQTT command order before the command futures are spawned.
///
/// The device control semaphore prevents overlapping hardware requests, but tasks
/// spawned from consecutive broker messages can reach that semaphore out of order.
/// A small queue per normalized device id preserves broker arrival order without
/// making a slow device block commands for every other device.
#[derive(Default)]
struct MqttDispatchQueues {
    by_device: HashMap<String, tokio::sync::mpsc::UnboundedSender<MqttDispatchTask>>,
}

impl MqttDispatchQueues {
    async fn enqueue_boxed(&mut self, topic: &str, state: &StateHandle, task: MqttDispatchTask) {
        let Some(device_key) = mqtt_device_dispatch_key(state, topic).await else {
            tokio::spawn(task);
            return;
        };

        let sender = self
            .by_device
            .entry(device_key.clone())
            .or_insert_with(|| start_mqtt_dispatch_queue(device_key.clone()))
            .clone();

        if let Err(error) = sender.send(task) {
            // The worker only exits when all senders are dropped, so this is a
            // shutdown/recovery path. Restart the lane rather than violating
            // device ordering with an independent fallback task.
            self.by_device.remove(&device_key);
            log::error!(
                "MQTT dispatch queue for device {device_key} closed unexpectedly; \
                 restarting it"
            );
            let replacement = start_mqtt_dispatch_queue(device_key.clone());
            match replacement.send(error.0) {
                Ok(()) => {
                    self.by_device.insert(device_key, replacement);
                }
                Err(_) => {
                    log::error!(
                        "Replacement MQTT dispatch queue for device {device_key} \
                         also closed; dropping the current message"
                    );
                }
            }
        }
    }
}

struct MqttDispatchRequest {
    topic: String,
    task: MqttDispatchTask,
}

fn start_mqtt_dispatch_scheduler(
    state: StateHandle,
) -> tokio::sync::mpsc::UnboundedSender<MqttDispatchRequest> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(run_mqtt_dispatch_scheduler(state, receiver));
    sender
}

async fn run_mqtt_dispatch_scheduler(
    state: StateHandle,
    mut receiver: tokio::sync::mpsc::UnboundedReceiver<MqttDispatchRequest>,
) {
    let mut queues = MqttDispatchQueues::default();
    while let Some(request) = receiver.recv().await {
        queues
            .enqueue_boxed(&request.topic, &state, request.task)
            .await;
    }
}

fn start_mqtt_dispatch_queue(
    device_key: String,
) -> tokio::sync::mpsc::UnboundedSender<MqttDispatchTask> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(run_mqtt_dispatch_queue(device_key, receiver));
    sender
}

async fn run_mqtt_dispatch_queue(
    device_key: String,
    mut receiver: tokio::sync::mpsc::UnboundedReceiver<MqttDispatchTask>,
) {
    while let Some(task) = receiver.recv().await {
        // Isolate panics to the current command so one bad handler cannot drop
        // every later command already queued for this device.
        if let Err(error) = tokio::spawn(task).await {
            log::error!("MQTT dispatch task for device {device_key} panicked: {error}");
        }
    }
}

fn mqtt_device_dispatch_label(topic: &str) -> Option<&str> {
    device_id_segment(topic)
}

async fn mqtt_device_dispatch_key(state: &StateHandle, topic: &str) -> Option<String> {
    let label = mqtt_device_dispatch_label(topic)?;
    let device = state.resolve_device(label).await?;

    let normalized: String = device
        .id
        .chars()
        .filter(|character| *character != ':' && *character != ' ')
        .map(|character| character.to_ascii_lowercase())
        .collect();

    (!normalized.is_empty()).then_some(normalized)
}

/// How frequently we re-assert that we are online by publishing to the
/// availability topic.
///
/// We publish availability without the retain flag, which means that only
/// the clients that are subscribed to the availability topic *at the moment
/// we publish* will ever see it.
///
/// hass subscribes to our availability topic as a side effect of setting up
/// each entity from its discovery config, so immediately after we (re)publish
/// our discovery configs there is a window in which hass has not yet
/// subscribed. `register_with_hass` sleeps for `10ms * number_of_entities`
/// to try to stay ahead of that, but on a busy hass startup -- a host reboot,
/// or a core update, where hass is creating thousands of entities across all
/// of its integrations -- setting up our entities can take minutes rather
/// than milliseconds.
///
/// When that happens the single "online" message is delivered to nobody, and
/// because nothing ever publishes it again, every one of our entities stays
/// `unavailable` in hass until the user manually restarts us, even though we
/// are connected to the broker and happily polling Govee.
/// See <https://github.com/wez/govee2mqtt/issues/572>,
/// <https://github.com/wez/govee2mqtt/issues/227> and
/// <https://github.com/wez/govee2mqtt/issues/110>.
///
/// Re-asserting availability on an interval bounds that window to the
/// interval, without needing to make any assumptions about how long hass
/// takes to subscribe.
const HASS_AVAILABILITY_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(60);

/// How long to wait after an unexpected disconnect for libmosquitto to
/// reconnect. Its loop thread exits on errors it treats as fatal and then
/// sends no further event, so we return an error, which ends the process
/// and lets the supervisor restart us. A broker outage longer than this
/// restarts us too, so it is long enough to ride out a broker host reboot.
const MQTT_RECONNECT_GRACE: tokio::time::Duration = tokio::time::Duration::from_secs(300);

/// Monotonic counter used to debounce `mqtt_homeassitant_status`: each
/// "online" status bumps this, and a pending re-registration checks it
/// after its delay to see if a newer status message has superseded it.
static HASS_STATUS_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(clap::Parser, Debug)]
pub struct HassArguments {
    /// The mqtt broker hostname or address.
    /// You may also set this via the GOVEE_MQTT_HOST environment variable.
    #[arg(long, global = true)]
    mqtt_host: Option<String>,

    /// The mqtt broker port
    /// You may also set this via the GOVEE_MQTT_PORT environment variable.
    /// If unspecified, uses 1883
    #[arg(long, global = true)]
    mqtt_port: Option<u16>,

    /// The username to authenticate against the broker
    /// You may also set this via the GOVEE_MQTT_USER environment variable.
    #[arg(long, global = true)]
    mqtt_username: Option<String>,

    /// The password to authenticate against the broker
    /// You may also set this via the GOVEE_MQTT_PASSWORD environment variable.
    #[arg(long, global = true)]
    mqtt_password: Option<String>,

    #[arg(long, global = true)]
    mqtt_bind_address: Option<String>,

    /// The MQTT discovery prefix configured in Home Assistant.
    /// You may also set this via the GOVEE_HASS_DISCOVERY_PREFIX environment
    /// variable. If unspecified, uses "homeassistant".
    #[arg(long, global = true)]
    hass_discovery_prefix: Option<String>,

    /// The temperature scale to use when showing temperature values as
    /// entities in home assistant. Can be either "C" or "F" for Celsius
    /// or Fahrenheit respectively.
    /// You may also set this via the GOVEE_TEMPERATURE_SCALE environment
    /// variable.
    #[arg(long, global = true)]
    temperature_scale: Option<String>,
}

impl HassArguments {
    pub fn opt_mqtt_host(&self) -> anyhow::Result<Option<String>> {
        match &self.mqtt_host {
            Some(h) => Ok(Some(h.to_string())),
            None => opt_env_var("GOVEE_MQTT_HOST"),
        }
    }

    pub fn mqtt_host(&self) -> anyhow::Result<String> {
        self.opt_mqtt_host()?.ok_or_else(|| {
            anyhow::anyhow!(
                "Please specify the mqtt broker either via the \
                --mqtt-host parameter or by setting $GOVEE_MQTT_HOST"
            )
        })
    }

    pub fn mqtt_port(&self) -> anyhow::Result<u16> {
        match self.mqtt_port {
            Some(p) => Ok(p),
            None => Ok(opt_env_var("GOVEE_MQTT_PORT")?.unwrap_or(1883)),
        }
    }

    pub fn mqtt_username(&self) -> anyhow::Result<Option<String>> {
        match self.mqtt_username.clone() {
            Some(u) => Ok(Some(u)),
            None => opt_env_var("GOVEE_MQTT_USER"),
        }
    }

    pub fn mqtt_password(&self) -> anyhow::Result<Option<String>> {
        match self.mqtt_password.clone() {
            Some(u) => Ok(Some(u)),
            None => opt_env_var("GOVEE_MQTT_PASSWORD"),
        }
    }

    pub fn hass_discovery_prefix(&self) -> anyhow::Result<String> {
        let prefix = match &self.hass_discovery_prefix {
            Some(p) => Some(p.to_string()),
            None => opt_env_var("GOVEE_HASS_DISCOVERY_PREFIX")?,
        };
        Ok(normalize_discovery_prefix(prefix.as_deref()))
    }

    pub fn temperature_scale(&self) -> anyhow::Result<TemperatureScale> {
        match &self.temperature_scale {
            Some(s) => Ok(s.parse()?),
            None => {
                Ok(opt_env_var("GOVEE_TEMPERATURE_SCALE")?.unwrap_or(TemperatureScale::Celsius))
            }
        }
    }
}

/// A blank value means the default, and stray slashes would produce topics
/// that Home Assistant does not watch.
fn normalize_discovery_prefix(prefix: Option<&str>) -> String {
    let prefix = prefix.unwrap_or_default().trim().trim_matches('/');
    if prefix.is_empty() {
        "homeassistant".to_string()
    } else {
        prefix.to_string()
    }
}

#[derive(Clone)]
pub struct HassClient {
    client: Client,
}

impl HassClient {
    async fn register_with_hass(&self, state: &StateHandle) -> anyhow::Result<()> {
        let entities = enumerate_all_entites(state).await?;

        // Register the configs
        log::trace!("register_with_hass: register entities");
        entities.publish_config(state, self).await?;

        // Allow hass extra time to register the entities before
        // we mark them as available
        let delay = tokio::time::Duration::from_millis((10 * entities.len()) as u64);
        log::info!(
            "Wait {delay:?} for hass to settle on {} entity configs",
            entities.len()
        );
        tokio::time::sleep(delay).await;

        // Mark as available
        log::trace!("register_with_hass: mark as online");
        self.publish(availability_topic(), "online")
            .await
            .context("online -> availability_topic")?;

        for device in state.devices().await {
            self.publish_device_availability(&device).await?;
        }

        // report initial state
        log::trace!("register_with_hass: reporting state");
        entities.notify_state(self).await.context("notify_state")?;

        log::trace!("register_with_hass: done");

        Ok(())
    }

    /// Retained, so that Home Assistant gets it whenever it subscribes. A
    /// stopped bridge is still covered by the bridge availability topic.
    async fn publish_device_availability(&self, device: &ServiceDevice) -> anyhow::Result<()> {
        if !offline_unavailable_enabled() {
            return Ok(());
        }
        let topic = device_availability_topic_for_id(&topic_safe_id(device));
        let online = device.device_state().and_then(|state| state.online);
        let payload = device_availability_payload(online);
        log::trace!("{topic} -> {payload}");
        self.client
            .publish(topic, payload, QoS::AtMostOnce, true)
            .await?;
        Ok(())
    }

    pub async fn publish<T: AsRef<str> + std::fmt::Display, P: AsRef<[u8]> + std::fmt::Display>(
        &self,
        topic: T,
        payload: P,
    ) -> anyhow::Result<()> {
        log::trace!("{topic} -> {payload}");
        self.client
            .publish(topic, payload, QoS::AtMostOnce, false)
            .await?;
        Ok(())
    }

    pub async fn publish_obj<T: AsRef<str> + std::fmt::Display, P: Serialize>(
        &self,
        topic: T,
        payload: P,
    ) -> anyhow::Result<()> {
        let payload = serde_json::to_string(&payload)?;
        log::trace!("{topic} -> {payload}");
        self.client
            .publish(topic, payload, QoS::AtMostOnce, false)
            .await?;
        Ok(())
    }

    pub async fn publish_obj_retained<T: AsRef<str> + std::fmt::Display, P: Serialize>(
        &self,
        topic: T,
        payload: P,
    ) -> anyhow::Result<()> {
        let payload = serde_json::to_string(&payload)?;
        log::trace!("{topic} -> {payload} (retained)");
        self.client
            .publish(topic, payload, QoS::AtMostOnce, true)
            .await?;
        Ok(())
    }

    /// Publishes at QoS 1, for messages the broker must not drop. With
    /// retain, it keeps them for when we subscribe again after a restart.
    pub async fn publish_reliably<
        T: AsRef<str> + std::fmt::Display,
        P: AsRef<[u8]> + std::fmt::Display,
    >(
        &self,
        topic: T,
        payload: P,
        retain: bool,
    ) -> anyhow::Result<()> {
        log::trace!("{topic} -> {payload}");
        self.client
            .publish(topic, payload, QoS::AtLeastOnce, retain)
            .await?;
        Ok(())
    }

    pub async fn advise_hass_of_light_state(
        &self,
        device: &ServiceDevice,
        state: &StateHandle,
    ) -> anyhow::Result<()> {
        self.publish_device_availability(device).await?;

        let mut entities = EntityList::new();
        enumerate_entities_for_device(device, state, &mut entities).await?;
        entities.notify_state(self).await?;

        Ok(())
    }
}

pub fn topic_safe_string(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        if c == ':' || c == ' ' || c == '\\' || c == '/' || c == '\'' || c == '"' {
            result.push('_');
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    result
}

pub fn topic_safe_id(device: &ServiceDevice) -> String {
    let mut id = device.id.to_string();
    id.retain(|c| c != ':');
    id.retain(|c| c != ' ');
    id
}

pub fn switch_instance_state_topic(device: &ServiceDevice, instance: &str) -> String {
    format!(
        "gv2mqtt/switch/{id}/{instance}/state",
        id = topic_safe_id(device)
    )
}

/// Holds the retained EnergyCounterState of a plug. Home Assistant
/// doesn't subscribe to it, see mqtt_energy_counter
pub fn energy_counter_topic(device: &ServiceDevice) -> String {
    format!("gv2mqtt/energy/{id}/counter", id = topic_safe_id(device))
}

pub fn light_state_topic(device: &ServiceDevice) -> String {
    format!("gv2mqtt/light/{id}/state", id = topic_safe_id(device))
}

/// The topic on which we publish event states for a device's event
/// capability (eg: ice-maker-full, water-empty). The matching HASS event
/// entity subscribes to this topic.
pub fn event_state_topic(device: &ServiceDevice, instance: &str) -> String {
    format!(
        "gv2mqtt/event/{id}/{inst}/state",
        id = topic_safe_id(device),
        inst = topic_safe_string(instance)
    )
}

pub fn light_segment_state_topic(device: &ServiceDevice, segment: u32) -> String {
    format!(
        "gv2mqtt/light/{id}/state/{segment}",
        id = topic_safe_id(device)
    )
}

/// All entities use the same topic so that we can mark unavailable
/// via last-will
pub fn availability_topic() -> String {
    "gv2mqtt/availability".to_string()
}

pub fn device_availability_topic_for_id(id: &str) -> String {
    format!("gv2mqtt/{id}/availability")
}

/// Whether devices that Govee reports as offline become unavailable in Home
/// Assistant. Off by default, because Govee's report is not always right.
pub fn offline_unavailable_enabled() -> bool {
    std::env::var("GOVEE_OFFLINE_UNAVAILABLE")
        .map(|v| v == "true")
        .unwrap_or(false)
}

/// Only an explicit "offline" counts; LAN and IoT updates do not say.
fn device_availability_payload(online: Option<bool>) -> &'static str {
    if online == Some(false) {
        "offline"
    } else {
        "online"
    }
}

pub fn oneclick_topic() -> anyhow::Result<CommandTopic> {
    crate::hass_mqtt::command_routes::instantiate_route(ONECLICK_ROUTE, &[])
}

pub fn purge_cache_topic() -> anyhow::Result<CommandTopic> {
    crate::hass_mqtt::command_routes::instantiate_route(PURGE_CACHES_ROUTE, &[])
}

#[derive(Deserialize)]
pub struct IdParameter {
    pub id: String,
}

#[derive(Deserialize)]
struct MusicPaletteCommand {
    style: String,
    colors: Vec<String>,
    #[serde(default = "default_music_sensitivity")]
    sensitivity: u8,
}

fn default_music_sensitivity() -> u8 {
    crate::platform_api::DEFAULT_MUSIC_SENSITIVITY
}

fn music_palette_enabled(value: Option<&str>) -> bool {
    value == Some("true")
}

/// `gv2mqtt/<id>/set-music-palette` with a JSON payload like
/// `{"style": "Rhythm", "colors": ["#0000ff", "#ff0000"], "sensitivity": 99}`.
///
/// Programs music mode with a caller-chosen palette over LAN, for the SKUs
/// mapped in `src/music.rs`. Opt-in: the topic only acts when
/// `GOVEE_MUSIC_PALETTE=true` is set. Documented in docs/MUSIC_MODE.md.
async fn mqtt_set_music_palette(
    Payload(payload): Payload<String>,
    Params(IdParameter { id }): Params<IdParameter>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    let configured = std::env::var(MUSIC_PALETTE_ENV_VAR).ok();
    let enabled = music_palette_enabled(configured.as_deref());
    anyhow::ensure!(
        enabled,
        "set-music-palette is opt-in: set GOVEE_MUSIC_PALETTE=true to enable it"
    );

    let command: MusicPaletteCommand = serde_json::from_str(&payload)
        .with_context(|| format!("parsing set-music-palette payload {payload:?}"))?;

    let device = state.resolve_device_for_control(&id).await?;

    let profile = crate::music::music_profile(&device.sku, &command.style).ok_or_else(|| {
        match crate::music::music_styles(&device.sku) {
            Some(styles) => anyhow::anyhow!(
                "style {:?} is not mapped for {}; mapped styles: {}",
                command.style,
                device.sku,
                styles.join(", ")
            ),
            None => anyhow::anyhow!(
                "{} has no music profile table entry yet; \
                 docs/MUSIC_MODE.md describes how to map a new SKU",
                device.sku
            ),
        }
    })?;

    let colors = command
        .colors
        .iter()
        .map(|color| crate::music::parse_hex_color(color))
        .collect::<anyhow::Result<Vec<_>>>()?;

    state
        .device_set_music_palette(
            &device,
            &crate::ble::SetMusicPalette {
                profile,
                colors,
                sensitivity: command.sensitivity,
            },
        )
        .await
}

/// Someone pressed the "Scene Next" button
async fn mqtt_scene_next(
    Params(IdParameter { id }): Params<IdParameter>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    scene_cycle(&state, &id, 1).await
}

/// Someone pressed the "Scene Previous" button
async fn mqtt_scene_prev(
    Params(IdParameter { id }): Params<IdParameter>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    scene_cycle(&state, &id, -1).await
}

/// Computes the target scene index for cycling.
/// Returns the index into `scenes` to activate.
fn compute_scene_cycle_index(
    scenes: &[String],
    current_name: Option<&str>,
    direction: i32,
) -> usize {
    let total = scenes.len() as i32;
    match current_name.and_then(|name| scenes.iter().position(|n| n.eq_ignore_ascii_case(name))) {
        Some(idx) => ((idx as i32 + direction).rem_euclid(total)) as usize,
        None => {
            if direction > 0 {
                0
            } else {
                (total - 1) as usize
            }
        }
    }
}

/// Shared logic for scene next/prev cycling
async fn scene_cycle(state: &StateHandle, id: &str, direction: i32) -> anyhow::Result<()> {
    // Acquire Coordinator first to prevent races with concurrent scene changes
    let coord = state.resolve_device_for_control(id).await?;

    let catalog = state.device_list_scenes_categorized(&coord).await?;
    let flat: Vec<String> = catalog
        .into_iter()
        .flat_map(|cat| cat.scenes.into_iter().map(|s| s.name))
        .collect();

    if flat.is_empty() {
        anyhow::bail!("No scenes available for device {id}");
    }

    let current_name = coord.active_scene_name().map(|s| s.to_string());
    let new_idx = compute_scene_cycle_index(&flat, current_name.as_deref(), direction);

    let target_scene = &flat[new_idx];

    log::info!(
        "Scene cycle {}: {} -> {} (index {} of {})",
        if direction > 0 { "next" } else { "prev" },
        current_name.as_deref().unwrap_or("None"),
        target_scene,
        new_idx,
        flat.len()
    );

    state.device_set_scene(&coord, target_scene).await?;

    Ok(())
}

/// Someone clicked the "Request Platform API State" button
async fn mqtt_request_platform_data(
    Params(IdParameter { id }): Params<IdParameter>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    let device = state.resolve_device_read_only(&id).await?;
    log::info!("Request Platform API State for {device}");
    if !state.poll_platform_api(&device).await? {
        log::warn!("Unable to poll platform API for {device}");
    }
    Ok(())
}

#[derive(Deserialize, Debug, Clone)]
struct HassLightCommand {
    state: String,
    /// Kelvin, because the light config sets `color_temp_kelvin`
    color_temp: Option<u32>,
    color: Option<DeviceColor>,
    effect: Option<String>,
    brightness: Option<u8>,
    /// Home Assistant's `transition` attribute: the fade duration in
    /// seconds. See `ramp_brightness` for how it is honored.
    transition: Option<f64>,
}

/// The maximum accepted transition duration, in seconds. HA itself allows
/// arbitrarily large values; we clamp so that a stray number cannot hold
/// the device coordinator open essentially forever.
const TRANSITION_MAX_SECONDS: f64 = 3600.0;
/// Govee's LAN API guidance says consecutive commands should be at least
/// a second apart; we never send ramp steps any faster than this.
const TRANSITION_MIN_INTERVAL: Duration = Duration::from_secs(1);
/// For transitions long enough to allow it, aim for this many steps...
const TRANSITION_TARGET_STEPS: usize = 10;
/// ...and never exceed this many, regardless of how long the transition is.
const TRANSITION_MAX_STEPS: usize = 20;

/// Clamp a caller supplied transition duration to a sane range.
pub fn clamp_transition_seconds(seconds: f64) -> f64 {
    if seconds.is_finite() {
        seconds.clamp(0.0, TRANSITION_MAX_SECONDS)
    } else {
        0.0
    }
}

/// Plan a stepped brightness ramp of the given `duration`.
///
/// Returns `(num_steps, interval_between_steps)`. Longer transitions get
/// `TRANSITION_TARGET_STEPS..=TRANSITION_MAX_STEPS` evenly spaced steps;
/// the interval never drops below `TRANSITION_MIN_INTERVAL`, so shorter
/// transitions get fewer steps, degrading to a single final set when the
/// duration can't fit even one step at that rate.
pub fn transition_plan(duration: Duration) -> (usize, Duration) {
    let secs = duration.as_secs_f64().max(0.0);
    if secs <= 0.0 {
        return (1, Duration::ZERO);
    }

    let desired = (secs.round() as usize).clamp(TRANSITION_TARGET_STEPS, TRANSITION_MAX_STEPS);
    let mut interval = duration / (desired as u32);
    if interval < TRANSITION_MIN_INTERVAL {
        interval = TRANSITION_MIN_INTERVAL;
    }

    let steps = ((secs / interval.as_secs_f64()).floor() as usize).max(1);
    (steps, interval)
}

/// Whether a transition is long enough to fade. A shorter one has room for a
/// single step, which would only delay the change. Home Assistant can send
/// fractions of a second with an ordinary turn-on.
fn is_fade(duration: Duration) -> bool {
    transition_plan(duration).0 > 1
}

/// Compute the brightness values to send for a ramp from `from` to `to`
/// in `steps` linearly spaced increments.
///
/// The result always ends exactly at `to`, never repeats a value twice in
/// a row (rounding can collapse adjacent steps for short ranges), and is
/// monotonic in the direction of the ramp.
pub fn ramp_values(from: u8, to: u8, steps: usize) -> Vec<u8> {
    let steps = steps.max(1);
    let mut values = Vec::with_capacity(steps);

    for i in 1..=steps {
        let v = from as f64 + (to as f64 - from as f64) * (i as f64 / steps as f64);
        let v = v.round().clamp(0.0, 255.0) as u8;
        if values.last() != Some(&v) {
            values.push(v);
        }
    }
    if values.last() != Some(&to) {
        values.push(to);
    }
    values
}

/// Honor HA's `transition` attribute with a series of brightness commands,
/// because the LAN protocol has no fade primitive. Colour and colour
/// temperature are applied at the start of the ramp.
///
/// Runs in its own task and takes the device permit only for each step, so a
/// long fade never delays other commands. Any later control request for the
/// device ends it.
async fn ramp_brightness(
    state: StateHandle,
    device: ServiceDevice,
    generation: u64,
    from: u8,
    to: u8,
    duration: Duration,
) -> anyhow::Result<()> {
    let (steps, interval) = transition_plan(duration);
    let values = ramp_values(from, to, steps);
    log::info!(
        "Transitioning {device} brightness {from} -> {to} over {duration:?} in {steps} steps"
    );

    for value in values {
        if interval > Duration::ZERO {
            tokio::time::sleep(interval).await;
        }
        let _permit = state.acquire_device_update_permit(&device).await?;
        if state.control_generation(&device.id).await != generation {
            log::info!("Transition for {device} superseded by a newer command");
            return Ok(());
        }
        let current = state.device_by_id(&device.id).await;
        state
            .device_set_brightness(current.as_ref().unwrap_or(&device), value)
            .await?;
    }
    Ok(())
}

/// HASS is sending a command to a light
async fn mqtt_light_command(
    Payload(payload): Payload<String>,
    Params(IdParameter { id }): Params<IdParameter>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    let device = state.resolve_device_for_control(&id).await?;

    let command: HassLightCommand = serde_json::from_str(&payload)?;
    log::info!("Command for {device}: {payload}");

    let is_light = device.device_type() == DeviceType::Light;

    if command.state == "OFF" {
        if is_light {
            state
                .device_light_power_on(&device, false)
                .await
                .context("mqtt_light_command: state.device_power_on")?;
        } else {
            state
                .device_set_brightness(&device, 0)
                .await
                .context("mqtt_light_command: state.device_set_brightness")?;
        }
    } else {
        let transition = command
            .transition
            .map(clamp_transition_seconds)
            .filter(|seconds| *seconds > 0.0)
            .map(Duration::from_secs_f64);

        // Stepped commands are only reasonable over the LAN; the cloud paths
        // are rate limited. A scene change opts out too.
        let ramp = match (transition, command.brightness, &command.effect) {
            (Some(duration), Some(target), None)
                if is_fade(duration) && device.lan_device.is_some() =>
            {
                Some((duration, target))
            }
            _ => None,
        };

        if let Some((duration, target)) = ramp {
            let from = device
                .device_state()
                .filter(|s| s.on)
                .map(|s| s.brightness)
                .unwrap_or(1);
            // Start dim, so that a colour change cannot flash a light that
            // was off at its previous brightness.
            state
                .device_set_brightness(&device, from)
                .await
                .context("mqtt_light_command: state.device_set_brightness")?;
            if let Some(color) = &command.color {
                state
                    .device_set_color_rgb(&device, color.r, color.g, color.b)
                    .await
                    .context("mqtt_light_command: state.device_set_color_rgb")?;
            }
            if let Some(color_temp) = command.color_temp {
                state
                    .device_set_color_temperature(&device, color_temp)
                    .await
                    .context("mqtt_light_command: state.device_set_color_temperature")?;
            }

            let ramp_device = (*device).clone();
            let generation = device.generation();
            let state = state.clone();
            tokio::spawn(async move {
                let result =
                    ramp_brightness(state, ramp_device, generation, from, target, duration).await;
                if let Err(err) = result {
                    log::error!("While transitioning brightness: {err:#}");
                }
            });
            return Ok(());
        }
        if transition.is_some() {
            log::debug!(
                "Ignoring transition for {device}: it needs LAN control, \
                 a brightness, no effect, and at least 2 seconds"
            );
        }

        let mut power_on = true;

        if let Some(brightness) = command.brightness {
            state
                .device_set_brightness(&device, brightness)
                .await
                .context("mqtt_light_command: state.device_set_brightness")?;
            power_on = false;
        }

        if let Some(effect) = &command.effect {
            state
                .device_set_scene_with_music_color(&device, effect, command.color)
                .await
                .context("mqtt_light_command: state.device_set_scene")?;
            // A Music effect can carry one RGB colour in the same Platform API
            // struct; its presence disables autoColor. Ordinary scenes still
            // ignore colour and temperature, as before. Brightness is okay.
            return Ok(());
        }

        if let Some(color) = &command.color {
            state
                .device_set_color_rgb(&device, color.r, color.g, color.b)
                .await
                .context("mqtt_light_command: state.device_set_color_rgb")?;
            power_on = false;
        }
        if let Some(color_temp) = command.color_temp {
            state
                .device_set_color_temperature(&device, color_temp)
                .await
                .context("mqtt_light_command: state.device_set_color_temperature")?;
            power_on = false;
        }

        if power_on {
            if is_light {
                state
                    .device_light_power_on(&device, true)
                    .await
                    .context("mqtt_light_command: state.device_power_on")?;
            } else if command.brightness.is_none() {
                // The device is not primarily a light and we don't have
                // a guaranteed way to power it on without setting the
                // brightness to something, and we know we didn't set
                // the brightness just now, so let's turn it on 100%
                state
                    .device_set_brightness(&device, 100)
                    .await
                    .context("mqtt_light_command: state.device_set_brightness")?;
            }
        }
    }

    Ok(())
}

#[derive(Deserialize)]
struct IdAndSeg {
    id: String,
    segment: String,
}

async fn mqtt_light_segment_command(
    Payload(payload): Payload<String>,
    Params(IdAndSeg { id, segment }): Params<IdAndSeg>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    let device = state.resolve_device_for_control_without_poll(&id).await?;
    let segment: u32 = segment.parse()?;

    let command: HassLightCommand = from_json(&payload)?;
    log::info!("Command for {device} segment {segment}: {payload}");

    if command.state == "OFF" && device.supports_local_segments() {
        // Black turns one panel off without affecting the others. Not sent to a
        // light that is already off: the frame could wake it, for instance when
        // a whole area is switched off.
        if device.device_state().map(|s| s.on).unwrap_or(false) {
            state
                .device_set_segment_rgb(&device, segment, 0, 0, 0)
                .await?;
        }
        return Ok(());
    }

    if let Some(brightness) = command.brightness {
        state
            .device_set_segment_brightness(&device, segment, brightness)
            .await?;
    }
    // OFF does nothing for Platform API devices. We used to set brightness to
    // zero, but some devices don't have a 0, and setting it powers up the rest
    // of the device, so turning off a whole area would turn it back on.
    if let Some(color) = &command.color {
        state
            .device_set_segment_rgb(&device, segment, color.r, color.g, color.b)
            .await?;
        // A solid per-segment color ends any scene the bridge had applied.
        state
            .device_mut(&device.sku, &device.id)
            .await
            .set_active_scene(None);
    }

    Ok(())
}

async fn mqtt_purge_caches(State(state): State<StateHandle>) -> anyhow::Result<()> {
    log::info!("mqtt_purge_caches");
    crate::cache::purge_cache()?;
    state.clear_scene_catalogs().await;
    state
        .get_hass_client()
        .await
        .context("mqtt client is not ready")?
        .register_with_hass(&state)
        .await
        .context("register_with_hass")
}

async fn mqtt_oneclick(
    Payload(name): Payload<String>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    log::info!("mqtt_oneclick: {name}");

    let undoc = state.get_undoc_client().await.ok_or_else(|| {
        anyhow::anyhow!("Govee cloud API unavailable: is your email/password configured?")
    })?;
    let items = undoc.parse_one_clicks().await?;
    let item = items
        .iter()
        .find(|item| item.name == name)
        .ok_or_else(|| anyhow::anyhow!("didn't find item {name}"))?;

    let iot = state
        .get_iot_client()
        .await
        .ok_or_else(|| anyhow::anyhow!("AWS IoT client is not available"))?;

    iot.activate_one_click(item).await
}

#[derive(Deserialize)]
struct IdAndInst {
    id: String,
    instance: String,
}

/// Govee reports no state for most toggles, so assume a command took effect. A
/// real value from a later poll or push still overrides it.
async fn publish_assumed_toggle_state(
    state: &StateHandle,
    device: &ServiceDevice,
    instance: &str,
    on: bool,
) -> anyhow::Result<()> {
    if let Some(hass) = state.get_hass_client().await {
        hass.publish(
            switch_instance_state_topic(device, instance),
            if on { "ON" } else { "OFF" },
        )
        .await?;
    }
    Ok(())
}

async fn mqtt_switch_command(
    Payload(command): Payload<String>,
    Params(IdAndInst { id, instance }): Params<IdAndInst>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    log::info!("{instance} for {id}: {command}");
    let device = state.resolve_device_for_control(&id).await?;

    let on = match command.as_str() {
        "ON" | "on" => true,
        "OFF" | "off" => false,
        _ => anyhow::bail!("invalid {command} for {id}"),
    };

    if instance == "powerSwitch" {
        state.device_power_on(&device, on).await?;
    } else if matches!(
        instance.as_str(),
        "mainLightToggle" | "backgroundLightToggle"
    ) {
        state
            .device_set_element_toggle(&device, &instance, on)
            .await?;
        publish_assumed_toggle_state(&state, &device, &instance, on).await?;
    } else if instance == "videoMode" {
        // Synthetic toggle (see CapabilitySwitch::video_mode); not a real
        // Platform-API capability, so it must not fall through to set_toggle_state.
        state.device_set_video_mode(&device, on).await?;
        publish_assumed_toggle_state(&state, &device, &instance, on).await?;
    } else if let Some(client) = state.get_platform_client().await {
        if let Some(http_dev) = &device.http_device_info {
            client.set_toggle_state(http_dev, &instance, on).await?;
            publish_assumed_toggle_state(&state, &device, &instance, on).await?;
        } else {
            anyhow::bail!("No platform state available to set {id} {instance} to {on}");
        }
    } else {
        anyhow::bail!("Unsupported command '{command}' for {id} {instance}");
    }

    Ok(())
}

/// HASS is advising us that its status has changed
///
/// HA's birth/will topic (`<discovery_prefix>/status`) carries both the
/// "online" birth message on startup and the "offline" will/shutdown
/// message. Historically this handler reacted to *both*, which meant a
/// single HA restart could fire two overlapping, unsynchronized calls to
/// this handler: one for "offline" (started while HA is still shutting
/// down or not yet back up) and one for "online" (the one that actually
/// matters). Each call independently sleeps for `HASS_REGISTER_DELAY`
/// before calling `register_with_hass`, and nothing prevented two of
/// these delayed calls from being in flight at once and racing each
/// other's publishes. See https://github.com/wez/govee2mqtt/issues/572.
///
/// Only "online" should ever trigger re-registration, and if multiple
/// "online" messages arrive in quick succession (HA's MQTT connection can
/// flap a few times while other integrations are still loading at boot),
/// only the *last* one should actually run: the generation counter below
/// debounces that by having each pending call check, after its delay,
/// whether a newer status message has superseded it.
async fn mqtt_homeassitant_status(
    Payload(status): Payload<String>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    if status != "online" {
        log::info!("Home Assistant status changed: {status}; ignoring (only 'online' triggers re-registration)");
        return Ok(());
    }

    let my_generation = HASS_STATUS_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    log::info!("Home Assistant status changed: {status}, waiting {HASS_REGISTER_DELAY:?} before re-registering entities");
    tokio::time::sleep(HASS_REGISTER_DELAY).await;

    if HASS_STATUS_GENERATION.load(Ordering::SeqCst) != my_generation {
        log::info!(
            "A newer Home Assistant online message superseded this one; skipping stale re-registration"
        );
        return Ok(());
    }

    let client = state
        .get_hass_client()
        .await
        .context("hass client is not available")?;

    client.register_with_hass(&state).await?;

    Ok(())
}

/// Restores the energy counter state that a previous run saved in the
/// retained counter topic of a plug, see EnergyCounterState. Until that's
/// complete, EnergyMonitoringSensor publishes an empty message to the
/// topic along with its state. MQTT keeps the order of messages within a topic, so once we
/// receive it, any retained state has arrived before it.
async fn mqtt_energy_counter(
    Payload(payload): Payload<String>,
    Params(IdParameter { id }): Params<IdParameter>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    let Some(device) = state.resolve_device(&id).await else {
        return Ok(());
    };
    let mut device = state.device_mut(&device.sku, &device.id).await;
    if payload.is_empty() {
        device.energy_counter_restored = true;
    } else if let Ok(saved) = serde_json::from_str::<EnergyCounterState>(&payload) {
        device.restore_energy_counter(saved);
    }
    Ok(())
}

async fn run_mqtt_loop(
    state: StateHandle,
    subscriber: Receiver<Event>,
    client: Client,
) -> anyhow::Result<()> {
    // Give LAN disco a chance to get current state before
    // we register with hass
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    async fn rebuild_router(
        client: &Client,
        state: &StateHandle,
    ) -> anyhow::Result<Arc<MqttRouter<StateHandle>>> {
        let disco_prefix = state.get_hass_disco_prefix().await;
        let mut router: MqttRouter<StateHandle> = MqttRouter::new(client.clone());
        bind_mqtt_command_routes(&mut MqttRouteBind::Live(&mut router), &disco_prefix).await?;
        // The router subscribes at QoS 0, which would downgrade the delivery
        // of the counter state, see HassClient::publish_reliably
        client
            .subscribe("gv2mqtt/energy/+/counter", QoS::AtLeastOnce)
            .await?;

        tokio::time::sleep(HASS_REGISTER_DELAY).await;
        state
            .get_hass_client()
            .await
            .context("mqtt client is not ready")?
            .register_with_hass(state)
            .await
            .context("register_with_hass")?;

        Ok(Arc::new(router))
    }

    let mut router = rebuild_router(&client, &state).await?;
    let mut need_rebuild = false;
    // Canonical device lookup needs the async state mutex. Keep it in a FIFO
    // scheduler task so the broker event loop can continue draining reconnect
    // events while a state mutation temporarily owns that mutex.
    let dispatch_scheduler = start_mqtt_dispatch_scheduler(state.clone());
    let mut disconnected_since: Option<tokio::time::Instant> = None;

    while let Some(event) = next_mqtt_event(&subscriber, disconnected_since).await? {
        match event {
            Event::Message(msg) => {
                let router = router.clone();
                let dispatch_state = state.clone();
                let topic = msg.topic.clone();
                dispatch_scheduler
                    .send(MqttDispatchRequest {
                        topic,
                        task: Box::pin(async move {
                            if let Err(err) = router.dispatch(msg.clone(), dispatch_state).await {
                                log::error!("While dispatching {msg:?}: {err:#}");
                            }
                        }),
                    })
                    .map_err(|_| anyhow::anyhow!("MQTT dispatch scheduler stopped"))?;
            }
            Event::Disconnected(reason) => {
                log::warn!("MQTT disconnected with reason={reason}");
                need_rebuild = true;
                disconnected_since.get_or_insert_with(tokio::time::Instant::now);
            }
            Event::Connected(status) => {
                log::info!("MQTT connected with status={status}");
                disconnected_since = None;
                if need_rebuild {
                    router = rebuild_router(&client, &state).await?;
                }
            }
        }
    }

    log::info!("subscriber.recv loop terminated");

    Ok(())
}

/// Waits for the next client event. While disconnected, fails if none
/// arrives within `MQTT_RECONNECT_GRACE` of the first disconnect.
/// Returns `None` when the event stream has ended.
async fn next_mqtt_event(
    subscriber: &Receiver<Event>,
    disconnected_since: Option<tokio::time::Instant>,
) -> anyhow::Result<Option<Event>> {
    let event = match disconnected_since {
        Some(since) => {
            match tokio::time::timeout_at(since + MQTT_RECONNECT_GRACE, subscriber.recv()).await {
                Ok(event) => event,
                Err(_) => anyhow::bail!(
                    "MQTT connection was not restored within {MQTT_RECONNECT_GRACE:?}"
                ),
            }
        }
        None => subscriber.recv().await,
    };
    Ok(event.ok())
}

pub async fn spawn_hass_integration(
    state: StateHandle,
    args: &HassArguments,
) -> anyhow::Result<()> {
    let client = Client::with_id(
        // Some brokers reject a "/" in a client id
        &format!("govee2mqtt-{}", uuid::Uuid::new_v4().simple()),
        true,
    )?;

    state.set_temperature_scale(args.temperature_scale()?).await;

    let mqtt_host = args.mqtt_host()?;
    let mqtt_username = args.mqtt_username()?;
    let mqtt_password = args.mqtt_password()?;
    let mqtt_port = args.mqtt_port()?;

    client.set_last_will(availability_topic(), "offline", QoS::AtMostOnce, false)?;

    if mqtt_username.is_some() != mqtt_password.is_some() {
        log::error!(
            "MQTT username and password either both need to be set, or both need to be unset"
        );
    }
    client.set_username_and_password(mqtt_username.as_deref(), mqtt_password.as_deref())?;

    let mut connected = false;
    for _ in 0..30 {
        log::info!("Attempting connection to mqtt broker {mqtt_host}:{mqtt_port}...");
        match tokio::time::timeout(
            Duration::from_secs(125),
            client.connect(
                &mqtt_host,
                mqtt_port.into(),
                Duration::from_secs(120),
                args.mqtt_bind_address.as_deref(),
            ),
        )
        .await
        {
            Ok(Ok(status)) => {
                log::info!("Connected to mqtt broker {mqtt_host}:{mqtt_port}, status={status}");
                connected = true;
                break;
            }
            Ok(Err(err)) => {
                log::error!("Failed to connect to mqtt broker {mqtt_host}:{mqtt_port}: {err:#}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(_) => {
                log::error!("Failed to connect to mqtt broker {mqtt_host}:{mqtt_port}: Tokio timeout reached");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    anyhow::ensure!(
        connected,
        "Failed to connect to mqtt broker after several attempts"
    );

    let subscriber = client
        .subscriber()
        .context("the mqtt subscriber was already taken")?;

    state
        .set_hass_client(HassClient {
            client: client.clone(),
        })
        .await;

    let disco_prefix = args.hass_discovery_prefix()?;
    state.set_hass_disco_prefix(disco_prefix).await;

    // Periodically re-assert our availability, so that a hass instance which
    // subscribes to the availability topic after we published "online" still
    // learns that we are alive. See HASS_AVAILABILITY_INTERVAL.
    {
        let hass_client = HassClient {
            client: client.clone(),
        };
        tokio::spawn(async move {
            let mut warned = false;
            loop {
                // Sleep first: register_with_hass publishes availability as
                // part of the initial registration, so there is nothing for
                // us to do until that has had a chance to run.
                tokio::time::sleep(HASS_AVAILABILITY_INTERVAL).await;

                match hass_client.publish(availability_topic(), "online").await {
                    Ok(()) => warned = false,
                    Err(err) if !warned => {
                        // Expected while we are disconnected from the broker;
                        // only report the transition into failure so that a
                        // broker outage doesn't spam the log every interval.
                        log::warn!("while re-asserting availability: {err:#}");
                        warned = true;
                    }
                    Err(err) => {
                        log::debug!("while re-asserting availability: {err:#}");
                    }
                }
            }
        });
    }

    tokio::spawn(async move {
        let res = run_mqtt_loop(state, subscriber, client).await;
        if let Err(err) = res {
            log::error!("run_mqtt_loop: {err:#}");
            log::error!("FATAL: hass integration will not function.");
            log::error!("Pausing for 30 seconds before terminating.");
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            std::process::exit(1);
        } else {
            log::error!(
                "run_mqtt_loop exited unexpectedly. Terminating so HA can restart the addon."
            );
            std::process::exit(1);
        }
    });

    Ok(())
}

pub fn camel_case_to_space_separated(camel: &str) -> String {
    let mut chars = camel.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut result = first.to_ascii_uppercase().to_string();
    for c in chars {
        if c.is_uppercase() {
            result.push(' ');
        }
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hass_mqtt::instance::EntityInstance;
    use crate::platform_api::{
        DeviceCapability, DeviceCapabilityKind, DeviceParameters, EnumOption,
    };

    #[test]
    fn test_camel_case_to_space_separated() {
        assert_eq!(camel_case_to_space_separated("powerSwitch"), "Power Switch");
        assert_eq!(
            camel_case_to_space_separated("oscillationToggle"),
            "Oscillation Toggle"
        );
    }

    #[test]
    fn test_camel_case_chinese_no_panic() {
        assert_eq!(
            camel_case_to_space_separated("用于三灯头中的第二个"),
            "用于三灯头中的第二个"
        );
    }

    #[test]
    fn test_camel_case_empty() {
        assert_eq!(camel_case_to_space_separated(""), "");
    }

    #[test]
    fn test_camel_case_emoji() {
        assert_eq!(camel_case_to_space_separated("🔥lightMode"), "🔥light Mode");
    }

    fn mqtt_handler_basename(stringified: &str) -> &str {
        stringified
            .split("::")
            .map(str::trim)
            .last()
            .unwrap_or(stringified)
    }

    #[test]
    fn mqtt_route_handlers_match_their_patterns() {
        const EXPECTED: &[(&str, &str)] = &[
            ("{hass_disco_prefix}/status", "mqtt_homeassitant_status"),
            ("gv2mqtt/light/:id/command", "mqtt_light_command"),
            (
                "gv2mqtt/light/:id/command/:segment",
                "mqtt_light_segment_command",
            ),
            (
                "gv2mqtt/switch/:id/command/:instance",
                "mqtt_switch_command",
            ),
            ("gv2mqtt/oneclick", "mqtt_oneclick"),
            ("gv2mqtt/purge-caches", "mqtt_purge_caches"),
            (
                "gv2mqtt/:id/request-platform-data",
                "mqtt_request_platform_data",
            ),
            ("gv2mqtt/:id/scene-next", "mqtt_scene_next"),
            ("gv2mqtt/:id/scene-prev", "mqtt_scene_prev"),
            (
                "gv2mqtt/number/:id/command/:mode_name/:work_mode",
                "mqtt_number_command",
            ),
            (
                "gv2mqtt/humidifier/:id/set-mode",
                "mqtt_device_set_work_mode",
            ),
            ("gv2mqtt/:id/set-work-mode", "mqtt_device_set_work_mode"),
            (
                "gv2mqtt/:id/set-music-sensitivity",
                "mqtt_music_sensitivity_command",
            ),
            (
                "gv2mqtt/:id/clear-music-sensitivity",
                "mqtt_clear_music_sensitivity_command",
            ),
            (
                "gv2mqtt/humidifier/:id/set-target",
                "mqtt_humidifier_set_target",
            ),
            (
                "gv2mqtt/:id/set-temperature/:instance/:units",
                "mqtt_set_temperature",
            ),
            ("gv2mqtt/:id/set-mode-scene", "mqtt_set_mode_scene"),
            ("gv2mqtt/:id/set-music-palette", "mqtt_set_music_palette"),
            ("gv2mqtt/fan/:id/set-percentage", "mqtt_fan_set_percentage"),
            ("gv2mqtt/fan/:id/command", "mqtt_fan_command"),
            ("gv2mqtt/energy/:id/counter", "mqtt_energy_counter"),
        ];

        assert_eq!(MQTT_ROUTE_PAIRINGS.len(), EXPECTED.len());
        for (index, ((actual_pattern, actual_handler), (expected_pattern, expected_handler))) in
            MQTT_ROUTE_PAIRINGS.iter().zip(EXPECTED).enumerate()
        {
            assert_eq!(
                *actual_pattern, *expected_pattern,
                "pattern at index {index}"
            );
            assert_eq!(
                mqtt_handler_basename(actual_handler),
                *expected_handler,
                "handler for {expected_pattern} at index {index}"
            );
        }
    }

    /// Exercise the same binding helper used by `rebuild_router` without a
    /// broker. Every inventory topic must be registered even when an optional
    /// handler later rejects a command (for example, music palette opt-in).
    #[tokio::test]
    async fn mqtt_command_routes_register_the_complete_inventory() {
        let mut recorded = Vec::new();
        bind_mqtt_command_routes(&mut MqttRouteBind::Record(&mut recorded), "homeassistant")
            .await
            .expect("recording route bindings cannot fail");

        let mut expected = vec!["homeassistant/status".to_string()];
        expected.extend(
            crate::hass_mqtt::command_routes::REGISTERED_COMMAND_ROUTES
                .iter()
                .map(|route| (*route).to_string()),
        );
        assert_eq!(recorded, expected);
    }

    #[test]
    fn test_scene_cycle_next_from_middle() {
        let scenes: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        assert_eq!(compute_scene_cycle_index(&scenes, Some("B"), 1), 2);
    }

    async fn cycle_catalog_fixture(names: &[&str]) -> StateHandle {
        use crate::service::state::{SceneCatalogCache, SceneCatalogCategory, SceneCatalogEntry};
        let state = Arc::new(crate::service::state::State::new());
        state
            .device_mut("H6062", "cycle-test")
            .await
            .set_scene_catalog(SceneCatalogCache {
                platform_signature: None,
                categories: vec![SceneCatalogCategory {
                    name: "Test scenes".into(),
                    scenes: names
                        .iter()
                        .map(|name| SceneCatalogEntry {
                            name: (*name).into(),
                            icon_urls: vec![],
                            hint: None,
                        })
                        .collect(),
                }],
            });
        state
    }

    #[tokio::test]
    async fn regression_scene_cycle_composes_with_the_executable_catalog() -> anyhow::Result<()> {
        let state = cycle_catalog_fixture(&["", "A", "", "B", ""]).await;
        let device = state.resolve_device_read_only("cycle-test").await?;
        let flat: Vec<String> = state
            .device_list_scenes_categorized(&device)
            .await?
            .into_iter()
            .flat_map(|cat| cat.scenes.into_iter().map(|scene| scene.name))
            .collect();
        assert_eq!(flat, ["A", "B"]);
        for (current, direction, expected) in [
            (None, 1, "A"),
            (None, -1, "B"),
            (Some("unknown"), 1, "A"),
            (Some("unknown"), -1, "B"),
            (Some(""), 1, "A"),
            (Some(""), -1, "B"),
            (Some("a"), 1, "B"),
            (Some("a"), -1, "B"),
            (Some("B"), 1, "A"),
            (Some("B"), -1, "A"),
        ] {
            assert_eq!(
                flat[compute_scene_cycle_index(&flat, current, direction)],
                expected
            );
        }
        assert_eq!(
            state.device_list_scenes(&device).await?,
            flat,
            "selectors and cycling must expose the same executable names"
        );
        Ok(())
    }

    #[tokio::test]
    async fn regression_scene_cycle_keeps_a_real_none_scene() -> anyhow::Result<()> {
        let state = cycle_catalog_fixture(&["", "None", ""]).await;
        let device = state.resolve_device_read_only("cycle-test").await?;
        let scenes = state.device_list_scenes(&device).await?;
        assert_eq!(scenes, ["None"]);
        for current in [None, Some("None"), Some("unknown")] {
            for direction in [-1, 1] {
                assert_eq!(compute_scene_cycle_index(&scenes, current, direction), 0);
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn regression_scene_cycle_empty_only_catalog_is_not_executable() -> anyhow::Result<()> {
        let state = cycle_catalog_fixture(&["", ""]).await;
        let device = state.resolve_device_read_only("cycle-test").await?;
        assert!(state.device_list_scenes(&device).await?.is_empty());
        assert!(
            crate::hass_mqtt::select::SceneModeSelect::new(&device, &state)
                .await?
                .is_none()
        );
        for direction in [-1, 1] {
            let error = scene_cycle(&state, "cycle-test", direction)
                .await
                .err()
                .ok_or_else(|| anyhow::anyhow!("empty catalog was accepted"))?;
            assert!(
                error.to_string().contains("No scenes available"),
                "{error:#}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_scene_cycle_prev_from_middle() {
        let scenes: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        assert_eq!(compute_scene_cycle_index(&scenes, Some("B"), -1), 0);
    }

    #[test]
    fn test_scene_cycle_next_wraps_last_to_first() {
        let scenes: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        assert_eq!(compute_scene_cycle_index(&scenes, Some("C"), 1), 0);
    }

    #[test]
    fn test_scene_cycle_prev_wraps_first_to_last() {
        let scenes: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        assert_eq!(compute_scene_cycle_index(&scenes, Some("A"), -1), 2);
    }

    #[test]
    fn test_scene_cycle_no_active_scene_next() {
        let scenes: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        assert_eq!(compute_scene_cycle_index(&scenes, None, 1), 0);
    }

    #[test]
    fn test_scene_cycle_no_active_scene_prev() {
        let scenes: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        assert_eq!(compute_scene_cycle_index(&scenes, None, -1), 2);
    }

    #[test]
    fn test_scene_cycle_case_insensitive() {
        let scenes: Vec<String> = vec!["Sunset".into(), "Rainbow".into()];
        assert_eq!(compute_scene_cycle_index(&scenes, Some("sunset"), 1), 1);
    }

    #[test]
    fn test_scene_cycle_single_scene() {
        let scenes: Vec<String> = vec!["Only".into()];
        assert_eq!(compute_scene_cycle_index(&scenes, Some("Only"), 1), 0);
        assert_eq!(compute_scene_cycle_index(&scenes, Some("Only"), -1), 0);
    }

    #[test]
    fn test_scene_cycle_unknown_scene_treated_as_no_active() {
        let scenes: Vec<String> = vec!["A".into(), "B".into()];
        assert_eq!(
            compute_scene_cycle_index(&scenes, Some("nonexistent"), 1),
            0
        );
    }

    /// The add-on option must reach the bridge under the name the bridge reads.
    /// `music_palette` is declared in three files that nothing else ties
    /// together: a rename in any one of them leaves a toggle in the Home
    /// Assistant UI that silently does nothing.
    #[test]
    fn addon_music_palette_option_is_wired_to_the_env_var_the_bridge_reads() {
        const OPTION: &str = "music_palette";
        const CONFIG: &str = include_str!("../../addon/config.yaml");
        const RUN_SH: &str = include_str!("../../addon/run.sh");
        const TRANSLATIONS: &str = include_str!("../../addon/translations/en.yaml");

        assert!(
            CONFIG.contains(&format!("{OPTION}: \"bool?\"")),
            "addon/config.yaml must declare {OPTION} as an optional bool"
        );
        // Match against live shell only. `contains` on the raw file is
        // satisfied by a commented-out block, so the whole wiring could be
        // deleted with a `#` and this test would still pass.
        let live: Vec<&str> = RUN_SH
            .lines()
            .map(|line| line.split('#').next().unwrap_or("").trim_end())
            .filter(|line| !line.trim().is_empty())
            .collect();
        let live_contains =
            |needle: &str| -> bool { live.iter().any(|line| line.contains(needle)) };

        assert!(
            live_contains(&format!("bashio::config.has_value {OPTION}")),
            "addon/run.sh must read the {OPTION} option in live shell, not a comment"
        );
        assert!(
            live_contains(&format!(
                "{MUSIC_PALETTE_ENV_VAR}=\"$(bashio::config {OPTION})\""
            )),
            "addon/run.sh must read {OPTION} into {MUSIC_PALETTE_ENV_VAR}"
        );
        let export_at = live
            .iter()
            .position(|line| line.contains(&format!("export {MUSIC_PALETTE_ENV_VAR}")))
            .expect("addon/run.sh must export {MUSIC_PALETTE_ENV_VAR} in live shell");
        // An export placed after the bridge launches never reaches it.
        let launch_at = live
            .iter()
            .position(|line| line.contains("exec ") || line.contains("govee serve"))
            .expect("addon/run.sh must launch the bridge");
        assert!(
            export_at < launch_at,
            "addon/run.sh must export {MUSIC_PALETTE_ENV_VAR} before launching the bridge \
             (export at line {export_at}, launch at line {launch_at} of live shell)"
        );
        assert!(
            TRANSLATIONS.contains(&format!("{OPTION}:")),
            "addon/translations/en.yaml must label {OPTION}, or the add-on \
             config page shows a bare key"
        );
    }

    /// `bashio::config.has_value` is true for a boolean `false`, so switching
    /// the add-on toggle OFF exports `GOVEE_MUSIC_PALETTE=false` rather than
    /// leaving the variable unset. Anything but the exact string "true" has to
    /// keep the reverse-engineered LAN palette path disabled.
    #[test]
    fn music_palette_topic_requires_the_env_var_to_be_exactly_true() {
        for off in ["false", "False", "TRUE", "1", "yes", ""] {
            assert!(
                !music_palette_enabled(Some(off)),
                "{off:?} must not enable the topic"
            );
        }
        assert!(!music_palette_enabled(None));
        assert!(music_palette_enabled(Some("true")));
    }

    #[tokio::test]
    async fn mqtt_dispatch_keys_align_all_device_topic_layouts() {
        const DEVICE_ID: &str = "AA:BB:CC:DD:EE:FF:11:22";
        const DEVICE_IP: &str = "192.0.2.44";
        let state: StateHandle = Arc::new(crate::service::state::State::new());
        let mut info = crate::platform_api::test::music_device();
        info.sku = "H9999".to_string();
        info.device = DEVICE_ID.to_string();
        info.device_name = "Dispatch Test Light".to_string();
        {
            let mut device = state.device_mut(&info.sku, &info.device).await;
            device.set_http_device_info(info);
            device.set_lan_device(crate::lan_api::LanDevice {
                ip: std::net::IpAddr::from([192, 0, 2, 44]),
                device: DEVICE_ID.to_string(),
                sku: "H9999".to_string(),
                ble_version_hard: String::new(),
                ble_version_soft: String::new(),
                wifi_version_hard: String::new(),
                wifi_version_soft: String::new(),
            });
        }

        let expected = Some("aabbccddeeff1122".to_string());
        for topic in [
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/request-platform-data",
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/scene-next",
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/scene-prev",
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/set-work-mode",
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/set-music-sensitivity",
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/clear-music-sensitivity",
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/set-temperature/manual/C",
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/set-mode-scene",
            "gv2mqtt/AA:BB:CC:DD:EE:FF:11:22/set-music-palette",
            "gv2mqtt/light/AABBCCDDEEFF1122/command",
            "gv2mqtt/light/AABBCCDDEEFF1122/command/3",
            "gv2mqtt/switch/AABBCCDDEEFF1122/command/powerSwitch",
            "gv2mqtt/number/AABBCCDDEEFF1122/command/manual/1",
            "gv2mqtt/humidifier/AABBCCDDEEFF1122/set-mode",
            "gv2mqtt/humidifier/AABBCCDDEEFF1122/set-target",
        ] {
            assert_eq!(
                mqtt_device_dispatch_key(&state, topic).await,
                expected,
                "topic {topic} must join the same per-device queue"
            );
        }

        for alias in [
            DEVICE_ID,
            "AABBCCDDEEFF1122",
            "Dispatch Test Light",
            "H9999_1122",
            DEVICE_IP,
        ] {
            let topic = format!("gv2mqtt/{alias}/set-music-sensitivity");
            assert_eq!(
                mqtt_device_dispatch_key(&state, &topic).await,
                expected,
                "alias {alias} must resolve to the canonical device queue"
            );
        }

        for topic in [
            "homeassistant/status",
            "gv2mqtt/AABBCCDDEEFF1122/status",
            "gv2mqtt/availability",
            "gv2mqtt/oneclick",
            "gv2mqtt/purge-caches",
        ] {
            assert_eq!(
                mqtt_device_dispatch_key(&state, topic).await,
                None,
                "global topic {topic} must not be serialized behind a device"
            );
        }

        let mut queues = MqttDispatchQueues::default();
        let (unknown_done_tx, unknown_done_rx) = tokio::sync::oneshot::channel();
        queues
            .enqueue_boxed(
                "gv2mqtt/not-a-device/set-music-sensitivity",
                &state,
                Box::pin(async move {
                    let _ = unknown_done_tx.send(());
                }),
            )
            .await;
        assert!(
            queues.by_device.is_empty(),
            "unknown ids must not retain dispatch workers"
        );
        tokio::time::timeout(Duration::from_secs(1), unknown_done_rx)
            .await
            .expect("an unknown-id command is still dispatched")
            .expect("the unknown-id command reports completion");
    }

    #[tokio::test]
    async fn mqtt_dispatch_queue_keeps_devices_independent() {
        let state: StateHandle = Arc::new(crate::service::state::State::new());
        drop(state.device_mut("H0001", "device-a").await);
        drop(state.device_mut("H0002", "device-b").await);
        let scheduler = start_mqtt_dispatch_scheduler(state);
        let (first_started_tx, first_started_rx) = tokio::sync::oneshot::channel();
        let (release_first_tx, release_first_rx) = tokio::sync::oneshot::channel();
        let (same_device_tx, mut same_device_rx) = tokio::sync::oneshot::channel();
        let (other_device_tx, other_device_rx) = tokio::sync::oneshot::channel();

        assert!(
            scheduler
                .send(MqttDispatchRequest {
                    topic: "gv2mqtt/device-a/set-music-sensitivity".to_string(),
                    task: Box::pin(async move {
                        let _ = first_started_tx.send(());
                        let _ = release_first_rx.await;
                    }),
                })
                .is_ok(),
            "the dispatch scheduler accepts the first device-a command"
        );

        tokio::time::timeout(Duration::from_secs(1), first_started_rx)
            .await
            .expect("the first device-a command starts")
            .expect("the first device-a command reports startup");
        assert!(
            scheduler
                .send(MqttDispatchRequest {
                    topic: "gv2mqtt/light/device-a/command".to_string(),
                    task: Box::pin(async move {
                        let _ = same_device_tx.send(());
                    }),
                })
                .is_ok(),
            "the dispatch scheduler accepts the second device-a command"
        );
        assert!(
            scheduler
                .send(MqttDispatchRequest {
                    topic: "gv2mqtt/light/device-b/command".to_string(),
                    task: Box::pin(async move {
                        let _ = other_device_tx.send(());
                    }),
                })
                .is_ok(),
            "the dispatch scheduler accepts the device-b command"
        );
        tokio::time::timeout(Duration::from_secs(1), other_device_rx)
            .await
            .expect("device-b must not wait for device-a")
            .expect("the device-b command completes");
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut same_device_rx)
                .await
                .is_err(),
            "the second device-a command must wait in FIFO order"
        );

        let _ = release_first_tx.send(());
        tokio::time::timeout(Duration::from_secs(1), same_device_rx)
            .await
            .expect("the queued device-a command proceeds after release")
            .expect("the queued device-a command completes");
    }

    #[tokio::test]
    async fn mqtt_dispatch_queue_survives_a_handler_panic() {
        let state: StateHandle = Arc::new(crate::service::state::State::new());
        drop(state.device_mut("H0001", "device-a").await);
        let scheduler = start_mqtt_dispatch_scheduler(state);
        let (after_panic_tx, after_panic_rx) = tokio::sync::oneshot::channel();

        assert!(
            scheduler
                .send(MqttDispatchRequest {
                    topic: "gv2mqtt/device-a/set-music-sensitivity".to_string(),
                    task: Box::pin(async move {
                        panic!("synthetic handler panic");
                    }),
                })
                .is_ok(),
            "the scheduler accepts the panicking command"
        );
        assert!(
            scheduler
                .send(MqttDispatchRequest {
                    topic: "gv2mqtt/light/device-a/command".to_string(),
                    task: Box::pin(async move {
                        let _ = after_panic_tx.send(());
                    }),
                })
                .is_ok(),
            "the scheduler accepts the command after the panic"
        );

        tokio::time::timeout(Duration::from_secs(1), after_panic_rx)
            .await
            .expect("the device lane continues after a handler panic")
            .expect("the command after the panic reports completion");
    }

    #[tokio::test]
    async fn mqtt_dispatch_applies_sensitivity_before_the_next_music_effect() {
        use crate::platform_api::test::{capture_server, live_path_guard, music_device};

        let _serialized = live_path_guard();
        let (base_url, captured) = capture_server();
        let info = music_device();
        let state: StateHandle = Arc::new(crate::service::state::State::new());
        state
            .set_platform_client(crate::platform_api::GoveeApiClient::new_for_test(
                "test-key", base_url,
            ))
            .await;
        {
            state
                .device_mut(&info.sku, &info.device)
                .await
                .set_http_device_info(info.clone());
        }

        let device = state
            .device_by_id(&info.device)
            .await
            .expect("the music device is in state");
        let topic_id = topic_safe_id(&device);
        let before = captured
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len();
        let (release_sensitivity_tx, release_sensitivity_rx) = tokio::sync::oneshot::channel();
        let (sensitivity_started_tx, sensitivity_started_rx) = tokio::sync::oneshot::channel();
        let (effect_done_tx, mut effect_done_rx) = tokio::sync::oneshot::channel();
        let scheduler = start_mqtt_dispatch_scheduler(state.clone());

        let sensitivity_state = state.clone();
        let sensitivity_id = topic_id.clone();
        assert!(
            scheduler
                .send(MqttDispatchRequest {
                    topic: format!("gv2mqtt/{topic_id}/set-music-sensitivity"),
                    task: Box::pin(async move {
                        let _ = sensitivity_started_tx.send(());
                        let _ = release_sensitivity_rx.await;
                        crate::hass_mqtt::number::mqtt_music_sensitivity_command(
                            Payload("30".to_string()),
                            Params(IdParameter { id: sensitivity_id }),
                            State(sensitivity_state),
                        )
                        .await
                        .expect("the queued sensitivity command succeeds");
                    }),
                })
                .is_ok(),
            "the scheduler accepts the sensitivity command"
        );
        tokio::time::timeout(Duration::from_secs(1), sensitivity_started_rx)
            .await
            .expect("the sensitivity command starts")
            .expect("the sensitivity command reports startup");

        let effect_state = state.clone();
        let effect_id = topic_id.clone();
        assert!(
            scheduler
                .send(MqttDispatchRequest {
                    topic: format!("gv2mqtt/light/{topic_id}/command"),
                    task: Box::pin(async move {
                        let result = mqtt_light_command(
                            Payload(
                                serde_json::json!({
                                    "state": "ON",
                                    "effect": "Music: Rhythm",
                                })
                                .to_string(),
                            ),
                            Params(IdParameter { id: effect_id }),
                            State(effect_state),
                        )
                        .await;
                        let _ = effect_done_tx.send(result);
                    }),
                })
                .is_ok(),
            "the scheduler accepts the Music effect command"
        );

        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut effect_done_rx)
                .await
                .is_err(),
            "the effect must wait for the earlier sensitivity command"
        );
        let _ = release_sensitivity_tx.send(());
        tokio::time::timeout(Duration::from_secs(1), effect_done_rx)
            .await
            .expect("the Music effect proceeds after sensitivity is stored")
            .expect("the effect worker reports completion")
            .expect("the Music effect reaches the capture server");

        let sent = captured.lock().unwrap_or_else(|error| error.into_inner());
        assert_eq!(sent.len(), before + 1, "exactly one Platform request");
        assert_eq!(
            sent[before]["payload"]["capability"]["value"]["sensitivity"], 30,
            "the next Music effect must carry the preceding slider value"
        );
    }

    #[tokio::test]
    async fn music_effect_with_rgb_disables_auto_color_on_the_wire() {
        use crate::platform_api::test::{capture_server, live_path_guard, music_device};

        let _serialized = live_path_guard();
        let (base_url, captured) = capture_server();
        let info = music_device();
        let state: StateHandle = Arc::new(crate::service::state::State::new());
        state
            .set_platform_client(crate::platform_api::GoveeApiClient::new_for_test(
                "test-key", base_url,
            ))
            .await;
        {
            let mut device = state.device_mut(&info.sku, &info.device).await;
            device.set_http_device_info(info.clone());
            device.set_music_sensitivity(42);
        }

        let before = captured.lock().unwrap_or_else(|e| e.into_inner()).len();
        mqtt_light_command(
            Payload(
                serde_json::json!({
                    "state": "ON",
                    "effect": "Music: Rhythm",
                    "color": {"r": 0x12, "g": 0x34, "b": 0x56},
                })
                .to_string(),
            ),
            Params(IdParameter {
                id: info.device.clone(),
            }),
            State(state),
        )
        .await
        .expect("the HA light command reaches the capture server");

        let sent = captured.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(sent.len(), before + 1, "exactly one request");
        let value = &sent[before]["payload"]["capability"]["value"];
        assert_eq!(value["sensitivity"], 42);
        assert_eq!(value["autoColor"], 0);
        assert_eq!(value["rgb"], 0x123456);
    }

    #[tokio::test]
    async fn ordinary_effect_still_ignores_rgb_in_the_same_command() {
        use crate::platform_api::test::{capture_server, live_path_guard};

        let _serialized = live_path_guard();
        let (base_url, captured) = capture_server();
        let info = crate::platform_api::HttpDeviceInfo {
            sku: "H9999".to_string(),
            device: "AA:BB:CC:DD:EE:FF:11:22".to_string(),
            device_name: "Ordinary Scene Test".to_string(),
            device_type: DeviceType::Light,
            capabilities: vec![DeviceCapability {
                kind: DeviceCapabilityKind::Mode,
                instance: "lightScene".to_string(),
                parameters: Some(DeviceParameters::Enum {
                    options: vec![EnumOption {
                        name: "Aurora".to_string(),
                        value: serde_json::json!(7),
                        extras: HashMap::new(),
                    }],
                }),
                alarm_type: None,
                event_state: None,
            }],
        };

        let state: StateHandle = Arc::new(crate::service::state::State::new());
        state
            .set_platform_client(crate::platform_api::GoveeApiClient::new_for_test(
                "test-key", base_url,
            ))
            .await;
        {
            state
                .device_mut(&info.sku, &info.device)
                .await
                .set_http_device_info(info.clone());
        }

        let before = captured.lock().unwrap_or_else(|e| e.into_inner()).len();
        mqtt_light_command(
            Payload(
                serde_json::json!({
                    "state": "ON",
                    "effect": "Aurora",
                    "color": {"r": 0x12, "g": 0x34, "b": 0x56},
                })
                .to_string(),
            ),
            Params(IdParameter {
                id: info.device.clone(),
            }),
            State(state),
        )
        .await
        .expect("the ordinary effect reaches the capture server");

        let sent = captured.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(sent.len(), before + 1, "exactly one request");
        let capability = &sent[before]["payload"]["capability"];
        assert_eq!(capability["instance"], "lightScene");
        assert_eq!(capability["value"], 7);
        assert!(capability["value"].get("rgb").is_none());
        assert!(capability["value"].get("autoColor").is_none());
        assert!(capability["value"].get("sensitivity").is_none());
    }

    /// A device that vanished from the state map between discovery and the
    /// state sweep must be skipped, not turned into an error that aborts the
    /// whole `EntityList::notify_state` loop for every other entity.
    #[tokio::test]
    async fn music_sensitivity_notify_skips_a_device_that_left_the_state_map() {
        let state: StateHandle = Arc::new(crate::service::state::State::new());
        let device = ServiceDevice::new("H607C", "AA:BB:CC:DD:EE:FF:11:22");
        let entity = crate::hass_mqtt::number::MusicSensitivityNumber::new(&device, &state)
            .expect("registered music sensitivity route");
        // Never dropped: destroying a client cancels and joins its network
        // thread, which hangs with MinGW winpthreads on Windows.
        let client = std::mem::ManuallyDrop::new(HassClient {
            client: Client::with_auto_id().expect("mosquitto client"),
        });

        assert!(state.devices().await.is_empty());
        tokio::time::timeout(Duration::from_secs(5), entity.notify_state(&client))
            .await
            .expect("notify must not block")
            .expect("an absent device is skipped, not an error");
    }

    #[tokio::test]
    async fn next_mqtt_event_gives_up_after_reconnect_grace() {
        let (_tx, rx) = async_channel::unbounded::<Event>();
        let since = tokio::time::Instant::now() - MQTT_RECONNECT_GRACE;
        let res = tokio::time::timeout(Duration::from_secs(5), next_mqtt_event(&rx, Some(since)))
            .await
            .expect("the reconnect deadline was ignored");
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn next_mqtt_event_delivers_queued_event_while_disconnected() {
        let (tx, rx) = async_channel::unbounded();
        tx.send(Event::Connected(mosquitto_rs::ConnectionStatus(0)))
            .await
            .unwrap();
        let since = tokio::time::Instant::now();
        assert!(matches!(
            next_mqtt_event(&rx, Some(since)).await,
            Ok(Some(Event::Connected(_)))
        ));
    }

    #[tokio::test]
    async fn next_mqtt_event_ends_with_the_stream() {
        let (tx, rx) = async_channel::unbounded::<Event>();
        drop(tx);
        assert!(matches!(next_mqtt_event(&rx, None).await, Ok(None)));
    }
}

#[cfg(test)]
mod discovery_prefix_test {
    use super::normalize_discovery_prefix;

    #[test]
    fn blank_means_default_and_slashes_are_trimmed() {
        assert_eq!(normalize_discovery_prefix(None), "homeassistant");
        assert_eq!(normalize_discovery_prefix(Some("  ")), "homeassistant");
        assert_eq!(normalize_discovery_prefix(Some("indigo/")), "indigo");
        assert_eq!(normalize_discovery_prefix(Some("/ha/disco")), "ha/disco");
    }
}

#[cfg(test)]
mod device_availability_test {
    use super::device_availability_payload;

    #[test]
    fn only_an_explicit_offline_report_counts() {
        assert_eq!(device_availability_payload(Some(false)), "offline");
        assert_eq!(device_availability_payload(Some(true)), "online");
        assert_eq!(device_availability_payload(None), "online");
    }
}

#[cfg(test)]
mod transition_test {
    use super::*;

    #[test]
    fn transition_seconds_are_clamped() {
        assert_eq!(clamp_transition_seconds(3.0), 3.0);
        assert_eq!(clamp_transition_seconds(-10.0), 0.0);
        assert_eq!(clamp_transition_seconds(100000.0), 3600.0);
        assert_eq!(clamp_transition_seconds(f64::NAN), 0.0);
        assert_eq!(clamp_transition_seconds(f64::INFINITY), 0.0);
    }

    #[test]
    fn transition_plan_hits_step_targets() {
        // A 15s transition gets one step per second, 15 steps total
        let (steps, interval) = transition_plan(Duration::from_secs(15));
        assert_eq!(steps, 15);
        assert_eq!(interval, Duration::from_secs(1));

        // A very long transition is capped at the max step count
        let (steps, interval) = transition_plan(Duration::from_secs(3600));
        assert_eq!(steps, TRANSITION_MAX_STEPS);
        assert_eq!(interval, Duration::from_secs(180));

        // A short transition still respects the minimum interval:
        // 3 seconds yields 3 one-second steps, not 10 sub-second ones
        let (steps, interval) = transition_plan(Duration::from_secs(3));
        assert_eq!(steps, 3);
        assert_eq!(interval, Duration::from_secs(1));

        // Anything shorter than one step degrades to a single final set
        let (steps, _) = transition_plan(Duration::from_millis(500));
        assert_eq!(steps, 1);
        let (steps, interval) = transition_plan(Duration::ZERO);
        assert_eq!((steps, interval), (1, Duration::ZERO));
    }

    #[test]
    fn short_transitions_apply_immediately() {
        assert!(!is_fade(Duration::from_millis(250)));
        assert!(!is_fade(Duration::from_millis(1900)));
        assert!(is_fade(Duration::from_secs(2)));
        assert!(is_fade(Duration::from_secs(3600)));
    }

    #[test]
    fn ramp_values_endpoints_and_monotonicity() {
        let up = ramp_values(10, 100, 10);
        assert_eq!(up.first(), Some(&19));
        assert_eq!(up.last(), Some(&100));
        assert!(up.windows(2).all(|w| w[0] < w[1]));

        let down = ramp_values(100, 10, 10);
        assert_eq!(down.last(), Some(&10));
        assert!(down.windows(2).all(|w| w[0] > w[1]));

        // No-op ramps and degenerate step counts still land exactly on target
        assert_eq!(ramp_values(50, 50, 10), vec![50]);
        assert_eq!(ramp_values(0, 100, 0), vec![100]);

        // When rounding collapses steps for a tiny range we never emit
        // the same value twice in a row, and we always finish on target
        let small = ramp_values(10, 12, 20);
        assert_eq!(small.last(), Some(&12));
        assert!(small.windows(2).all(|w| w[0] != w[1]));
    }

    /// A superseded ramp must not touch the device. This device has no way to
    /// be controlled, so any attempt would be an error.
    #[tokio::test]
    async fn a_newer_command_ends_the_ramp() {
        let state = std::sync::Arc::new(crate::service::state::State::new());
        let device = ServiceDevice::new("H9999", "AA:BB");
        drop(state.device_mut("H9999", "AA:BB").await);

        let first = state.resolve_device_for_control_without_poll("AA:BB").await;
        let generation = first.expect("device").generation();
        let ramp = |state: &StateHandle| {
            ramp_brightness(
                state.clone(),
                device.clone(),
                generation,
                1,
                50,
                Duration::ZERO,
            )
        };
        assert!(ramp(&state).await.is_err(), "still current, so it tries");

        drop(state.resolve_device_for_control_without_poll("AA:BB").await);
        assert!(ramp(&state).await.is_ok(), "superseded, so it stops");
    }
}
