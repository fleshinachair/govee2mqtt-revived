use crate::ble::{
    NotifyElementState, NotifyHumidifierNightlightParams, NotifyKettleBaseStatus,
    NotifyKettleHeatingState, NotifyKettleKeepWarm,
};
use crate::commands::serve::POLL_INTERVAL;
use crate::lan_api::{DeviceColor, DeviceStatus as LanDeviceStatus, LanDevice};
use crate::platform_api::{
    DeviceCapability, DeviceCapabilityState, DeviceType, HttpDeviceInfo, HttpDeviceState,
};
use crate::service::quirks::{resolve_quirk, Quirk, BULB, DEFAULT_COLOR_TEMP_RANGE};
use crate::service::state::SceneCatalogCache;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::net::IpAddr;

#[derive(Default, Clone, Debug)]
pub struct Device {
    pub sku: String,
    pub id: String,

    /// Probed LAN device information, found either via discovery
    /// or explicit probing by IP address
    pub lan_device: Option<LanDevice>,
    pub last_lan_device_update: Option<DateTime<Utc>>,

    pub lan_device_status: Option<LanDeviceStatus>,
    pub last_lan_device_status_update: Option<DateTime<Utc>>,

    pub http_device_info: Option<HttpDeviceInfo>,
    pub last_http_device_update: Option<DateTime<Utc>>,

    pub http_device_state: Option<HttpDeviceState>,
    pub last_http_device_state_update: Option<DateTime<Utc>>,

    pub undoc_device_info: Option<UndocDeviceInfo>,
    pub last_undoc_device_info_update: Option<DateTime<Utc>>,

    pub iot_device_status: Option<LanDeviceStatus>,
    pub last_iot_device_status_update: Option<DateTime<Utc>>,

    /// When an IoT packet last carried an explicit `state.mode`.
    /// `last_iot_device_status_update` is re-stamped by every IoT packet,
    /// including mode-less ones whose merge carries the cached mode
    /// forward, so it cannot serve as the mode observation time.
    /// Stamped by the IoT subscriber merge (see service/iot.rs).
    pub last_iot_mode_update: Option<DateTime<Utc>>,

    pub nightlight_state: Option<NotifyHumidifierNightlightParams>,

    /// Last reported main-panel / halo-ring state, for fixtures that
    /// have independently switchable elements
    pub element_state: Option<NotifyElementState>,
    pub target_humidity_percent: Option<u8>,
    pub humidifier_work_mode: Option<u8>,
    pub humidifier_param_by_mode: HashMap<u8, u8>,
    pub kettle_base_status: Option<NotifyKettleBaseStatus>,
    /// When the kettle was last seen going from off its base to on it.
    /// Only known once that transition has been observed while running.
    pub kettle_placed_on_base: Option<DateTime<Utc>>,
    pub kettle_heating_state: Option<NotifyKettleHeatingState>,
    pub kettle_keep_warm: Option<NotifyKettleKeepWarm>,

    pub last_polled: Option<DateTime<Utc>>,

    /// The scene the bridge last applied to this device. Govee never reports the
    /// live scene, so this is the bridge's own record. Cleared only when the bridge
    /// itself sets a solid color/temperature (see the command paths in state.rs).
    active_scene: Option<String>,
    /// Cached scene catalog to avoid repeated API calls during state notifications
    scene_catalog_cache: Option<SceneCatalogCache>,

    /// Sensitivity applied the next time a `Music:` effect is selected.
    ///
    /// This is a stored preference, not device state. Govee never reports the
    /// live value: the Platform API answers `""` for `musicMode` on lights, and
    /// the `aa 05 13` BLE read-back only tracks values written over BLE or LAN,
    /// not ones set through `music_setting` (measured on H607C, see #39).
    ///
    /// It cannot be pushed on its own either. `musicMode` is `required: true`
    /// in the capability struct, so Govee rejects a sensitivity-only call with
    /// 400 `Missing parameter`, and including the style would switch the light
    /// into music mode and power it on — an unacceptable side effect of moving
    /// a slider. So the value is held here and applied on the next style change.
    music_sensitivity: Option<u8>,
}

impl std::fmt::Display for Device {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{} ({} {})", self.name(), self.id, self.sku)
    }
}

