use crate::ble::{Base64HexBytes, GoveeBlePacket, HumidifierAutoMode, NotifyHumidifierMode};
use crate::lan_api::{DeviceColor, DeviceStatus};
use crate::platform_api::from_json;
use crate::service::device::Device;
use crate::service::state::StateHandle;
use crate::undoc_api::{ms_timestamp, DeviceEntry, LoginAccountResponse, ParsedOneClick};
use crate::UndocApiArguments;
use anyhow::Context;
use async_channel::Receiver;
use chrono::Utc;
use mosquitto_rs::{Event, QoS};
use serde::{Deserialize, Deserializer};
use std::time::Duration;
use tokio::time::timeout;

#[derive(Clone)]
pub struct IotClient {
    client: mosquitto_rs::Client,
}

impl IotClient {
    pub fn is_device_compatible(&self, device: &DeviceEntry) -> bool {
        device.device_ext.device_settings.topic.is_some()
    }

    pub async fn request_status_update(&self, device: &DeviceEntry) -> anyhow::Result<()> {
        let device_topic = device.device_topic()?;

        self.client
            .publish(
                device_topic,
                serde_json::to_string(&serde_json::json!({
                    "msg": {
                        "cmd": "status",
                        "cmdVersion": 2,
                        "transaction": format!("v_{}000", ms_timestamp()),
                        "type": 0,
                    }
                }))?,
                QoS::AtMostOnce,
                false,
            )
            .await?;

        Ok(())
    }

    pub async fn set_power_state(&self, device: &DeviceEntry, on: bool) -> anyhow::Result<()> {
        log::trace!("set_power_state for {} to {on}", device.device);
        let device_topic = device.device_topic()?;

        fn pwr(is_on: bool, on: u8, off: u8) -> u8 {
            if is_on {
                on
            } else {
                off
            }
        }

        let power_state = match device.sku.as_str() {
            "H5080" | "H5083" => pwr(on, 17, 16),
            _ => pwr(on, 1, 0),
        };

        self.client
            .publish(
                device_topic,
                serde_json::to_string(&serde_json::json!({
                    "msg": {
                        "cmd": "turn",
                        "data": {
                            "val": power_state,
                        },
                        "cmdVersion": 0,
                        "transaction": format!("v_{}000", ms_timestamp()),
                        "type": 1,
                    }
                }))?,
                QoS::AtMostOnce,
                false,
            )
            .await
            .context("IotClient::set_power_state")?;
        Ok(())
    }

    pub async fn set_brightness(&self, device: &DeviceEntry, percent: u8) -> anyhow::Result<()> {
        log::trace!("set_brightness for {} to {percent}", device.device);
        let device_topic = device.device_topic()?;
        self.client
            .publish(
                device_topic,
                serde_json::to_string(&serde_json::json!({
                    "msg": {
                        "cmd": "brightness",
                        "data": {
                            "val": percent,
                        },
                        "cmdVersion": 0,
                        "transaction": format!("v_{}000", ms_timestamp()),
                        "type": 1,
                    }
                }))?,
                QoS::AtMostOnce,
                false,
            )
            .await
            .context("IotClient::set_brightness")?;
        Ok(())
    }

    pub async fn set_color_temperature(
        &self,
        device: &DeviceEntry,
        kelvin: u32,
    ) -> anyhow::Result<()> {
        log::trace!("set_color_temperature for {} to {kelvin}", device.device);
        let device_topic = device.device_topic()?;

        self.client
            .publish(
                device_topic,
                serde_json::to_string(&serde_json::json!({
                    "msg": {
                        "cmd": "colorwc",
                        "data": {
                            "color": {
                                "r": 0,
                                "g": 0,
                                "b": 0,
                            },
                            "colorTemInKelvin": kelvin,
                        },
                        "cmdVersion": 0,
                        "transaction": format!("v_{}000", ms_timestamp()),
                        "type": 1,
                    }
                }))?,
                QoS::AtMostOnce,
                false,
            )
            .await
            .context("IotClient::set_color_temperature")?;
        Ok(())
    }

    pub async fn set_color_rgb(
        &self,
        device: &DeviceEntry,
        r: u8,
        g: u8,
        b: u8,
    ) -> anyhow::Result<()> {
        log::trace!("set_color_rgb for {} to {r},{g},{b}", device.device);
        let device_topic = device.device_topic()?;

        self.client
            .publish(
                device_topic,
                serde_json::to_string(&serde_json::json!({
                    "msg": {
                        "cmd": "colorwc",
                        "data": {
                            "color":{
                                "r": r,
                                "g": g,
                                "b": b,
                            },
                            "colorTemInKelvin": 0,
                        },
                        "cmdVersion": 0,
                        "transaction": format!("v_{}000", ms_timestamp()),
                        "type": 1,
                    }
                }))?,
                QoS::AtMostOnce,
                false,
            )
            .await
            .context("IotClient::set_color_rgb")?;
        Ok(())
    }

