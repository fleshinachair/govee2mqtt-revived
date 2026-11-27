use crate::hass_mqtt::base::{Device, EntityConfig, Origin};
use crate::hass_mqtt::command_routes::{instantiate_route, AUTO_STOP_ROUTE, SWITCH_COMMAND_ROUTE};
use crate::hass_mqtt::instance::{publish_entity_config, EntityInstance};
use crate::platform_api::DeviceCapability;
use crate::service::device::Device as ServiceDevice;
use crate::service::hass::{
    availability_topic, camel_case_to_space_separated, switch_instance_state_topic, topic_safe_id,
    HassClient, IdParameter,
};
use crate::service::state::StateHandle;
use async_trait::async_trait;
use mosquitto_rs::router::{Params, Payload, State};
use serde::Serialize;
use serde_json::json;

#[derive(Serialize, Clone, Debug)]
pub struct SwitchConfig {
    #[serde(flatten)]
    pub base: EntityConfig,
    pub command_topic: crate::hass_mqtt::command_routes::CommandTopic,
    pub state_topic: String,
}

impl SwitchConfig {
    pub async fn for_device(
        device: &ServiceDevice,
        instance: &DeviceCapability,
    ) -> anyhow::Result<Self> {
        let id = topic_safe_id(device);
        let command_topic = instantiate_route(
            SWITCH_COMMAND_ROUTE,
            &[("id", &id), ("instance", &instance.instance)],
        )?;
        let state_topic = switch_instance_state_topic(device, &instance.instance);
        let availability_topic = availability_topic();
        let unique_id = format!("gv2mqtt-{id}-{inst}", inst = instance.instance);

        Ok(Self {
            base: EntityConfig {
                availability_topic,
                name: Some(camel_case_to_space_separated(&instance.instance)),
                device_class: None,
                origin: Origin::default(),
                device: Device::for_device(device),
                unique_id,
                entity_category: None,
                icon: None,
            },
            command_topic,
            state_topic,
        })
    }

    pub async fn publish(&self, state: &StateHandle, client: &HassClient) -> anyhow::Result<()> {
        publish_entity_config("switch", state, client, &self.base, self).await
    }
}

pub struct CapabilitySwitch {
    switch: SwitchConfig,
    device_id: String,
    state: StateHandle,
    instance_name: String,
}

impl CapabilitySwitch {
    pub async fn new(
        device: &ServiceDevice,
        state: &StateHandle,
        instance: &DeviceCapability,
    ) -> anyhow::Result<Self> {
        let switch = SwitchConfig::for_device(device, instance).await?;
        Ok(Self {
            switch,
            device_id: device.id.to_string(),
            state: state.clone(),
            instance_name: instance.instance.to_string(),
        })
    }

    /// Switch for the camera "video mode" of an Envisual/DreamView TV backlight.
    /// Govee's Platform API never advertises this as a capability, so we build a
    /// synthetic `videoMode` toggle; commands are routed to a raw-frame send in
    /// `mqtt_switch_command`. State is optimistic (Govee reports nothing for it).
    pub async fn video_mode(device: &ServiceDevice, state: &StateHandle) -> anyhow::Result<Self> {
        Self::new(
            device,
            state,
            &DeviceCapability::synthetic_toggle("videoMode"),
        )
        .await
    }
}

#[async_trait]
impl EntityInstance for CapabilitySwitch {
    async fn publish_config(&self, state: &StateHandle, client: &HassClient) -> anyhow::Result<()> {
        self.switch.publish(state, client).await
    }