/// Represents the device state; synthesized from the various
/// sources of facts that we have in the Device
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceState {
    /// Whether the device is powered on
    pub on: bool,
    /// Whether the light function of the device is powered on
    pub light_on: Option<bool>,

    /// Whether the device is connected to the Govee cloud
    pub online: Option<bool>,

    /// The color temperature in kelvin
    pub kelvin: u32,

    /// The color
    pub color: crate::lan_api::DeviceColor,

    /// The brightness in percent (0-100)
    pub brightness: u8,

    /// The active effect mode, if known
    pub scene: Option<String>,

    /// The active work mode number as reported by the device via
    /// the AWS IoT status message, when known. SKU-specific meaning
    /// (eg: H607C reports 5 for manual color and 4 for music mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<i64>,

    /// When `mode` was learned from the AWS IoT status message. The LAN and
    /// Platform API projections carry the last IoT mode forward, so their
    /// `updated` stamp says nothing about the mode's age; this field keeps
    /// the original observation time so consumers can judge staleness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_updated: Option<DateTime<Utc>>,

    /// Where the information came from
    pub source: &'static str,
    pub updated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UndocDeviceInfo {
    pub room_name: Option<String>,
    pub entry: crate::undoc_api::DeviceEntry,
}

impl Device {
    /// Create a new device given just its sku and id.
    /// No other facts are known or reflected by it at this time;
    /// they will need to be added by the caller.
    pub fn new<S: Into<String>, I: Into<String>>(sku: S, id: I) -> Self {
        Self {
            sku: sku.into(),
            id: id.into(),
            ..Self::default()
        }
    }

    /// Returns the device name; either the name defined in the Govee App,
    /// or, if we don't have the information for some reason, then we compute
    /// a name from the SKU and the last couple of bytes from the device id,
    /// similar to the device name that would show up in a BLE scan, or
    /// the default name for the device if not otherwise configured in the
    /// Govee App.
    pub fn name(&self) -> String {
        if let Some(name) = self.govee_name() {
            return name.to_string();
        }
        self.computed_name()
    }

    /// Returns the name defined for the device in the Govee App
    pub fn govee_name(&self) -> Option<&str> {
        if let Some(info) = &self.http_device_info {
            return Some(&info.device_name);
        }
        None
    }

    pub fn room_name(&self) -> Option<&str> {
        if let Some(info) = &self.undoc_device_info {
            return info.room_name.as_deref();
        }
        None
    }

    /// compute a name from the SKU and the last couple of bytes from the
    /// device id, similar to the device name that would show up in a BLE
    /// scan, or the default name for the device if not otherwise configured
    /// in the Govee App.
    pub fn computed_name(&self) -> String {
        // The id is usually "XX:XX:XX:XX:XX:XX:XX:XX" but some devices
        // report it without colons, and in lowercase.  Normalize it.
        let mut id = String::new();
        for c in self.id.chars() {
            if c == ':' {
                continue;
            }
            id.push(c.to_ascii_uppercase());
        }

        format!("{}_{}", self.sku, &id[id.len().saturating_sub(4)..])
    }

    pub fn preferred_poll_interval(&self) -> chrono::Duration {
        match self.device_type() {
            // If the kettle is on, read its temperature more frequently
            DeviceType::Kettle => {
                if self.device_state().map(|s| s.on).unwrap_or(false) {
                    chrono::Duration::seconds(60)
                } else {
                    *POLL_INTERVAL
                }
            }
            _ => *POLL_INTERVAL,
        }
    }

    pub fn ip_addr(&self) -> Option<IpAddr> {
        self.lan_device.as_ref().map(|device| device.ip)
    }

    pub fn set_last_polled(&mut self) {
        self.last_polled.replace(Utc::now());
    }

    pub fn set_element_state(&mut self, params: NotifyElementState) {
        self.element_state.replace(params);
    }

    pub fn set_nightlight_state(&mut self, params: NotifyHumidifierNightlightParams) {
        self.nightlight_state.replace(params);
    }

    pub fn set_target_humidity(&mut self, percent: u8) {
        self.target_humidity_percent.replace(percent);
    }

    pub fn set_kettle_base_status(&mut self, status: NotifyKettleBaseStatus) {
        let was_off_base = self
            .kettle_base_status
            .map(|prior| !prior.is_on_base())
            .unwrap_or(false);
        if was_off_base && status.is_on_base() {
            self.kettle_placed_on_base.replace(Utc::now());
        }
        self.kettle_base_status.replace(status);
    }

    pub fn set_kettle_heating_state(&mut self, state: NotifyKettleHeatingState) {
        self.kettle_heating_state.replace(state);
    }

    pub fn set_kettle_keep_warm(&mut self, keep_warm: NotifyKettleKeepWarm) {
        self.kettle_keep_warm.replace(keep_warm);
    }

    /// Minutes of keep-warm left, while the kettle is keeping warm.
    pub fn kettle_keep_warm_remaining(&self) -> Option<u8> {
        if !self.kettle_heating_state?.is_keeping_warm() {
            return None;
        }
        Some(self.kettle_keep_warm?.remaining_minutes)
    }

    pub fn set_humidifier_work_mode_and_param(&mut self, mode: u8, param: u8) {
        self.humidifier_work_mode.replace(mode);
        self.humidifier_param_by_mode.insert(mode, param);
    }

    /// Update the LAN device information
    pub fn set_lan_device(&mut self, device: LanDevice) {
        self.lan_device.replace(device);
        self.last_lan_device_update.replace(Utc::now());
    }

    /// Update the LAN device status information
    pub fn set_lan_device_status(&mut self, status: LanDeviceStatus) -> bool {
        let changed = self
            .lan_device_status
            .as_ref()
            .map(|prior| *prior != status)
            .unwrap_or(true);
        self.lan_device_status.replace(status);
        self.last_lan_device_status_update.replace(Utc::now());
        self.clear_scene_if_light_powered_off(self.compute_lan_device_state());
        changed
    }

    pub fn set_iot_device_status(&mut self, status: LanDeviceStatus) {
        self.iot_device_status.replace(status);
        self.last_iot_device_status_update.replace(Utc::now());
        self.clear_scene_if_light_powered_off(self.compute_iot_device_state());
    }

    pub fn set_http_device_info(&mut self, info: HttpDeviceInfo) {
        self.http_device_info.replace(info);
        self.last_http_device_update.replace(Utc::now());
    }

    pub fn set_http_device_state(&mut self, state: HttpDeviceState) {
        self.http_device_state.replace(state);
        self.last_http_device_state_update.replace(Utc::now());
        self.clear_scene_if_light_powered_off(self.compute_http_device_state());
    }

    pub fn set_undoc_device_info(
        &mut self,
        entry: crate::undoc_api::DeviceEntry,
        room_name: Option<&str>,
    ) {
        self.undoc_device_info.replace(UndocDeviceInfo {
            entry,
            room_name: room_name.map(|s| s.to_string()),
        });
        self.last_undoc_device_info_update.replace(Utc::now());
    }

    pub fn compute_iot_device_state(&self) -> Option<DeviceState> {
        let updated = self.last_iot_device_status_update?;
        let status = self.iot_device_status.as_ref()?;

        Some(DeviceState {
            on: status.on,
            light_on: if self.device_type() == DeviceType::Light {
                Some(status.on)
            } else {
                self.nightlight_state.as_ref().map(|s| s.on)
            },
            online: None,
            brightness: status.brightness,
            color: status.color,
            kelvin: status.color_temperature_kelvin,
            scene: self.active_scene.clone(),
            mode: status.mode,
            mode_updated: status.mode.and(self.last_iot_mode_update),
            source: "AWS IoT API",
            updated,
        })
    }

    pub fn compute_lan_device_state(&self) -> Option<DeviceState> {
        let updated = self.last_lan_device_status_update?;
        let status = self.lan_device_status.as_ref()?;

        // The LAN devStatus response doesn't carry a mode field; carry over
        // the last mode learned via AWS IoT, if any. Keep the original IoT
        // observation time in `mode_updated`: this projection's `updated` is
        // the LAN poll time, which would present an hours-old mode as fresh.
        let (mode, mode_updated) = match status.mode {
            Some(mode) => (Some(mode), Some(updated)),
            None => match self.iot_device_status.as_ref().and_then(|s| s.mode) {
                Some(mode) => (Some(mode), self.last_iot_mode_update),
                None => (None, None),
            },
        };

        Some(DeviceState {
            on: status.on,
            light_on: Some(status.on), // assumption: LAN API == light
            online: None,
            brightness: status.brightness,
            color: status.color,
            kelvin: status.color_temperature_kelvin,
            scene: self.active_scene.clone(),
            mode,
            mode_updated,
            source: "LAN API",
            updated,
        })
    }

    pub fn compute_http_device_state(&self) -> Option<DeviceState> {
        let updated = self.last_http_device_state_update?;
        let state = self.http_device_state.as_ref()?;

        let mut online = None;
        let mut on = false;
        let mut light_on = None;
        let mut brightness = 0;
        let mut color = DeviceColor::default();
        let mut kelvin = 0;

        #[derive(serde::Deserialize)]
        struct IntegerValueState {
            value: u32,
        }
        #[derive(serde::Deserialize)]
        struct BoolValueState {
            value: bool,
        }

        let light_instance = self.get_light_power_toggle_instance_name();

        for cap in &state.capabilities {
            if let Ok(value) = serde_json::from_value::<IntegerValueState>(cap.state.clone()) {
                if light_instance
                    .map(|inst| inst == cap.instance.as_str())
                    .unwrap_or(false)
                {
                    light_on.replace(value.value != 0);
                }

                match cap.instance.as_str() {
                    "powerSwitch" => {
                        on = value.value != 0;
                    }
                    "colorRgb" => {
                        color = DeviceColor {
                            r: ((value.value >> 16) & 0xff) as u8,
                            g: ((value.value >> 8) & 0xff) as u8,
                            b: (value.value & 0xff) as u8,
                        };
                    }
                    "brightness" => {
                        brightness = value.value as u8;
                    }
                    "colorTemperatureK" => {
                        kelvin = value.value;
                    }
                    _ => {}
                }
            } else if cap.instance == "online" {
                if let Ok(value) = serde_json::from_value::<BoolValueState>(cap.state.clone()) {
                    online.replace(value.value);
                }
            }
        }

        // The Platform API doesn't report a work mode for lights; carry over
        // the last mode learned via AWS IoT, if any, keeping the original IoT
        // observation time (see compute_lan_device_state).
        let mode = self.iot_device_status.as_ref().and_then(|s| s.mode);
        let mode_updated = mode.and(self.last_iot_mode_update);

        Some(DeviceState {
            on,
            light_on,
            online,
            brightness,
            color,
            kelvin,
            scene: self.active_scene.clone(),
            mode,
            mode_updated,
            source: "PLATFORM API",
            updated,
        })
    }

    /// Returns the most recently received state information
    pub fn device_state(&self) -> Option<DeviceState> {
        let mut candidates = vec![];

        if let Some(state) = self.compute_lan_device_state() {
            candidates.push(state);
        }
        if let Some(state) = self.compute_http_device_state() {
            candidates.push(state);
        }
        if let Some(state) = self.compute_iot_device_state() {
            candidates.push(state);
        }

        candidates.sort_by_key(|a| a.updated);

        candidates.pop()
    }

    /// Returns the active scene name, if any
    pub fn active_scene_name(&self) -> Option<&str> {
        self.active_scene.as_deref()
    }

    /// Returns the full cached scene catalog metadata, if available
    pub fn scene_catalog_cache(&self) -> Option<&SceneCatalogCache> {
        self.scene_catalog_cache.as_ref()
    }

    /// Caches the scene catalog for this device
    pub fn set_scene_catalog(&mut self, catalog: SceneCatalogCache) {
        self.scene_catalog_cache = Some(catalog);
    }

    /// Drops the cached scene catalog so the next request refetches it. Used by the
    /// MQTT cache-purge button, which otherwise only clears the on-disk cache.
    pub fn clear_scene_catalog(&mut self) {
        self.scene_catalog_cache = None;
    }

    /// Records the active scene name the bridge just applied, or clears it.
    /// We do NOT clear based on observed color changes: Govee scenes animate their
    /// colors, so a poll-time color diff is not a reliable "scene ended" signal and
    /// caused the bridge to forget scenes it had correctly set. The scene is cleared
    /// by the explicit command paths that move the light to a solid color/temperature,
    /// and by `clear_scene_if_light_powered_off` when the device reports the light off.
    pub fn set_active_scene(&mut self, scene: Option<&str>) {
        self.active_scene = scene.map(|s| s.to_string());
    }

    /// Sensitivity to apply on the next `Music:` effect selection. The fallback
    /// is the Platform API's own default, so an untouched install sends exactly
    /// what it sent before the preference existed.
    pub fn music_sensitivity(&self) -> u8 {
        self.music_sensitivity
            .unwrap_or(crate::platform_api::DEFAULT_MUSIC_SENSITIVITY)
    }

    /// The user's stored preference, if one has been chosen. Keeping the raw
    /// `Option` separate from [`Self::music_sensitivity`] lets Home Assistant
    /// report "unknown" instead of presenting the fallback as observed state.
    pub fn music_sensitivity_value(&self) -> Option<u8> {
        self.music_sensitivity
    }

    /// Store the sensitivity for the next `Music:` effect selection. Values are
    /// clamped rather than rejected: Govee's own range is 0-100 and HA can send
    /// anything the user types into the number box.
    pub fn set_music_sensitivity(&mut self, value: u8) {
        self.music_sensitivity = Some(value.min(100));
    }

    /// Forget the stored preference, returning the entity to "unknown" and the
    /// next `Music:` effect to the historical default. Driven by the Clear
    /// Music Sensitivity button; Home Assistant cannot reach "unset" through a
    /// number entity, because it reads `payload_reset` on the state topic and
    /// never publishes it to the command topic.
    pub fn clear_music_sensitivity(&mut self) {
        self.music_sensitivity = None;
    }

    /// Clears the remembered scene if the device reports the light powered off.
    /// "Off" is an unambiguous, non-animating signal that no scene is playing, so it
    /// recovers the common case where the light is turned off outside the bridge
    /// (Govee app, physical button, another integration) without the animation
    /// false-positives the old color-diff check produced. Color changes while the
    /// light stays on remain "last applied by the bridge" until a bridge command
    /// moves it off the scene.
    fn clear_scene_if_light_powered_off(&mut self, source_state: Option<DeviceState>) {
        let is_light_off = source_state.map(|s| s.light_on.unwrap_or(s.on)) == Some(false);
        if self.active_scene.is_some() && is_light_off {
            self.active_scene.take();
        }
    }

    pub fn device_type(&self) -> DeviceType {
        if let Some(info) = &self.http_device_info {
            info.device_type.clone()
        } else if let Some(q) = resolve_quirk(&self.sku) {
            q.device_type.clone()
        } else {
            DeviceType::Light
        }
    }

    /// Indicate whether we require the platform API data in order
    /// to correctly report the device
    pub fn needs_platform_poll(&self) -> bool {
        if !self.iot_api_supported() {
            return true;
        }

        let device_type = self.device_type();
        match (device_type, self.sku.as_str()) {
            (_, "H7160") => false,
            (DeviceType::Humidifier, _) => true,
            (DeviceType::Light, _) => false,
            (DeviceType::Kettle, _) => true,
            _ => true,
        }
    }

    pub fn pollable_via_lan(&self) -> bool {
        self.lan_device.is_some()
    }

    pub fn pollable_via_iot(&self) -> bool {
        if !self.iot_api_supported() {
            return false;
        }
        let device_type = self.device_type();
        matches!(
            (device_type, self.sku.as_str()),
            (_, "H7160") | (DeviceType::Light, _)
        )
    }

    pub fn avoid_platform_api(&self) -> bool {
        if let Some(quirk) = self.resolve_quirk() {
            if quirk.avoid_platform_api {
                return true;
            }
            if self.lan_device.is_some()
                && !self
                    .http_device_info
                    .as_ref()
                    .map(|info| info.supports_rgb())
                    .unwrap_or(false)
            {
                // Conflicting information:
                // Platform API says that this device isn't
                // a light, but the LAN API support suggests
                // that it is a light!
                // Therefore we will not trust the Platform API
                return true;
            }
        }
        false
    }

    pub fn resolve_quirk(&self) -> Option<Quirk> {
        match resolve_quirk(&self.sku) {
            Some(q) => Some(q.clone()),
            None => {
                // It's an unknown device, but since it showed up via LAN disco,
                // we can assume that it is a light
                if self.lan_device.is_some() {
                    Some(Quirk::light(Cow::Owned(self.sku.to_string()), BULB).with_lan_api())
                } else {
                    None
                }
            }
        }
    }

    pub fn get_capability_by_instance(&self, instance: &str) -> Option<&DeviceCapability> {
        self.http_device_info
            .as_ref()
            .and_then(|info| info.capability_by_instance(instance))
    }

    pub fn get_state_capability_by_instance(
        &self,
        instance: &str,
    ) -> Option<&DeviceCapabilityState> {
        self.http_device_state
            .as_ref()
            .and_then(|info| info.capability_by_instance(instance))
    }

    pub fn get_light_power_toggle_instance_name(&self) -> Option<&'static str> {
        match self.device_type() {
            DeviceType::Light => Some("powerSwitch"),
            _ => {
                // If the device's primary function is not a light,
                // then we need to avoid powering on its other function
                // here.  If it has a nightlight capability, that is
                // probably what we are controlling.
                // We may need to expand this to other power toggles
                // in the future.
                if self
                    .get_capability_by_instance("nightlightToggle")
                    .is_some()
                {
                    Some("nightlightToggle")
                } else {
                    None
                }
            }
        }
    }

    pub fn get_color_temperature_range(&self) -> Option<(u32, u32)> {
        let quirk = self.resolve_quirk();

        // The range reported by the Platform API reflects what the
        // device actually supports, unless the quirk tells us that the
        // platform metadata for this SKU is unreliable.
        let platform_range = if quirk.as_ref().is_some_and(|q| q.avoid_platform_api) {
            None
        } else {
            self.http_device_info
                .as_ref()
                .and_then(|info| info.get_color_temperature_range())
                // An empty or inverted range is not usable
                .filter(|(min, max)| *min > 0 && min < max)
        };

        if let Some(quirk) = quirk {
            return match quirk.color_temp_range {
                None => None,
                // The generic default is just a guess; prefer the real
                // range when Govee tells us what it is. Sending a value
                // outside the supported range can leave some devices
                // (eg: H60C1) on but emitting no light.
                Some(range) if range == DEFAULT_COLOR_TEMP_RANGE => platform_range.or(Some(range)),
                // An explicit per-SKU range always wins
                Some(range) => Some(range),
            };
        }

        if self.lan_device.is_some() {
            // LAN API support suggests that it is a light
            return platform_range.or(Some(DEFAULT_COLOR_TEMP_RANGE));
        }

        platform_range
    }

    pub fn supports_brightness(&self) -> bool {
        if let Some(quirk) = self.resolve_quirk() {
            return quirk.supports_brightness;
        }

        if self.lan_device.is_some() {
            // LAN API support suggests that it is a light
            return true;
        }

        self.http_device_info
            .as_ref()
            .map(|info| info.supports_brightness())
            .unwrap_or(false)
    }

    pub fn iot_api_supported(&self) -> bool {
        if let Some(quirk) = self.resolve_quirk() {
            return quirk.iot_api_supported;
        }

        false
    }

    pub fn supports_rgb(&self) -> bool {
        if let Some(quirk) = self.resolve_quirk() {
            return quirk.supports_rgb;
        }

        if self.lan_device.is_some() {
            // LAN API support suggests that it is a light
            return true;
        }

        self.http_device_info
            .as_ref()
            .map(|info| info.supports_rgb())
            .unwrap_or(false)
    }

    pub fn is_ble_only_device(&self) -> Option<bool> {
        if let Some(quirk) = self.resolve_quirk() {
            return Some(quirk.ble_only);
        }

        if self.http_device_info.is_some() {
            // truly BLE-only devices are not returned via the Platform API,
            // unless we have a quirk to say otherwise
            return Some(false);
        }

        self.undoc_device_info
            .as_ref()
            .map(|info| info.entry.device_ext.device_settings.wifi_name.is_none())
    }

    pub fn is_controllable(&self) -> bool {
        !matches!(self.is_ble_only_device(), Some(true))
    }
}

