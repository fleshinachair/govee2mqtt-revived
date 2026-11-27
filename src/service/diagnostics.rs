//! The report behind the "Report a problem" page. Its contents end up in a
//! public issue, so it is built from an approved list: nothing is copied unless
//! it is known not to identify the user. Anything else is replaced.

use crate::platform_api::{DeviceCapability, DeviceCapabilityKind};
use crate::service::device::Device;
use crate::service::quirks::resolve_quirk;
use crate::service::state::StateHandle;
use crate::version_info::govee_version;
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::{json, Map, Value as JsonValue};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Stands in for any text that may have been chosen by the user
const TEXT: &str = "<text>";
/// Scene lists run to hundreds of entries, which say little beyond a few
const LONGEST_LIST: usize = 40;
/// Keys of capability metadata whose text comes from Govee's own schema
const SCHEMA_KEYS: [&str; 6] = [
    "type",
    "instance",
    "dataType",
    "fieldName",
    "unit",
    "elementType",
];

#[derive(Default, Serialize, Debug, Clone, Copy)]
pub struct ProblemCount {
    pub errors: u64,
    pub warnings: u64,
}

static PROBLEMS: Lazy<Mutex<BTreeMap<String, ProblemCount>>> = Lazy::new(Default::default);

/// Count logged warnings and errors by the part of the code that logged them.
/// The messages themselves are not kept.
pub fn note_log(level: log::Level, module: Option<&str>) {
    if level > log::Level::Warn {
        return;
    }
    if let Ok(mut problems) = PROBLEMS.lock() {
        let count = problems
            .entry(module.unwrap_or("unknown").to_string())
            .or_default();
        if level == log::Level::Error {
            count.errors += 1;
        } else {
            count.warnings += 1;
        }
    }
}

#[derive(Serialize, Debug)]
pub struct Report {
    pub version: &'static str,
    pub install: &'static str,
    pub arch: &'static str,
    pub connections: Connections,
    pub problems_since_start: BTreeMap<String, ProblemCount>,
    pub devices: Vec<DeviceReport>,
}

/// Which connections were set up
#[derive(Serialize, Debug)]
pub struct Connections {
    pub platform_api: bool,
    pub govee_account: bool,
    pub aws_iot: bool,
    pub lan: bool,
    pub mqtt: bool,
}

#[derive(Serialize, Debug)]
pub struct DeviceReport {
    pub model: String,
    pub device_type: String,
    pub in_model_table: bool,
    pub seen_on_lan: bool,
    pub lan_versions: Option<BTreeMap<&'static str, String>>,
    pub in_platform_api: bool,
    pub in_govee_account: bool,
    pub has_iot_topic: Option<bool>,
    pub bluetooth_only: Option<bool>,
    pub has_lan_status: bool,
    pub has_iot_status: bool,
    pub capabilities: Vec<JsonValue>,
    pub readings: Vec<JsonValue>,
}

pub async fn collect(state: &StateHandle) -> Report {
    let mut devices: Vec<DeviceReport> = state.devices().await.iter().map(describe).collect();
    devices.sort_by(|a, b| a.model.cmp(&b.model));

    Report {
        version: govee_version(),
        install: if std::env::var_os("SUPERVISOR_TOKEN").is_some() {
            "Home Assistant add-on"
        } else if std::path::Path::new("/.dockerenv").exists() {
            "Docker"
        } else {
            "other"
        },
        arch: std::env::consts::ARCH,
        connections: Connections {
            platform_api: state.get_platform_client().await.is_some(),
            govee_account: state.get_undoc_client().await.is_some(),
            aws_iot: state.get_iot_client().await.is_some(),
            lan: state.get_lan_client().await.is_some(),
            mqtt: state.get_hass_client().await.is_some(),
        },
        problems_since_start: PROBLEMS.lock().map(|p| p.clone()).unwrap_or_default(),
        devices,
    }
}