    pub async fn send_real(
        &self,
        device: &DeviceEntry,
        commands: Vec<String>,
    ) -> anyhow::Result<()> {
        log::trace!("send_real for {} to {commands:?}", device.device);
        let device_topic = device.device_topic()?;

        self.client
            .publish(
                device_topic,
                serde_json::to_string(&serde_json::json!({
                    "msg": {
                        "cmd": "ptReal",
                        "data": {
                            "command": commands,
                        },
                        "cmdVersion": 0,
                        "transaction": format!("v_{}000", ms_timestamp()),
                        "type": 1,
                    }
                }))?,
                QoS::AtMostOnce,
                false,
            )
            .await
            .context("IotClient::send_real")?;
        Ok(())
    }

    pub async fn activate_one_click(&self, item: &ParsedOneClick) -> anyhow::Result<()> {
        anyhow::ensure!(
            !item.entries.is_empty(),
            "{} has nothing that can be sent. Govee gives no command for some              actions, such as DreamView ones",
            item.name
        );
        for entry in &item.entries {
            for command in &entry.msgs {
                self.client
                    .publish(
                        entry.topic.as_str(),
                        serde_json::to_string(command)?,
                        QoS::AtMostOnce,
                        false,
                    )
                    .await
                    .context("sending OneClick")?;
            }
        }
        Ok(())
    }
}

/// How long we give libmosquitto to re-establish a dropped IoT connection
/// before we give up on that client and build a fresh one.
///
/// libmosquitto's own reconnect normally succeeds within a couple of
/// seconds, but it does not always try: when the broker drops the TCP
/// connection without a TLS close_notify, OpenSSL 3 reports
/// `unexpected eof while reading` as a protocol error, libmosquitto sets
/// `errno = EPROTO`, and `mosquitto_loop_forever` treats that as fatal and
/// returns. The loop thread exits, nothing reconnects, and the only
/// visible trace is a single `IoT disconnected` warning.
pub(crate) const IOT_RECONNECT_GRACE: Duration = Duration::from_secs(60);

/// Delay before building a replacement IoT client; doubles on each
/// consecutive failure up to `IOT_RESTART_MAX_DELAY`.
const IOT_RESTART_MIN_DELAY: Duration = Duration::from_secs(5);
const IOT_RESTART_MAX_DELAY: Duration = Duration::from_secs(300);

/// A connected IoT client together with its event stream
struct IotSession {
    client: mosquitto_rs::Client,
    subscriptions: Receiver<Event>,
    acct: LoginAccountResponse,
}

pub async fn start_iot_client(
    args: &UndocApiArguments,
    state: StateHandle,
    acct: Option<LoginAccountResponse>,
) -> anyhow::Result<()> {
    let session = connect_iot_client(args, &state, acct).await?;

    let args = args.clone();
    tokio::spawn(async move {
        run_iot_supervisor(args, state, session).await;
    });

    Ok(())
}

async fn connect_iot_client(
    args: &UndocApiArguments,
    state: &StateHandle,
    acct: Option<LoginAccountResponse>,
) -> anyhow::Result<IotSession> {
    let client = args.api_client()?;
    let acct = match acct {
        Some(a) => a,
        None => client.login_account_cached().await?,
    };
    log::trace!("{acct:#?}");
    let res = client.get_iot_key(&acct.token).await?;
    log::trace!("{res:#?}");

    let key_bytes = data_encoding::BASE64.decode(res.p12.as_bytes())?;

    log::trace!("parsing IoT PFX key");
    let container = p12::PFX::parse(&key_bytes).context("PFX::parse")?;
    for key in container.key_bags(&res.p12_pass).context("key_bags")? {
        let priv_key = openssl::pkey::PKey::private_key_from_der(&key).context("from_der")?;
        let pem = priv_key
            .private_key_to_pem_pkcs8()
            .context("to_pem_pkcs8")?;
        std::fs::write(&args.govee_iot_key, &pem)?;
    }
    for cert in container.cert_bags(&res.p12_pass).context("cert_bags")? {
        let cert = openssl::x509::X509::from_der(&cert).context("x509 from der")?;
        let pem = cert.to_pem().context("cert.to_pem")?;
        std::fs::write(&args.govee_iot_cert, &pem)?;
    }

    let client = mosquitto_rs::Client::with_id(
        &format!(
            "AP/{account_id}/{id}",
            account_id = *acct.account_id,
            id = uuid::Uuid::new_v4().simple()
        ),
        true,
    )
    .context("new client")?;
    client
        .configure_tls(
            Some(&args.amazon_root_ca),
            None::<&std::path::Path>,
            Some(&args.govee_iot_cert),
            Some(&args.govee_iot_key),
            None,
        )
        .context("configure_tls")?;
    log::trace!("Connecting to IoT {} port 8883", res.endpoint);
    let status = timeout(
        Duration::from_secs(60),
        client.connect(&res.endpoint, 8883, Duration::from_secs(120), None),
    )
    .await
    .with_context(|| format!("timeout connecting to IoT {}:8883 in AWS", res.endpoint))?
    .with_context(|| format!("failed to connect to IoT {}:8883 in AWS", res.endpoint))?;
    log::info!("Connected to IoT: {}:8883 {status}", res.endpoint);

    let subscriptions = client
        .subscriber()
        .context("subscriber was already taken")?;

    state
        .set_iot_client(IotClient {
            client: client.clone(),
        })
        .await;

    Ok(IotSession {
        client,
        subscriptions,
        acct,
    })
}