#[cfg(test)]
mod test {
    use crate::ble::{NotifyKettleBaseStatus, NotifyKettleHeatingState, NotifyKettleKeepWarm};

    #[test]
    fn kettle_keep_warm_remaining() {
        let mut device = Device::new("H7170", "AA:BB");
        let state = |state| NotifyKettleHeatingState { state };
        let countdown = |remaining_minutes| NotifyKettleKeepWarm {
            enabled: 1,
            total_minutes: 120,
            remaining_minutes,
        };

        assert_eq!(device.kettle_keep_warm_remaining(), None);
        device.set_kettle_heating_state(state(1));
        device.set_kettle_keep_warm(countdown(120));
        assert_eq!(device.kettle_keep_warm_remaining(), None);
        device.set_kettle_heating_state(state(2));
        device.set_kettle_keep_warm(countdown(97));
        assert_eq!(device.kettle_keep_warm_remaining(), Some(97));
        device.set_kettle_heating_state(state(0));
        assert_eq!(device.kettle_keep_warm_remaining(), None);
    }

    #[test]
    fn kettle_placed_on_base() {
        let mut device = Device::new("H7170", "AA:BB");
        let status = |status| NotifyKettleBaseStatus { status };

        // First report on the base: we did not see it being put down
        device.set_kettle_base_status(status(0));
        assert!(device.kettle_placed_on_base.is_none());

        // Lifted, then put back
        device.set_kettle_base_status(status(2));
        assert!(device.kettle_placed_on_base.is_none());
        device.set_kettle_base_status(status(0));
        let placed = device.kettle_placed_on_base.expect("placed");

        // Repeated on-base and out-of-water reports keep the time
        device.set_kettle_base_status(status(0));
        device.set_kettle_base_status(status(1));
        assert_eq!(device.kettle_placed_on_base, Some(placed));
    }