/// Short text made of the characters that Govee's identifiers use. Notably
/// excludes the colons of a device id or network address.
fn is_label(text: &str) -> bool {
    text.len() <= 60
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-'))
}

fn label(text: &str) -> String {
    if is_label(text) {
        text.to_string()
    } else {
        TEXT.to_string()
    }
}

/// A version number such as `3.01.10`, and nothing else
fn version_number(text: &str) -> String {
    if text.len() <= 20
        && text.chars().all(|c| c.is_ascii_digit() || c == '.')
        && text.split('.').count() < 4
    {
        text.to_string()
    } else {
        TEXT.to_string()
    }
}

/// Which text survives [`scrub`]
#[derive(Clone, Copy)]
struct Keep {
    schema: bool,
    option_names: bool,
}

/// Copy `value`, keeping numbers, booleans and the shape, but only such text as
/// `keep` allows.
fn scrub(value: &JsonValue, key: &str, keep: Keep) -> JsonValue {
    match value {
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => value.clone(),
        JsonValue::String(text) => {
            let allowed =
                (keep.schema && SCHEMA_KEYS.contains(&key)) || (keep.option_names && key == "name");
            if text.is_empty() || (allowed && is_label(text)) {
                value.clone()
            } else {
                json!(TEXT)
            }
        }
        JsonValue::Array(items) => {
            let mut result: Vec<JsonValue> = items
                .iter()
                .take(LONGEST_LIST)
                .map(|item| scrub(item, key, keep))
                .collect();
            if items.len() > LONGEST_LIST {
                result.push(json!(format!("<{} more>", items.len() - LONGEST_LIST)));
            }
            JsonValue::Array(result)
        }
        JsonValue::Object(map) => JsonValue::Object(
            map.iter()
                .map(|(name, item)| {
                    let shown = if is_label(name) && !name.contains(' ') {
                        name.to_string()
                    } else {
                        "<key>".to_string()
                    };
                    (shown, scrub(item, name, keep))
                })
                .collect::<Map<_, _>>(),
        ),
    }
}

fn describe_capability(cap: &DeviceCapability) -> Option<JsonValue> {
    // Only the option names that Govee chooses. Scenes and snapshots are
    // named by the user.
    let option_names = matches!(
        cap.kind,
        DeviceCapabilityKind::OnOff | DeviceCapabilityKind::Toggle | DeviceCapabilityKind::WorkMode
    ) || cap.instance == "gearMode";

    serde_json::to_value(cap).ok().map(|value| {
        scrub(
            &value,
            "",
            Keep {
                schema: true,
                option_names,
            },
        )
    })
}

