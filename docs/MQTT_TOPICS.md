# MQTT topics

The topics the bridge uses on your broker, for access control lists and for
driving it without Home Assistant. Home Assistant needs none of this: it learns
the topics by itself.

`<id>` is the device ID without colons, such as `AABBCCDDEEFF1122`. `<prefix>`
is the discovery prefix, `homeassistant` unless you changed it.

## Access control

A broker user for the bridge needs:

| Access | Topics |
|---|---|
| Read and write | `gv2mqtt/#` |
| Write | `<prefix>/#` |
| Read | `<prefix>/status`, `hass/status` |

If you have Govee devices that only report events through Govee's cloud, those
arrive over a separate connection to Govee and need nothing from your broker.

## Topics the bridge listens on

| Topic | Payload |
|---|---|
| `gv2mqtt/light/<id>/command` | Home Assistant JSON light command, such as `{"state":"ON","brightness":50}`, with optional `color`, `color_temp` (kelvin), `effect` and `transition` |
| `gv2mqtt/light/<id>/command/<segment>` | The same, for one segment |
| `gv2mqtt/switch/<id>/command/<instance>` | `ON` or `OFF`. `<instance>` is Govee's name, such as `powerSwitch` |
| `gv2mqtt/fan/<id>/command` | `ON` or `OFF` |
| `gv2mqtt/fan/<id>/set-percentage` | A speed step number |
| `gv2mqtt/number/<id>/command/<mode_name>/<work_mode>` | A whole number, the setting for that work mode |
| `gv2mqtt/humidifier/<id>/set-mode` | A work mode name |
| `gv2mqtt/humidifier/<id>/set-target` | A target humidity in percent |
| `gv2mqtt/<id>/set-work-mode` | A work mode name |
| `gv2mqtt/<id>/set-temperature/<instance>/<units>` | A temperature. `<units>` is the scale it is in, as advertised in discovery |
| `gv2mqtt/<id>/set-auto-stop` | `ON` or `OFF`. Retain it, so that it survives a restart |
| `gv2mqtt/<id>/set-mode-scene` | A scene name |
| `gv2mqtt/<id>/scene-next` | Anything |
| `gv2mqtt/<id>/scene-prev` | Anything |
| `gv2mqtt/<id>/set-music-sensitivity` | `0` to `100` |
| `gv2mqtt/<id>/clear-music-sensitivity` | Anything |
| `gv2mqtt/<id>/set-music-palette` | See [music mode](MUSIC_MODE.md). Off unless enabled |
| `gv2mqtt/<id>/request-platform-data` | Anything |
| `gv2mqtt/oneclick` | The name of a Tap-to-Run scene |
| `gv2mqtt/purge-caches` | Anything |
| `gv2mqtt/energy/<id>/counter` | The bridge's own saved energy counter |
| `<prefix>/status`, `hass/status` | `online` makes the bridge announce its entities again |

## Topics the bridge publishes

| Topic | Content |
|---|---|
| `<prefix>/<component>/<unique_id>/config` | Home Assistant discovery. Not retained |
| `gv2mqtt/availability` | `online` or `offline`, repeated every minute. Not retained |
| `gv2mqtt/<id>/availability` | The same for one device, only with `offline_unavailable`. Retained |
| `gv2mqtt/light/<id>/state` | Light state as JSON |
| `gv2mqtt/light/<id>/state/<segment>` | Segment state as JSON |
| `gv2mqtt/switch/<id>/<instance>/state` | `ON` or `OFF` |
| `gv2mqtt/fan/<id>/state` | Fan power |
| `gv2mqtt/fan/<id>/notify-percentage`, `notify-preset-mode`, `notify-oscillation` | Fan speed, preset and oscillation |
| `gv2mqtt/humidifier/<id>/state`, `notify-mode`, `notify-target` | Humidifier state |
| `gv2mqtt/number/<id>/state/<mode_name>` | A work mode setting |
| `gv2mqtt/sensor/<unique_id>/state` | A sensor value |
| `gv2mqtt/sensor/<unique_id>/attributes` | A sensor's attributes as JSON |
| `gv2mqtt/binary_sensor/<unique_id>/state` | A binary sensor value |
| `gv2mqtt/event/<id>/<instance>/state` | A device event |
| `gv2mqtt/<id>/advise-set-temperature` | The target temperature |
| `gv2mqtt/<id>/auto-stop/state` | `ON` or `OFF` |
| `gv2mqtt/<id>/notify-work-mode` | The current work mode |
| `gv2mqtt/<id>/notify-mode-scene` | The current scene of a device that is not a light |
| `gv2mqtt/<id>/notify-music-sensitivity` | The stored sensitivity, or `None` |
| `gv2mqtt/<id>/scene-catalog` | The scenes by category, as JSON. Retained |
| `gv2mqtt/energy/<id>/counter` | The saved energy counter. Retained |