    use super::*;

    #[test]
    fn name_compute() {
        let device = Device::new("H6000", "AA:BB:CC:DD:EE:FF:42:2A");
        assert_eq!(device.name(), "H6000_422A");

        let device = Device::new("H6127", "cef142b0b354995f");
        assert_eq!(device.name(), "H6127_995F");

        let device = Device::new("H6127", "ce");
        assert_eq!(device.name(), "H6127_CE");
    }

    #[test]
    fn active_scene_round_trips() {
        let mut device = Device::new("H6000", "AA:BB:CC:DD:EE:FF:42:2A");
        assert_eq!(device.active_scene_name(), None);
        device.set_active_scene(Some("Aurora"));
        assert_eq!(device.active_scene_name(), Some("Aurora"));
        device.set_active_scene(None);
        assert_eq!(device.active_scene_name(), None);
    }

    /// Regression guard for the self-clear-on-animation bug: a status poll reporting a
    /// different color (every animated Govee scene does this) must NOT make the bridge
    /// forget a scene it set itself. Clearing only happens on explicit command paths.
    #[test]
    fn status_poll_does_not_clear_active_scene() {
        let animated_frame = LanDeviceStatus {
            on: true,
            brightness: 100,
            color: DeviceColor { r: 1, g: 2, b: 3 },
            color_temperature_kelvin: 0,
            mode: None,
        };

        // LAN poll
        let mut device = Device::new("H6000", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_active_scene(Some("Aurora"));
        device.set_lan_device_status(animated_frame.clone());
        assert_eq!(device.active_scene_name(), Some("Aurora"));

        // IoT poll (primary path for animated-scene devices)
        let mut device = Device::new("H6000", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_active_scene(Some("Aurora"));
        device.set_iot_device_status(animated_frame);
        assert_eq!(device.active_scene_name(), Some("Aurora"));
    }

    /// A poll reporting the light OFF unambiguously means no scene is playing, so it
    /// must clear the remembered scene (recovers "turned off in the Govee app").
    #[test]
    fn powered_off_poll_clears_active_scene() {
        let off_frame = LanDeviceStatus {
            on: false,
            brightness: 0,
            color: DeviceColor { r: 0, g: 0, b: 0 },
            color_temperature_kelvin: 0,
            mode: None,
        };

        // LAN poll
        let mut device = Device::new("H6000", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_active_scene(Some("Aurora"));
        device.set_lan_device_status(off_frame.clone());
        assert_eq!(device.active_scene_name(), None);

        // IoT poll
        let mut device = Device::new("H6000", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_active_scene(Some("Aurora"));
        device.set_iot_device_status(off_frame);
        assert_eq!(device.active_scene_name(), None);
    }

    #[test]
    fn powered_off_poll_clears_active_scene_even_with_newer_cached_source() {
        let on_frame = LanDeviceStatus {
            on: true,
            brightness: 100,
            color: DeviceColor { r: 1, g: 2, b: 3 },
            color_temperature_kelvin: 0,
            mode: None,
        };
        let off_frame = LanDeviceStatus {
            on: false,
            brightness: 0,
            color: DeviceColor { r: 0, g: 0, b: 0 },
            color_temperature_kelvin: 0,
            mode: None,
        };

        let mut device = Device::new("H6000", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_lan_device_status(on_frame);
        device.last_lan_device_status_update = Some(Utc::now() + chrono::Duration::seconds(5));
        device.set_active_scene(Some("Aurora"));

        device.set_iot_device_status(off_frame);

        assert_eq!(device.active_scene_name(), None);
    }

    fn status_with_mode(mode: Option<i64>) -> LanDeviceStatus {
        LanDeviceStatus {
            on: true,
            brightness: 100,
            color: DeviceColor { r: 255, g: 0, b: 0 },
            color_temperature_kelvin: 0,
            mode,
        }
    }

    /// Regression guard for the mode field itself: a `mode` learned from an
    /// AWS IoT status must survive into the synthesized `DeviceState`,
    /// stamped with the IoT observation time.
    #[test]
    fn iot_status_mode_reaches_device_state() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        // Mirror the subscriber merge: the status cache and the mode
        // observation are stamped together when the packet carries a mode.
        device.set_iot_device_status(status_with_mode(Some(5)));
        device.last_iot_mode_update = device.last_iot_device_status_update;

        let state = device.device_state().expect("iot state");
        assert_eq!(state.mode, Some(5));
        assert!(state.mode_updated.is_some());
        assert_eq!(state.mode_updated, device.last_iot_mode_update);
    }

    /// The LAN devStatus response has no mode field; the projection carries
    /// the last IoT-learned mode forward even when the LAN state is newer
    /// and wins the source race.
    #[test]
    fn iot_mode_carries_over_into_newer_lan_projection() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_iot_device_status(status_with_mode(Some(4)));
        device.set_lan_device_status(status_with_mode(None));
        device.last_lan_device_status_update = Some(Utc::now() + chrono::Duration::seconds(5));

        let state = device.device_state().expect("lan state");
        assert_eq!(state.source, "LAN API");
        assert_eq!(state.mode, Some(4));
    }

    /// The carry-over must not launder the mode's age. Observed live
    /// (2026-08-13, H607C): a LAN projection re-stamped a 10-hour-old IoT
    /// mode with a seconds-old `updated`. `mode_updated` has to keep the
    /// original IoT observation time so consumers can judge staleness.
    #[test]
    fn lan_carry_over_preserves_iot_mode_observation_time() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_iot_device_status(status_with_mode(Some(4)));
        let aged = Utc::now() - chrono::Duration::hours(10);
        device.last_iot_mode_update = Some(aged);

        device.set_lan_device_status(status_with_mode(None));

        let state = device.device_state().expect("lan state");
        assert_eq!(state.source, "LAN API");
        assert_eq!(state.mode, Some(4));
        assert_eq!(state.mode_updated, Some(aged));
        assert!(state.updated > aged + chrono::Duration::hours(9));
    }

    /// Same guarantee for the Platform API projection, which reports no work
    /// mode for lights either.
    #[test]
    fn platform_projection_preserves_iot_mode_observation_time() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_iot_device_status(status_with_mode(Some(4)));
        let aged = Utc::now() - chrono::Duration::hours(10);
        device.last_iot_mode_update = Some(aged);

        device.set_http_device_state(HttpDeviceState {
            sku: "H607C".to_string(),
            device: "AA:BB:CC:DD:EE:FF:42:2A".to_string(),
            capabilities: vec![],
        });

        let state = device.compute_http_device_state().expect("http state");
        assert_eq!(state.mode, Some(4));
        assert_eq!(state.mode_updated, Some(aged));
    }

    /// Without any IoT report the projections must not invent a mode or an
    /// observation time.
    #[test]
    fn no_iot_report_means_no_mode_and_no_timestamp() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:42:2A");
        device.set_lan_device_status(status_with_mode(None));

        let state = device.device_state().expect("lan state");
        assert_eq!(state.mode, None);
        assert_eq!(state.mode_updated, None);
    }