fn describe(device: &Device) -> DeviceReport {
    let device_type = serde_json::to_value(device.device_type())
        .ok()
        .and_then(|value| value.as_str().map(label))
        .unwrap_or_else(|| TEXT.to_string());

    let settings = device
        .undoc_device_info
        .as_ref()
        .map(|info| &info.entry.device_ext.device_settings);

    let capabilities = device
        .http_device_info
        .iter()
        .flat_map(|info| info.capabilities.iter())
        .filter_map(describe_capability)
        .collect();

    let readings = device
        .http_device_state
        .iter()
        .flat_map(|state| state.capabilities.iter())
        .map(|cap| {
            json!({
                "type": label(&cap.kind.to_string()),
                "instance": label(&cap.instance),
                "state": scrub(
                    &cap.state,
                    "",
                    Keep { schema: false, option_names: false },
                ),
            })
        })
        .collect();

    DeviceReport {
        model: label(&device.sku),
        device_type,
        in_model_table: resolve_quirk(&device.sku).is_some(),
        seen_on_lan: device.lan_device.is_some(),
        lan_versions: device.lan_device.as_ref().map(|lan| {
            BTreeMap::from([
                ("ble_hardware", version_number(&lan.ble_version_hard)),
                ("ble_software", version_number(&lan.ble_version_soft)),
                ("wifi_hardware", version_number(&lan.wifi_version_hard)),
                ("wifi_software", version_number(&lan.wifi_version_soft)),
            ])
        }),
        in_platform_api: device.http_device_info.is_some(),
        in_govee_account: device.undoc_device_info.is_some(),
        has_iot_topic: settings.map(|s| s.topic.is_some()),
        bluetooth_only: settings.map(|s| s.wifi_name.is_none()),
        has_lan_status: device.lan_device_status.is_some(),
        has_iot_status: device.iot_device_status.is_some(),
        capabilities,
        readings,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::platform_api::from_json;

    #[test]
    fn nothing_that_identifies_the_user_is_reported() {
        let id = "AA:BB:CC:DD:EE:FF:42:2A";
        let mut device = Device::new("H7122", id);
        device.set_http_device_info(
            from_json(
                json!({
                    "sku": "H7122",
                    "device": id,
                    "deviceName": "PRIVATE room name",
                    "type": "devices.types.air_purifier",
                    "capabilities": [
                        {
                            "type": "devices.capabilities.work_mode",
                            "instance": "workMode",
                            "parameters": {
                                "dataType": "STRUCT",
                                "fields": [{
                                    "fieldName": "workMode",
                                    "dataType": "ENUM",
                                    "options": [{ "name": "Sleep", "value": 1 }]
                                }]
                            }
                        },
                        {
                            "type": "devices.capabilities.dynamic_scene",
                            "instance": "diyScene",
                            "parameters": {
                                "dataType": "ENUM",
                                "options": [{ "name": "PRIVATE scene", "value": 7 }]
                            }
                        }
                    ]
                })
                .to_string(),
            )
            .unwrap(),
        );
        device.set_http_device_state(
            from_json(
                json!({
                    "sku": "H7122",
                    "device": id,
                    "capabilities": [{
                        "type": "devices.capabilities.property",
                        "instance": "airQuality",
                        "state": { "value": 6, "name": "PRIVATE reading", "type": "PRIVATE" }
                    }]
                })
                .to_string(),
            )
            .unwrap(),
        );
        device.set_lan_device(
            from_json(
                json!({
                    "ip": "192.0.2.23",
                    "device": id,
                    "sku": "H7122",
                    "bleVersionHard": "3.01.10",
                    "bleVersionSoft": "1.03.01",
                    "wifiVersionHard": "1.00.10",
                    "wifiVersionSoft": "192.0.2.23"
                })
                .to_string(),
            )
            .unwrap(),
        );

        let report = serde_json::to_string(&describe(&device)).unwrap();

        for private in ["PRIVATE", id, "EE:FF", "192.0.2"] {
            assert!(!report.contains(private), "{private} in {report}");
        }
        // What is needed to fix things is still there
        for useful in [
            "H7122",
            "devices.types.air_purifier",
            "airQuality",
            "Sleep",
            "diyScene",
            "3.01.10",
        ] {
            assert!(report.contains(useful), "{useful} missing from {report}");
        }
        assert!(report.contains(r#""value":6"#), "{report}");
    }

    #[test]
    fn long_lists_are_cut_short() {
        let many: Vec<u32> = (0..100).collect();
        let keep = Keep {
            schema: false,
            option_names: false,
        };
        let result = scrub(&json!(many), "", keep);
        assert_eq!(result.as_array().map(Vec::len), Some(LONGEST_LIST + 1));
        assert_eq!(result[LONGEST_LIST], json!("<60 more>"));
    }

    #[test]
    fn only_warnings_and_errors_are_counted() {
        let module = Some("govee::diagnostics_test");
        note_log(log::Level::Error, module);
        note_log(log::Level::Warn, module);
        note_log(log::Level::Info, module);

        let counts = PROBLEMS.lock().unwrap()["govee::diagnostics_test"];
        assert_eq!((counts.errors, counts.warnings), (1, 1));
    }
}