/// Runs the IoT event loop for `session`, and whenever that session ends,
/// for whatever reason, builds a replacement client so that the IoT path
/// recovers without needing the whole process to be restarted.
async fn run_iot_supervisor(args: UndocApiArguments, state: StateHandle, session: IotSession) {
    let mut session = session;
    let mut delay = IOT_RESTART_MIN_DELAY;
    loop {
        let started = tokio::time::Instant::now();
        let IotSession {
            client,
            subscriptions,
            acct,
        } = session;
        match run_iot_subscriber(subscriptions, state.clone(), client, acct).await {
            Ok(()) => log::warn!("IoT loop terminated"),
            Err(err) => log::error!("IoT loop failed: {err:#}"),
        }

        // A session that stayed up for a while earns a quick restart; one
        // that died young backs off so that we can't spin against the
        // Govee API.
        if started.elapsed() >= IOT_RESTART_MAX_DELAY {
            delay = IOT_RESTART_MIN_DELAY;
        }

        session = loop {
            log::info!("Starting a new IoT client in {delay:?}");
            tokio::time::sleep(delay).await;
            delay = next_restart_delay(delay);
            match connect_iot_client(&args, &state, None).await {
                Ok(session) => break session,
                Err(err) => log::error!("Failed to start a new IoT client: {err:#}"),
            }
        };
    }
}

fn next_restart_delay(delay: Duration) -> Duration {
    (delay * 2).min(IOT_RESTART_MAX_DELAY)
}

/// The outcome of waiting for the next IoT event
#[derive(Debug)]
pub(crate) enum WatchedEvent {
    Event(Event),
    /// The client closed its event stream; it will not produce more events
    Closed,
    /// The connection was lost and has not come back within the grace period
    Stalled(Duration),
}

/// Tracks whether the IoT connection is up, so that a disconnect that
/// libmosquitto never recovers from is noticed instead of waited on forever.
pub(crate) struct ConnectionWatch {
    grace: Duration,
    disconnected_since: Option<tokio::time::Instant>,
}

impl ConnectionWatch {
    pub(crate) fn new(grace: Duration) -> Self {
        Self {
            grace,
            disconnected_since: None,
        }
    }

    pub(crate) async fn recv(&mut self, subscriptions: &Receiver<Event>) -> WatchedEvent {
        let event = match self.disconnected_since {
            None => subscriptions.recv().await,
            Some(since) => {
                match tokio::time::timeout_at(since + self.grace, subscriptions.recv()).await {
                    Ok(event) => event,
                    Err(_) => return WatchedEvent::Stalled(since.elapsed()),
                }
            }
        };
        let Ok(event) = event else {
            return WatchedEvent::Closed;
        };
        match &event {
            Event::Disconnected(_) => {
                if self.disconnected_since.is_none() {
                    self.disconnected_since = Some(tokio::time::Instant::now());
                }
            }
            Event::Connected(_) => {
                self.disconnected_since = None;
            }
            Event::Message(_) => {}
        }
        WatchedEvent::Event(event)
    }
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Packet {
    sku: Option<String>,
    device: Option<String>,
    /// may actually be found in msg.cmd
    cmd: Option<String>,
    /// This is an embedded json string
    msg: Option<String>,
    state: StateUpdate,
    op: Option<OpData>,
}

#[derive(Deserialize, Debug)]
struct StateUpdate {
    #[serde(rename = "onOff")]
    pub on_off: Option<u8>,
    pub brightness: Option<u8>,
    pub color: Option<DeviceColor>,
    #[serde(rename = "colorTemInKelvin")]
    pub color_temperature_kelvin: Option<u32>,
    /// Active work mode number, present in `cmd:"status"` messages
    /// from (at least) light devices. SKU-specific meaning.
    #[serde(default)]
    pub mode: Option<i64>,
    pub sku: Option<String>,
    pub device: Option<String>,
    /// Kept as a plain value, so that devices sending something else
    /// here can't break the parsing of the whole packet
    pub sta: Option<serde_json::Value>,
}

impl StateUpdate {
    /// sta.stc holds underscore separated numbers, eg: "19_0_77_40_1".
    /// Seen with the H5086
    fn stc_numbers(&self) -> Option<std::str::Split<'_, char>> {
        Some(self.sta.as_ref()?.get("stc")?.as_str()?.split('_'))
    }

    /// The fourth number in sta.stc is the uptime in seconds:
    /// it restarts when the plug reboots
    fn uptime_seconds(&self) -> Option<u64> {
        self.stc_numbers()?
            .nth(3)?
            .parse::<u32>()
            .ok()
            .map(u64::from)
    }