    #[test]
    fn music_sensitivity_defaults_without_asserting_device_state() {
        let device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:11:22");

        // Unset reads as the historical constant, so an untouched install
        // sends exactly what it sent before the preference existed.
        assert_eq!(device.music_sensitivity(), 100);
        assert_eq!(
            device.music_sensitivity(),
            crate::platform_api::DEFAULT_MUSIC_SENSITIVITY,
            "the fallback must stay tied to the Platform API default"
        );

        // ...but we must not claim to have observed it. The entity keys its
        // "report nothing yet" branch on this.
        assert_eq!(device.music_sensitivity_value(), None);
    }

    #[test]
    fn music_sensitivity_round_trips_and_clamps() {
        let mut device = Device::new("H607C", "AA:BB:CC:DD:EE:FF:11:22");

        device.set_music_sensitivity(55);
        assert_eq!(device.music_sensitivity(), 55);
        assert_eq!(device.music_sensitivity_value(), Some(55));

        // Govee's range is 0-100; HA number boxes accept anything the user
        // types, so out-of-range input is clamped rather than rejected.
        device.set_music_sensitivity(255);
        assert_eq!(device.music_sensitivity(), 100);

        device.set_music_sensitivity(0);
        assert_eq!(device.music_sensitivity(), 0);
        assert_eq!(
            device.music_sensitivity_value(),
            Some(0),
            "zero is a real user choice, not an absent one"
        );
    }