    async fn notify_state(&self, client: &HassClient) -> anyhow::Result<()> {
        let Some(device) = self.state.device_by_id(&self.device_id).await else {
            log::warn!(
                "Device {} not found in state, skipping notify",
                self.device_id
            );
            return Ok(());
        };

        if self.instance_name == "powerSwitch" {
            if let Some(state) = device.device_state() {
                client
                    .publish(
                        &self.switch.state_topic,
                        if state.on { "ON" } else { "OFF" },
                    )
                    .await?;
            }
            return Ok(());
        }

        // TODO: currently, Govee don't return any meaningful data on
        // additional states. When they do, we'll need to start reporting
        // it here, but we'll also need to start polling it from the
        // platform API in order for it to even be available here.
        // Until then, the switch will show in the hass UI with an
        // unknown state but provide you with separate on and off push
        // buttons so that you can at least send the commands to the device.
        // <https://developer.govee.com/discuss/6596e84c901fb900312d5968>

        // Fixtures with independently switchable elements report both states
        // over IoT. That is the only place they are available: the platform
        // API returns an empty string for these two instances.
        if let Some(elements) = device.element_state {
            let on = match self.instance_name.as_str() {
                "mainLightToggle" => Some(elements.main),
                "backgroundLightToggle" => Some(elements.background),
                _ => None,
            };
            if let Some(on) = on {
                return client
                    .publish(&self.switch.state_topic, if on { "ON" } else { "OFF" })
                    .await;
            }
        }

        if let Some(cap) = device.get_state_capability_by_instance(&self.instance_name) {
            match cap.state.pointer("/value").and_then(|v| v.as_i64()) {
                Some(n) => {
                    return client
                        .publish(&self.switch.state_topic, if n != 0 { "ON" } else { "OFF" })
                        .await;
                }
                None => {
                    if cap.state.pointer("/value") == Some(&json!("")) {
                        log::trace!(
                            "CapabilitySwitch::notify_state ignore useless \
                                            empty string state for {cap:?}"
                        );
                    } else {
                        log::warn!("CapabilitySwitch::notify_state: Do something with {cap:#?}");
                    }
                    return Ok(());
                }
            }
        }
        log::trace!(
            "CapabilitySwitch::notify_state: didn't find state for {device} {instance}",
            instance = self.instance_name
        );
        Ok(())
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct AutoStopConfig {
    #[serde(flatten)]
    pub base: EntityConfig,
    pub command_topic: crate::hass_mqtt::command_routes::CommandTopic,
    pub state_topic: String,
    /// Has hass retain its commands, so that the broker tells us the
    /// setting again after we restart
    pub retain: bool,
}

fn auto_stop_state_topic(device: &ServiceDevice) -> String {
    format!("gv2mqtt/{id}/auto-stop/state", id = topic_safe_id(device))
}

fn on_off(on: bool) -> &'static str {
    if on {
        "ON"
    } else {
        "OFF"
    }
}

/// Chooses whether a heater stops at its target temperature, see AUTO_STOP_FIELD
pub struct AutoStopSwitch {
    switch: AutoStopConfig,
    device_id: String,
    state: StateHandle,
}

impl AutoStopSwitch {
    pub fn new(device: &ServiceDevice, state: &StateHandle) -> anyhow::Result<Self> {
        let id = topic_safe_id(device);
        Ok(Self {
            switch: AutoStopConfig {
                base: EntityConfig {
                    availability_topic: availability_topic(),
                    name: Some("Auto Stop".to_string()),
                    device_class: None,
                    origin: Origin::default(),
                    device: Device::for_device(device),
                    unique_id: format!("gv2mqtt-{id}-auto-stop"),
                    entity_category: Some("config".to_string()),
                    icon: Some("mdi:thermometer-off".to_string()),
                },
                command_topic: instantiate_route(AUTO_STOP_ROUTE, &[("id", &id)])?,
                state_topic: auto_stop_state_topic(device),
                retain: true,
            },
            device_id: device.id.to_string(),
            state: state.clone(),
        })
    }
}

#[async_trait]
impl EntityInstance for AutoStopSwitch {
    async fn publish_config(&self, state: &StateHandle, client: &HassClient) -> anyhow::Result<()> {
        publish_entity_config("switch", state, client, &self.switch.base, &self.switch).await
    }