    /// The first number in sta.stc is 19 in replies to our status
    /// requests, but 0 in replies to the Govee app's, and 0 or 1 right
    /// after a reboot. Once a day, a reply to the app carries a small
    /// energy counter that isn't today's, which looked like a reset
    fn answers_other_client(&self) -> bool {
        self.stc_numbers()
            .and_then(|mut numbers| numbers.next())
            .is_some_and(|first| first != "19")
    }
}

#[derive(Deserialize, Debug)]
#[allow(unused)]
struct OpData {
    #[serde(default, deserialize_with = "readable_frames")]
    command: Vec<Base64HexBytes>,

    // The next 4 fields are sourced from H6199
    // <https://github.com/wez/govee2mqtt/issues/36>
    #[serde(rename = "modeValue", default, deserialize_with = "readable_frames")]
    mode_value: Vec<Base64HexBytes>,
    #[serde(rename = "sleepValue", default, deserialize_with = "readable_frames")]
    sleep_value: Vec<Base64HexBytes>,
    #[serde(rename = "wakeupValue", default, deserialize_with = "readable_frames")]
    wakeup_value: Vec<Base64HexBytes>,
    #[serde(rename = "timerValue", default, deserialize_with = "readable_frames")]
    timer_value: Vec<Base64HexBytes>,
}

/// Keep the frames that can be read and skip the rest. An H7020 was seen to
/// send an entry that is not base64, and failing on it lost the whole status.
fn readable_frames<'de, D>(deserializer: D) -> Result<Vec<Base64HexBytes>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| match Base64HexBytes::deserialize(entry) {
            Ok(frame) => Some(frame),
            Err(err) => {
                log::debug!("Skipping an IoT frame that cannot be read: {err}");
                None
            }
        })
        .collect())
}

impl Packet {
    /// The sku can be in a couple of different places(!)
    fn sku(&self) -> Option<&str> {
        if let Some(sku) = self.sku.as_deref() {
            return Some(sku);
        }
        self.state.sku.as_deref()
    }
    fn device(&self) -> Option<&str> {
        if let Some(device) = self.device.as_deref() {
            return Some(device);
        }
        self.state.device.as_deref()
    }

    fn sku_and_device(&self) -> Option<(&str, &str)> {
        let sku = self.sku()?;
        let device = self.device()?;
        Some((sku, device))
    }
}

/// Merge one IoT packet into the device's cached IoT status.
///
/// Extracted from the subscriber loop so tests can drive it with real
/// payloads. One subtlety lives here rather than in `set_iot_device_status`:
/// that setter re-stamps `last_iot_device_status_update` for every packet,
/// including mode-less ones whose merge merely carries the cached `mode`
/// forward. The mode observation time is therefore stamped at this merge,
/// and only when the packet explicitly carries `state.mode`.
fn merge_iot_packet(device: &mut Device, sku: &str, packet: &Packet) {
    let mut state = match device.iot_device_status.clone() {
        Some(state) => state,
        None => match device.device_state() {
            Some(state) => DeviceStatus {
                on: state.on,
                brightness: state.brightness,
                color: state.color,
                color_temperature_kelvin: state.kelvin,
                mode: state.mode,
            },
            None => DeviceStatus::default(),
        },
    };

    if let Some(v) = packet.state.brightness {
        state.brightness = v;
        state.on = v != 0;
    }
    if let Some(v) = packet.state.color {
        state.color = v;
        state.on = true;
    }
    if let Some(v) = packet.state.color_temperature_kelvin {
        state.color_temperature_kelvin = v;
        state.on = true;
    }
    if let Some(v) = packet.state.mode {
        state.mode = Some(v);
        device.last_iot_mode_update = Some(Utc::now());
    }

    if let Some(op) = &packet.op {
        for cmd in &op.command {
            let decoded = cmd.decode_for_sku(sku);
            log::debug!("Decoded: {decoded:?} for {sku}");
            match decoded {
                GoveeBlePacket::NotifyEnergyMonitoring(_)
                    if packet.state.answers_other_client() =>
                {
                    // Our next request gets the real readings
                }
                GoveeBlePacket::NotifyEnergyMonitoring(report) => {
                    device.set_energy_monitoring(report);
                    // Without uptime we couldn't tell a reboot from a reset,
                    // so such a reading isn't tracked
                    if let Some(uptime) = packet.state.uptime_seconds() {
                        device.update_energy_counter(&report, uptime);
                    }
                }
                GoveeBlePacket::NotifyElementState(params) => {
                    device.set_element_state(params);
                }
                GoveeBlePacket::NotifyKettleBaseStatus(status) => {
                    device.set_kettle_base_status(status);
                }
                GoveeBlePacket::NotifyKettleHeatingState(heating) => {
                    device.set_kettle_heating_state(heating);
                }
                GoveeBlePacket::NotifyKettleKeepWarm(keep_warm) => {
                    device.set_kettle_keep_warm(keep_warm);
                }
                GoveeBlePacket::NotifyHumidifierNightlight(nl) => {
                    state.brightness = nl.brightness;
                    state.color = DeviceColor {
                        r: nl.r,
                        g: nl.g,
                        b: nl.b,
                    };
                    device.set_nightlight_state(nl);
                }
                GoveeBlePacket::NotifyHumidifierAutoMode(HumidifierAutoMode {
                    target_humidity,
                }) => {
                    device.set_target_humidity(target_humidity.as_percent());
                }
                GoveeBlePacket::NotifyHumidifierMode(NotifyHumidifierMode { mode, param }) => {
                    device.set_humidifier_work_mode_and_param(mode, param);
                }
                GoveeBlePacket::Generic(_) => {
                    // Ignore packets that we can't decode
                }
                GoveeBlePacket::SetHumidifierMode(_)
                | GoveeBlePacket::SetElementToggle(_)
                | GoveeBlePacket::SetHumidifierNightlight(_) => {
                    // Ignore packets that are essentially echoing
                    // commands sent to the device
                }
                _ => {
                    // But warn about the ones we could decode and
                    // aren't handling here
                    log::warn!("Taking no action for {decoded:?} for {sku}");
                }
            }
        }
    }

    // Check on/off last, as we can synthesize "on"
    // if the other fields are present
    if let Some(on_off) = packet.state.on_off {
        state.on = on_off != 0;
    }
    device.set_iot_device_status(state);
}