    fn http_info_with_color_temp(sku: &str, min: u32, max: u32) -> HttpDeviceInfo {
        serde_json::from_value(serde_json::json!({
            "sku": sku,
            "device": "AA:BB:CC:DD:EE:FF:42:2A",
            "type": "devices.types.light",
            "capabilities": [{
                "type": "devices.capabilities.color_setting",
                "instance": "colorTemperatureK",
                "parameters": {
                    "dataType": "INTEGER",
                    "range": {"min": min, "max": max, "precision": 1}
                }
            }]
        }))
        .unwrap()
    }

    fn lan_device(sku: &str) -> LanDevice {
        serde_json::from_value(serde_json::json!({
            "ip": "127.0.0.1",
            "device": "AA:BB:CC:DD:EE:FF:42:2A",
            "sku": sku,
            "bleVersionHard": "",
            "bleVersionSoft": "",
            "wifiVersionHard": "",
            "wifiVersionSoft": "",
        }))
        .unwrap()
    }

    #[test]
    fn color_temp_unknown_lan_sku_prefers_platform_range() {
        // An SKU with no quirk entry, discovered via LAN, that the
        // Platform API says supports 2700-6500K (eg: H60C1)
        let mut device = Device::new("H60C1", "AA:BB:CC:DD:EE:FF:42:2A");
        device.lan_device = Some(lan_device("H60C1"));
        assert_eq!(
            device.get_color_temperature_range(),
            Some(DEFAULT_COLOR_TEMP_RANGE)
        );

        device.http_device_info = Some(http_info_with_color_temp("H60C1", 2700, 6500));
        assert_eq!(device.get_color_temperature_range(), Some((2700, 6500)));
    }