    async fn notify_state(&self, client: &HassClient) -> anyhow::Result<()> {
        let Some(device) = self.state.device_by_id(&self.device_id).await else {
            log::warn!(
                "Device {} not found in state, skipping notify",
                self.device_id
            );
            return Ok(());
        };

        client
            .publish(&self.switch.state_topic, on_off(device.auto_stop))
            .await
    }
}

/// Remembers the choice. Nothing is sent to the device until its target
/// temperature is next set; this also runs when the broker replays the choice.
pub async fn mqtt_set_auto_stop(
    Payload(value): Payload<String>,
    Params(IdParameter { id }): Params<IdParameter>,
    State(state): State<StateHandle>,
) -> anyhow::Result<()> {
    let on = match value.as_str() {
        "ON" => true,
        "OFF" => false,
        // The retained choice was cleared
        "" => return Ok(()),
        other => anyhow::bail!("unexpected auto stop setting {other:?}"),
    };

    // A choice kept for a device we no longer have must not bring it back
    let Some(device) = state.resolve_device(&id).await else {
        return Ok(());
    };
    state.device_mut(&device.sku, &device.id).await.auto_stop = on;

    if let Some(client) = state.get_hass_client().await {
        client
            .publish(auto_stop_state_topic(&device), on_off(on))
            .await?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::platform_api::DeviceCapabilityKind;

    // H9999 is deliberately absent from the quirk table (see
    // hass_mqtt::enumerator's tests), so this fixture exercises only the
    // route-substitution logic, not quirk-derived behavior.
    const DEVICE_ID: &str = "AA:BB:CC:DD:EE:FF:11:22";
    const SKU: &str = "H9999";

    fn test_device() -> ServiceDevice {
        ServiceDevice::new(SKU, DEVICE_ID)
    }

    #[tokio::test]
    async fn auto_stop_remembers_the_choice() -> anyhow::Result<()> {
        let state: StateHandle = std::sync::Arc::new(crate::service::state::State::new());
        {
            let _device = state.device_mut(SKU, DEVICE_ID).await;
        }

        async fn send(state: &StateHandle, id: &str, value: &str) -> anyhow::Result<()> {
            mqtt_set_auto_stop(
                Payload(value.to_string()),
                Params(IdParameter { id: id.to_string() }),
                State(state.clone()),
            )
            .await
        }
        async fn stored(state: &StateHandle, id: &str) -> Option<bool> {
            state.device_by_id(id).await.map(|d| d.auto_stop)
        }

        assert_eq!(stored(&state, DEVICE_ID).await, Some(false));
        send(&state, DEVICE_ID, "ON").await?;
        assert_eq!(stored(&state, DEVICE_ID).await, Some(true));
        send(&state, DEVICE_ID, "").await?;
        assert_eq!(stored(&state, DEVICE_ID).await, Some(true));
        assert!(send(&state, DEVICE_ID, "maybe").await.is_err());
        send(&state, DEVICE_ID, "OFF").await?;
        assert_eq!(stored(&state, DEVICE_ID).await, Some(false));

        let unknown = "11:22:33:44:55:66:77:88";
        send(&state, unknown, "ON").await?;
        assert_eq!(stored(&state, unknown).await, None);
        Ok(())
    }

    fn power_switch_capability() -> DeviceCapability {
        DeviceCapability {
            kind: DeviceCapabilityKind::Toggle,
            instance: "powerSwitch".to_string(),
            parameters: None,
            alarm_type: None,
            event_state: None,
        }
    }

    /// `hass.rs` only subscribes to `SWITCH_COMMAND_ROUTE`. If the segment
    /// order or param names here ever drift from that registered pattern,
    /// Home Assistant renders a working-looking switch that writes to a
    /// topic the bridge's router never reads.
    #[tokio::test]
    async fn for_device_command_topic_matches_the_switch_route() {
        let device = test_device();
        let instance = power_switch_capability();
        let switch = SwitchConfig::for_device(&device, &instance)
            .await
            .expect("a well-formed capability must build a switch");

        let id = topic_safe_id(&device);
        let expected = format!(
            "gv2mqtt/switch/{id}/command/{inst}",
            inst = instance.instance
        );
        assert_eq!(switch.command_topic.as_str(), expected);
    }

    /// A malformed Platform API response (empty instance name) must fail
    /// loud through `instantiate_route`'s `ensure!` rather than silently
    /// advertise `.../command/` with a missing trailing segment that no
    /// registered route pattern matches.
    #[tokio::test]
    async fn for_device_rejects_an_empty_instance_name() {
        let device = test_device();
        let mut instance = power_switch_capability();
        instance.instance = String::new();

        let error = SwitchConfig::for_device(&device, &instance)
            .await
            .expect_err("an empty instance name must not be advertised");
        assert!(
            error.to_string().contains("empty parameter 'instance'"),
            "unexpected error: {error:#}"
        );
    }
}