async fn run_iot_subscriber(
    subscriptions: Receiver<Event>,
    state: StateHandle,
    client: mosquitto_rs::Client,
    acct: LoginAccountResponse,
) -> anyhow::Result<()> {
    let mut watch = ConnectionWatch::new(IOT_RECONNECT_GRACE);
    loop {
        let event = match watch.recv(&subscriptions).await {
            WatchedEvent::Event(event) => event,
            WatchedEvent::Closed => break,
            WatchedEvent::Stalled(elapsed) => {
                anyhow::bail!("IoT connection was lost and has not reconnected after {elapsed:?}");
            }
        };
        match event {
            Event::Message(msg) => {
                let payload = String::from_utf8_lossy(&msg.payload);
                log::trace!("{} -> {payload}", msg.topic);

                match from_json::<Packet, _>(&msg.payload) {
                    Ok(packet) => {
                        log::debug!("{packet:?}");
                        if let Some((sku, device_id)) = packet.sku_and_device() {
                            {
                                let mut device = state.device_mut(sku, device_id).await;
                                merge_iot_packet(&mut device, sku, &packet);
                            }
                            // Home Assistant's broker being away for a moment is
                            // no reason to end the Govee session
                            if let Err(err) = state.notify_of_state_change(device_id).await {
                                log::warn!("Telling Home Assistant about {device_id}: {err:#}");
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Decoding IoT Packet: {err:#} {payload}");
                    }
                }
            }
            Event::Disconnected(reason) => {
                log::warn!("IoT disconnected with reason {reason}");
            }
            Event::Connected(status) => {
                log::info!("IoT (re)connected with status {status}");

                client
                    .subscribe(&acct.topic, mosquitto_rs::QoS::AtMostOnce)
                    .await
                    .context("subscribe to account topic")?;
                // This logic tries to subscribe to the same data that is
                // being sent to the individual devices, but the server
                // will close the connection on us when we try this.
                if false {
                    let devices = state.devices().await;
                    for d in devices {
                        if let Some(undoc) = &d.undoc_device_info {
                            if let Ok(topic) = undoc.entry.device_topic() {
                                client
                                    .subscribe(topic, mosquitto_rs::QoS::AtMostOnce)
                                    .await
                                    .with_context(|| {
                                        format!("subscribe to device topic {topic}")
                                    })?;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    /// Trimmed from a live H607C `cmd:"status"` push (2026-08-04 capture,
    /// proof/pr38-runtime-iot-trace.log): `mode` rides at the top level of
    /// `state`, next to onOff/brightness/color. 5 = manual colour on this SKU.
    const STATUS_WITH_MODE: &str = r#"{
        "proType": 2,
        "sku": "H607C",
        "device": "30:F2:AA:BB:CC:DD:EE:FF",
        "cmd": "status",
        "type": 0,
        "transaction": "o_1785886845822",
        "pactType": 2,
        "pactCode": 1,
        "state": {
            "onOff": 0,
            "brightness": 100,
            "color": {"r": 255, "g": 0, "b": 0},
            "colorTemInKelvin": 0,
            "mode": 5,
            "sta": {"stc": "0_0_49_703425_-703_0_0_400004_38"},
            "result": 1
        },
        "op": {"command": ["qgUVAAAAAAAAAAAAAAAAAAAAALo="]}
    }"#;

    /// The first entry is what an H7020 sent, as recorded by phischl
    #[test]
    fn an_unreadable_frame_does_not_lose_the_status() {
        let packet: Packet = serde_json::from_str(
            r#"{
                "sku": "H7020",
                "device": "AA:BB:CC:DD:EE:FF:00:11",
                "state": {"onOff": 1, "brightness": 50},
                "op": {"command": [
                    "dfdebb7e6883b26be49d8dfa109",
                    "qgUVAAAAAAAAAAAAAAAAAAAAALo=",
                    17
                ]}
            }"#,
        )
        .unwrap();

        assert_eq!(packet.state.brightness, Some(50));
        assert_eq!(packet.op.map(|op| op.command.len()), Some(1));
    }

    #[test]
    fn parses_top_level_mode_from_status_push() {
        let packet: Packet = serde_json::from_str(STATUS_WITH_MODE).unwrap();
        assert_eq!(packet.state.mode, Some(5));
        assert_eq!(packet.state.on_off, Some(0));
        assert_eq!(packet.state.brightness, Some(100));
    }

    /// Payloads without the field (e.g. relayed devStatus shapes) must not
    /// invent one: `mode` stays `None` rather than defaulting to a number.
    #[test]
    fn missing_mode_parses_as_none() {
        let packet: Packet = serde_json::from_str(
            r#"{"sku":"H6000","device":"AA","cmd":"status",
                "state":{"onOff":1,"brightness":50}}"#,
        )
        .unwrap();
        assert_eq!(packet.state.mode, None);
    }

    /// A mode-bearing packet followed by a mode-less one: the merge carries
    /// the cached mode forward and re-stamps the status time, but the mode
    /// observation time must stay pinned to the packet that actually
    /// carried `state.mode` — otherwise every projection would present an
    /// hours-old mode as freshly observed.
    #[test]
    fn mode_less_packet_keeps_mode_observation_time_pinned() {
        let mut device = Device::new("H607C", "30:F2:AA:BB:CC:DD:EE:FF");

        let with_mode: Packet = serde_json::from_str(STATUS_WITH_MODE).unwrap();
        merge_iot_packet(&mut device, "H607C", &with_mode);
        let observed = device
            .last_iot_mode_update
            .expect("mode observation stamped");

        let mode_less: Packet = serde_json::from_str(
            r#"{"sku":"H607C","device":"30:F2:AA:BB:CC:DD:EE:FF","cmd":"status",
                "state":{"onOff":1,"brightness":42}}"#,
        )
        .unwrap();
        merge_iot_packet(&mut device, "H607C", &mode_less);

        let status = device.iot_device_status.as_ref().expect("cached status");
        assert_eq!(status.mode, Some(5), "cached mode carries forward");
        assert_eq!(device.last_iot_mode_update, Some(observed), "stamp pinned");
        assert!(
            device
                .last_iot_device_status_update
                .expect("status stamped")
                >= observed,
            "the mode-less packet still refreshes the status time"
        );

        let state = device.device_state().expect("device state");
        assert_eq!(state.mode, Some(5));
        assert_eq!(state.mode_updated, Some(observed));
    }
}

#[cfg(test)]
/// A TLS listener that speaks just enough MQTT to accept a CONNECT.
/// It drops the first connection at the TCP level, without a TLS
/// close_notify, which is what the AWS IoT endpoint was observed to do.
pub(crate) mod test_broker {
    use openssl::asn1::Asn1Time;
    use openssl::hash::MessageDigest;
    use openssl::pkey::PKey;
    use openssl::rsa::Rsa;
    use openssl::ssl::{SslAcceptor, SslMethod};
    use openssl::x509::extension::SubjectAlternativeName;
    use openssl::x509::{X509NameBuilder, X509};
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    pub struct Broker {
        pub port: u16,
        pub ca_file: PathBuf,
        pub connections: Arc<AtomicUsize>,
    }

    fn read_packet(stream: &mut impl Read) -> std::io::Result<u8> {
        let mut header = [0u8; 1];
        stream.read_exact(&mut header)?;
        let mut remaining = 0usize;
        let mut shift = 0;
        loop {
            let mut b = [0u8; 1];
            stream.read_exact(&mut b)?;
            remaining |= ((b[0] & 0x7f) as usize) << shift;
            shift += 7;
            if b[0] & 0x80 == 0 {
                break;
            }
        }
        let mut body = vec![0u8; remaining];
        stream.read_exact(&mut body)?;
        Ok(header[0] & 0xf0)
    }

    pub fn start() -> Broker {
        let rsa = Rsa::generate(2048).unwrap();
        let key = PKey::from_rsa(rsa).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "localhost").unwrap();
        let name = name.build();
        let mut cert = X509::builder().unwrap();
        cert.set_version(2).unwrap();
        cert.set_subject_name(&name).unwrap();
        cert.set_issuer_name(&name).unwrap();
        cert.set_pubkey(&key).unwrap();
        cert.set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        cert.set_not_after(&Asn1Time::days_from_now(1).unwrap())
            .unwrap();
        let san = SubjectAlternativeName::new()
            .dns("localhost")
            .ip("127.0.0.1")
            .build(&cert.x509v3_context(None, None))
            .unwrap();
        cert.append_extension(san).unwrap();
        cert.sign(&key, MessageDigest::sha256()).unwrap();
        let cert = cert.build();

        let dir = std::env::temp_dir().join(format!(
            "govee2mqtt-iot-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let ca_file = dir.join("ca.pem");
        std::fs::write(&ca_file, cert.to_pem().unwrap()).unwrap();

        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_private_key(&key).unwrap();
        acceptor.set_certificate(&cert).unwrap();
        let acceptor = acceptor.build();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let connections = Arc::new(AtomicUsize::new(0));
        let counter = connections.clone();

        std::thread::spawn(move || {
            let mut keep = vec![];
            for tcp in listener.incoming() {
                let Ok(tcp) = tcp else { continue };
                let Ok(mut tls) = acceptor.accept(tcp) else {
                    continue;
                };
                // 0x10 is CONNECT
                if read_packet(&mut tls).ok() != Some(0x10) {
                    continue;
                }
                // CONNACK, session not present, accepted
                if tls.write_all(&[0x20, 0x02, 0x00, 0x00]).is_err() {
                    continue;
                }
                let n = counter.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    // Close the TCP connection under the TLS session,
                    // with no close_notify.
                    let _ = tls.get_ref().shutdown(Shutdown::Both);
                } else {
                    keep.push(tls);
                }
            }
        });

        Broker {
            port,
            ca_file,
            connections,
        }
    }
}

#[cfg(test)]
mod reconnect_test {
    use super::test_broker;
    use super::*;
    use mosquitto_rs::{ConnectionStatus, ReasonCode};

    fn connected() -> Event {
        Event::Connected(ConnectionStatus(0))
    }

    /// MOSQ_ERR_CONN_LOST, which is what the TLS EOF case reports
    fn conn_lost() -> Event {
        Event::Disconnected(ReasonCode(7))
    }

    #[tokio::test]
    async fn watch_passes_events_through_while_connected() {
        let (tx, rx) = async_channel::unbounded();
        let mut watch = ConnectionWatch::new(Duration::from_millis(50));
        tx.send(connected()).await.unwrap();
        assert!(matches!(
            watch.recv(&rx).await,
            WatchedEvent::Event(Event::Connected(_))
        ));
        // While connected, an idle stream is not a stall: it just waits.
        assert!(timeout(Duration::from_millis(200), watch.recv(&rx))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn watch_reports_a_disconnect_that_never_recovers() {
        let (tx, rx) = async_channel::unbounded();
        let mut watch = ConnectionWatch::new(Duration::from_millis(50));
        tx.send(connected()).await.unwrap();
        tx.send(conn_lost()).await.unwrap();
        assert!(matches!(watch.recv(&rx).await, WatchedEvent::Event(_)));
        assert!(matches!(
            watch.recv(&rx).await,
            WatchedEvent::Event(Event::Disconnected(_))
        ));
        match watch.recv(&rx).await {
            WatchedEvent::Stalled(elapsed) => assert!(elapsed >= Duration::from_millis(50)),
            other => panic!("expected Stalled, got {other:?}"),
        }
        drop(tx);
    }

    #[tokio::test]
    async fn watch_accepts_a_reconnect_within_the_grace_period() {
        let (tx, rx) = async_channel::unbounded();
        let mut watch = ConnectionWatch::new(Duration::from_millis(200));
        tx.send(conn_lost()).await.unwrap();
        assert!(matches!(watch.recv(&rx).await, WatchedEvent::Event(_)));
        tx.send(connected()).await.unwrap();
        assert!(matches!(
            watch.recv(&rx).await,
            WatchedEvent::Event(Event::Connected(_))
        ));
        // Back to connected: waiting past the old deadline is not a stall.
        assert!(timeout(Duration::from_millis(400), watch.recv(&rx))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn watch_does_not_extend_the_deadline_on_repeated_disconnects() {
        let (tx, rx) = async_channel::unbounded();
        let mut watch = ConnectionWatch::new(Duration::from_millis(150));
        tx.send(conn_lost()).await.unwrap();
        assert!(matches!(watch.recv(&rx).await, WatchedEvent::Event(_)));
        let tx2 = tx.clone();
        tokio::spawn(async move {
            for _ in 0..10 {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let _ = tx2.send(conn_lost()).await;
            }
        });
        let started = tokio::time::Instant::now();
        loop {
            match watch.recv(&rx).await {
                WatchedEvent::Event(_) => continue,
                WatchedEvent::Stalled(_) => break,
                WatchedEvent::Closed => panic!("unexpected close"),
            }
        }
        assert!(started.elapsed() < Duration::from_millis(400));
    }

    #[tokio::test]
    async fn watch_reports_a_closed_stream() {
        let (tx, rx) = async_channel::unbounded::<Event>();
        let mut watch = ConnectionWatch::new(Duration::from_millis(50));
        drop(tx);
        assert!(matches!(watch.recv(&rx).await, WatchedEvent::Closed));
    }

    #[test]
    fn restart_delay_backs_off_and_is_capped() {
        let mut delay = IOT_RESTART_MIN_DELAY;
        let mut seen = vec![];
        for _ in 0..10 {
            seen.push(delay.as_secs());
            delay = next_restart_delay(delay);
        }
        assert_eq!(seen, vec![5, 10, 20, 40, 80, 160, 300, 300, 300, 300]);
    }

    async fn connect_test_client(
        broker: &test_broker::Broker,
        id: &str,
    ) -> std::mem::ManuallyDrop<mosquitto_rs::Client> {
        // Never dropped: destroying a mosquitto client hangs on Windows GNU
        // builds.
        let client = std::mem::ManuallyDrop::new(mosquitto_rs::Client::with_id(id, true).unwrap());
        client
            .configure_tls(
                Some(&broker.ca_file),
                None::<&std::path::Path>,
                None::<&std::path::Path>,
                None::<&std::path::Path>,
                None,
            )
            .unwrap();
        timeout(
            Duration::from_secs(10),
            client.connect(
                "localhost",
                broker.port.into(),
                Duration::from_secs(60),
                None,
            ),
        )
        .await
        .expect("connect timed out")
        .expect("connect failed");
        client
    }

    /// Regression test for the IoT path silently dying.
    ///
    /// When the TLS connection is dropped without a close_notify, OpenSSL 3
    /// reports "unexpected eof while reading", libmosquitto treats the
    /// resulting EPROTO as fatal, and its loop thread exits without ever
    /// reconnecting. The ConnectionWatch must turn that into a Stalled
    /// result so that run_iot_supervisor builds a fresh client.
    #[tokio::test]
    async fn tls_eof_without_close_notify_is_detected_as_a_stall() {
        let broker = test_broker::start();
        let grace = Duration::from_secs(4);

        let client = connect_test_client(&broker, "govee2mqtt-test").await;
        let subscriptions = client.subscriber().unwrap();

        let mut watch = ConnectionWatch::new(grace);
        let mut saw_disconnect = false;
        let outcome = timeout(grace + Duration::from_secs(5), async {
            loop {
                match watch.recv(&subscriptions).await {
                    WatchedEvent::Event(Event::Disconnected(_)) => saw_disconnect = true,
                    WatchedEvent::Event(Event::Connected(_)) if saw_disconnect => {
                        break "reconnected"
                    }
                    WatchedEvent::Event(_) => {}
                    WatchedEvent::Stalled(_) => break "stalled",
                    WatchedEvent::Closed => break "closed",
                }
            }
        })
        .await
        .expect("neither reconnected nor reported a stall");

        assert!(saw_disconnect, "the broker's TCP drop was not observed");
        assert_eq!(
            outcome, "stalled",
            "libmosquitto is expected to give up after a TLS EOF; \
             if it now reconnects on its own this test needs revisiting"
        );
        assert_eq!(
            broker.connections.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "libmosquitto made no reconnect attempt"
        );

        // A freshly built client, which is what run_iot_supervisor does
        // next, connects fine to the same endpoint.
        connect_test_client(&broker, "govee2mqtt-test-2").await;

        let _ = std::fs::remove_dir_all(broker.ca_file.parent().unwrap());
    }
}

#[cfg(test)]
mod energy_test {
    use super::*;

    #[test]
    fn uptime_from_sta() {
        // Captured from an H5086, trimmed
        let packet: Packet = from_json(
            r#"{"sku":"H5086","device":"08:E3:98:17:3C:95:2F:EE","cmd":"status",
               "state":{"onOff":1,"sta":{"stc":"19_0_77_40_1"},"result":1}}"#,
        )
        .unwrap();
        assert_eq!(packet.state.uptime_seconds(), Some(40));
        assert!(!packet.state.answers_other_client());
        assert_eq!(packet.state.on_off, Some(1));

        // Other shapes don't break parsing the rest of the packet
        let packet: Packet =
            from_json(r#"{"sku":"H6008","device":"x","state":{"onOff":0,"sta":5}}"#).unwrap();
        assert_eq!(packet.state.uptime_seconds(), None);
        assert!(!packet.state.answers_other_client());
        assert_eq!(packet.state.on_off, Some(0));
    }

    #[test]
    fn reply_to_the_govee_app() {
        // Captured from an H5086 at 22:25:55, trimmed: on-time 354s and
        // 7.2 Wh, while it had been on for 80749s and used 772.1 Wh today
        let packet: Packet = from_json(
            r#"{"sku":"H5086","device":"06:20:98:17:3C:95:F7:9E","cmd":"status",
               "transaction":"a_1791944755350",
               "state":{"onOff":1,"sta":{"stc":"0_0_70_325070_1"},"result":1}}"#,
        )
        .unwrap();
        assert!(packet.state.answers_other_client());
        assert_eq!(packet.state.uptime_seconds(), Some(325070));
    }
}