    #[test]
    fn color_temp_explicit_quirk_range_wins() {
        // H6022 has an explicit quirk range; it must not be overridden
        let mut device = Device::new("H6022", "AA:BB:CC:DD:EE:FF:42:2A");
        device.http_device_info = Some(http_info_with_color_temp("H6022", 2000, 9000));
        assert_eq!(device.get_color_temperature_range(), Some((2700, 6500)));
    }

    #[test]
    fn color_temp_unusable_platform_range_is_ignored() {
        let mut device = Device::new("H60C1", "AA:BB:CC:DD:EE:FF:42:2A");
        device.lan_device = Some(lan_device("H60C1"));
        for (min, max) in [(0, 0), (0, 6500), (6500, 2700), (4000, 4000)] {
            device.http_device_info = Some(http_info_with_color_temp("H60C1", min, max));
            assert_eq!(
                device.get_color_temperature_range(),
                Some(DEFAULT_COLOR_TEMP_RANGE),
                "{min}-{max}"
            );
        }
    }

    #[test]
    fn color_temp_broken_platform_is_ignored() {
        // H6141 has bogus platform metadata, so stick with the quirk default
        let mut device = Device::new("H6141", "AA:BB:CC:DD:EE:FF:42:2A");
        device.http_device_info = Some(http_info_with_color_temp("H6141", 2700, 6500));
        assert_eq!(
            device.get_color_temperature_range(),
            Some(DEFAULT_COLOR_TEMP_RANGE)
        );
    }
}
