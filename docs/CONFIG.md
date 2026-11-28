# Configuration Options

There are three ways to configure Govee2MQTT, depending on how you installed it:

| Column | When to use |
|--------|-------------|
| **CLI** | Command-line flags, e.g. `govee serve --govee-email user@example.com` |
| **ENV** | Environment variables in a `.env` file or `docker-compose.yml` |
| **App Config** | The configuration panel in the Home Assistant app UI |

If you installed Govee2MQTT as a **Home Assistant app**, use the **App Config** column.
If you're running in **Docker**, use the **ENV** column.

## Govee Credentials

Govee2MQTT can run without any Govee credentials, but it will only discover
devices that have LAN control enabled. For the best experience, configure
your credentials before your first run:

- **Email + password** (recommended) — the same login you use in the Govee Home app. This is the only way for Govee2MQTT to learn your room names and assign devices to the right Home Assistant areas.
- **API key** (optional but recommended) — enables scene control, segment colors, and music modes. [Get a free key from Govee's developer portal](https://developer.govee.com/reference/apply-you-govee-api-key).

|CLI|ENV|App Config|Default|Purpose|
|---|---|----------|-------|-------|
|`--govee-email`|`GOVEE_EMAIL`|`govee_email`|*(none)*|Your Govee account email|
|`--govee-password`|`GOVEE_PASSWORD`|`govee_password`|*(none)*|Your Govee account password|
|`--govee-2fa-code`|`GOVEE_2FA_CODE`|`govee_2fa_code`|*(none)*|Emailed verification code, only needed if your account has 2FA. See below.|
|`--api-key`|`GOVEE_API_KEY`|`govee_api_key`|*(none)*|Your Govee API key ([get one here](https://developer.govee.com/reference/apply-you-govee-api-key))|

*Concerned about sharing your credentials? See [Privacy](PRIVACY.md) for
details on how your data is used.*

### Two-factor authentication (2FA)

If your Govee account requires two-factor authentication:

1. **Start without a code.** Leave `govee_2fa_code` or `GOVEE_2FA_CODE` unset and
   start Govee2MQTT. On status **454**, it requests a code by email. Retries for
   the same account reuse that request for 15 minutes.
2. **Set the code.** In Home Assistant, paste it into `govee_2fa_code`. In
   Docker, set `GOVEE_2FA_CODE` in your `.env`. Govee codes expire after
   **about 15 minutes**.
3. **Restart Govee2MQTT.** Restart the Home Assistant add-on or Docker container
   to retry login with the code. After login succeeds, remove the code from the
   saved configuration. The running process also clears its copy.

If Govee returns status **454** or **455** with a configured code, clear it and
restart without one. This clears the cached email request, so the next no-code
454 can request another. Codes from Govee's web store (Shopify) use a different
`clientId` and may fail here with status 455. Use the code requested by
Govee2MQTT.

**If no email arrives.** Two warnings can appear and they mean different things.

`Could not request a Govee 2FA verification code` means the request never
reached Govee, so no code was sent. Restart to retry it right away.

`Govee 2FA verification request outcome unknown, assuming the code was sent`
means Govee answered but the reply was unreadable or reported a failure. A code
may well be sitting in your inbox already. Govee2MQTT will not request another
one for 15 minutes, because a second code invalidates the first — so restarting
inside that window will not send a new email.

In both cases the 2FA instructions above still apply and `govee_2fa_code` is
still the setting to fill in. Do **not** follow the "remove your Govee API
credentials" suggestion that appears further down that log message — that
disables cloud control entirely and is not the fix for a 2FA prompt.

> **Note on token refresh:** Govee's session tokens last days, sometimes weeks.
> When a later login requires 2FA, repeat the steps above. If Home Assistant
> stops seeing Govee devices, check the add-on logs for status 454 or 455.

## LAN API Control

Many Govee devices support local control over your home network, without
needing internet access. This is faster and more reliable than cloud control.

**Before you start:** You must enable the LAN API for each device individually
in the Govee Home app (device settings → LAN Control toggle).

The [Govee LAN API guide](https://app-h5.govee.com/user-manual/wlan-guide)
lists which devices support it.

### How discovery works

By default, Govee2MQTT finds devices using multicast — it sends a message to
a special network address and waits for devices to respond. This works
automatically on most networks.

**If your devices aren't found:** Some routers and Wi-Fi setups block
multicast traffic. Try these alternatives in order:

|CLI|ENV|App Config|Default|What it does|
|---|---|----------|-------|------------|
|`--broadcast-all`|`GOVEE_LAN_BROADCAST_ALL=true`|`broadcast_all`|*(off)*|Sends discovery to every network interface on your system. **Try this first** if multicast doesn't work.|
|`--scan`|`GOVEE_LAN_SCAN=10.0.0.1,10.0.0.2`|`scan`|*(none)*|Sends discovery directly to specific device IPs. Assign your Govee devices static IPs in your router first, then list them here (comma-separated).|
|`--no-multicast`|`GOVEE_LAN_NO_MULTICAST=true`|`no_multicast`|*(off)*|Disables the default multicast discovery. Only use this together with one of the alternatives above.|
|`--global-broadcast`|`GOVEE_LAN_BROADCAST_GLOBAL=true`|`global_broadcast`|*(off)*|Sends discovery to 255.255.255.255. Rarely helps if multicast already fails.|

[More about LAN API troubleshooting](LAN.md)

### Polling behavior on congested networks

Govee2MQTT polls every LAN device for its status at least every 30 seconds
(the pass over all devices is serial, so unresponsive devices stretch the
cycle) and after each command. On a congested 2.4 GHz network, retries for
unresponsive devices used to pile up (up to ~29 packets per device per
status query), making the congestion worse. Two mechanisms bound this; both
are tunable without rebuilding:

|CLI|ENV|App Config|Default|What it does|
|---|---|----------|-------|------------|
|`--lan-query-attempts`|`GOVEE_LAN_QUERY_ATTEMPTS=3`|`lan_query_attempts`|`3`|How many times a status query is sent before giving up. Clamped to 1–100.|
|`--lan-query-backoff-ms`|`GOVEE_LAN_QUERY_BACKOFF_MS=350`|`lan_query_backoff_ms`|`350`|Wait after the first attempt, in milliseconds. Doubles on each retry, capped at 3000 ms (the cap is fixed). Defaults give waits of 350 ms → 700 ms → 1400 ms, ~2.5 s total.|
|`--lan-breaker-threshold`|`GOVEE_LAN_BREAKER_THRESHOLD=3`|`lan_breaker_threshold`|`3`|After this many consecutive timeouts, background polling of that device is suspended (circuit breaker). `0` disables the breaker.|
|`--lan-breaker-cooldown`|`GOVEE_LAN_BREAKER_COOLDOWN=300`|`lan_breaker_cooldown`|`300`|Suspension length in seconds, clamped to 30–900. Doubles on repeated failure, capped at 900 s.|

The tradeoff: lowering attempts makes a congested network recover faster but
makes a slow-to-respond device more likely to report stale state. If you have
a device that reliably answers only after several seconds, raise
`lan_query_attempts` to 4–5. (Above 3 attempts a single query cycle exceeds
the ~5 s post-command confirmation window, so the confirmation poll makes one
full pass instead of re-checking until the commanded value sticks.)

The circuit breaker only affects **background polling**. Commands you send
(turn on, brightness, color) always go out, and their confirmation polls
always run. A suspended device that shows any sign of life — a discovery
response, for example — is granted an immediate status probe instead of
waiting out the cooldown, and the first successful status reply fully
resets the breaker; recovery after an outage therefore takes at most about
a minute. A device that answers discovery but keeps dropping status queries
stays suspended, at the cost of one probe per discovery cycle. Note that a
timed-out confirmation poll still counts toward the breaker threshold even
though it is never blocked: unreachability is evidence no matter which poll
observed it.

## MQTT Configuration

MQTT is the messaging protocol that connects Govee2MQTT to Home Assistant.
You need an MQTT broker (server) running — the
[Mosquitto app](https://www.home-assistant.io/integrations/mqtt/#configuration)
is the easiest option.

**Home Assistant app users:** If you installed Mosquitto as a Home Assistant
app, leave these fields blank — they are filled in automatically.

|CLI|ENV|App Config|Default|Purpose|
|---|---|----------|-------|-------|
|`--mqtt-host`|`GOVEE_MQTT_HOST`|`mqtt_host`|*(auto-detected)*|Host name or IP address of your MQTT broker|
|`--mqtt-port`|`GOVEE_MQTT_PORT`|`mqtt_port`|`1883`|Port number of your MQTT broker|
|`--mqtt-username`|`GOVEE_MQTT_USER`|`mqtt_username`|*(none)*|Username, if your broker requires authentication|
|`--mqtt-password`|`GOVEE_MQTT_PASSWORD`|`mqtt_password`|*(none)*|Password, if your broker requires authentication|
|`--mqtt-tls`|`GOVEE_MQTT_TLS`|`mqtt_tls`|*(off)*|Connect with TLS, trusting the usual certificate authorities|
|`--mqtt-ca`|`GOVEE_MQTT_CA`|`mqtt_ca`|*(none)*|Certificate file or directory of your own certificate authority. Turns TLS on|
|`--hass-discovery-prefix`|`GOVEE_HASS_DISCOVERY_PREFIX`|`hass_discovery_prefix`|`homeassistant`|Must match the discovery prefix of Home Assistant's MQTT integration|

With TLS the default port becomes `8883`, and the broker's certificate must
match the host name or address you give. In the Home Assistant app, set
`mqtt_host` and `mqtt_port` yourself, as the automatically filled port does not
use TLS. Put your certificate authority's file in this app's folder under
`addon_configs`, which the app sees as `/app/config`. Client certificates are
not supported.

## Offline devices

By default a device that has lost power keeps its last state in Home Assistant,
and its **Status** sensor shows Offline.

|ENV|App Config|Default|Purpose|
|---|----------|-------|-------|
|`GOVEE_OFFLINE_UNAVAILABLE=true`|`offline_unavailable`|*(off)*|Show a device's entities as unavailable while Govee reports it offline|

This relies on the Platform API's online flag, which is refreshed about every 15
minutes for cloud-only devices and is not always accurate. A device known only
over the LAN is never marked unavailable. The Status sensor and the **Request
Platform API State** button stay usable.

## Ignoring devices

Govee goes on listing groups and devices you no longer have, so deleting them
in Home Assistant does not last.

|ENV|App Config|Default|Purpose|
|---|----------|-------|-------|
|`GOVEE_IGNORE_DEVICES=14092648,Old lamp`|`ignore_devices`|*(none)*|Device IDs, models or names, separated by commas, to leave out of Home Assistant|

1. Add the device to the list. A model, such as `H6001`, ignores every device of
   that model. IDs and names are shown on the bridge's web page and in its log.
2. Restart the bridge.
3. In Home Assistant, open **Settings → Devices & services → MQTT**, choose the
   device, and delete it. It stays deleted.

The device is still shown on the bridge's web page. Tap-to-Run scenes are not
affected. Remove it from the list to bring it back.

## Lights that ignore commands

Some older lights without LAN control accept commands sent over AWS IoT and do
nothing. You can have chosen devices controlled through the Platform API
instead, and keep your account for Tap-to-Run and room names.

|ENV|App Config|Default|Purpose|
|---|----------|-------|-------|
|`GOVEE_NO_IOT_CONTROL=H6159,H6003`|`no_iot_control`|*(none)*|Models or device IDs, separated by commas, to control without AWS IoT|

This needs the Platform API key. Commands to a listed device are slower and
count against Govee's request limit. LAN control, where a device has it, is
still used first. A few model-specific features, such as the H1270's separate
lights, are only possible over AWS IoT and still use it.

## Wrong number of segments

Govee reports too many or too few segments for some lights. You can give the
right number.

|ENV|App Config|Default|Purpose|
|---|----------|-------|-------|
|`GOVEE_SEGMENT_COUNTS=H612D=20,H7094=4`|`segment_counts`|*(Govee's count)*|Model or device ID and its number of segments, from 0 to 128, separated by commas|

* An entry for a device ID beats one for its model. `0` removes the segments.
* It only works on lights that already have segment entities.
* After a restart of the bridge, extra segments are added. Segments you removed
  stay in Home Assistant as unavailable once Home Assistant has restarted too,
  and you can delete them under the MQTT integration.
* Segments beyond the number Govee reports may be refused by Govee. The bridge
  log then shows the error.
* It does not help where segments change the whole light instead of one part.

## Heaters and Auto Stop

Govee switches a heater's Auto Stop off whenever its target temperature is set
without it. A heater that has the option gets an **Auto Stop** switch. While it
is on, setting the target temperature from Home Assistant asks for Auto Stop
too. Flipping the switch sends nothing by itself, and it does not show what the
Govee app is set to, as Govee does not report that.

## Sensor reading interval

Thermometers, air quality monitors and other sensors are read from Govee every
15 minutes by default.

|ENV|App Config|Default|Purpose|
|---|----------|-------|-------|
|`GOVEE_SENSOR_POLL_INTERVAL=300`|`sensor_poll_interval`|`900`|Seconds between reads, from 60 to 900|

Each read of each sensor counts against Govee's daily request limit. Reading
more often cannot give newer values than the sensor has uploaded to Govee.

## Effects (Scenes)

By default Govee2MQTT publishes each light's full list of scene "effects" to
Home Assistant. Very long effect lists can overflow the Google Home
integration, which rejects oversized MQTT discovery payloads. These options
let you trim or disable the published effect list. They are environment-only
(no CLI flag).

|CLI|ENV|App Config|Default|Purpose|
|---|---|----------|-------|-------|
|*(none)*|`GOVEE_DISABLE_EFFECTS=true`|`disable_effects`|*(off)*|Stop publishing effect lists for every device. Use this if a long effect list breaks the Google Home integration.|
|*(none)*|`GOVEE_ALLOWED_EFFECTS=Aurora,Rainbow`|`allowed_effects`|*(all)*|Comma-separated allowlist; only effects matching these names are published. Keeps some effects while shrinking the payload.|

`disable_effects` wins: if it is set, no effects are published regardless of
`allowed_effects`.

## Music Mode

Music mode reacts to sound. Selecting a `Music: <Style>` effect on a light
already works through the Platform API with no additional music-specific
opt-in once the API key is configured.

Programming music mode with **your own palette** is opt-in, because the frames
are reverse-engineered and only mapped for a few SKUs. Turning it on enables the
`gv2mqtt/<id>/set-music-palette` MQTT command topic, which sends the palette
over the LAN API. See [Music mode](MUSIC_MODE.md) for the supported devices, the
payload format, and how to map a new SKU.

|CLI|ENV|App Config|Default|Purpose|
|---|---|----------|-------|-------|
|*(none)*|`GOVEE_MUSIC_PALETTE=true`|`music_palette`|*(off)*|Enable the `set-music-palette` command topic. Requires LAN control for the device.|

Sensitivity is separate. The **Music Sensitivity** number entity stores the value
used the next time you select a `Music:` effect; it is not sent on its own,
because Govee rejects a `music_setting` call that omits the style, and sending
the style would switch the light into music mode (and power it on) as a side
effect of moving a slider.

The value is held in memory. After a Govee2MQTT restart the entity reads as
unknown until you set it again, and a `Music:` effect selected in the meantime
uses the default of 100. Govee offers no way to read the current sensitivity
back from a light, so the bridge cannot recover it.

A **Clear Music Sensitivity** button ships alongside the slider and forgets the
stored value, putting the entity back to unknown and the next `Music:` effect
back on the default. It is a button rather than something you write to the
slider because Home Assistant reads a number's `payload_reset` on the state
topic and never publishes it to the command topic, so a number entity on its own
offers no way back to unknown.

The Platform API also accepts one fixed music colour. Send it together with the
effect through Home Assistant's normal light service:

```yaml
service: light.turn_on
target:
  entity_id: light.your_govee
data:
  effect: "Music: Rhythm"
  rgb_color: [18, 52, 86]
```

With `rgb_color`, the bridge sends `autoColor: 0` plus the packed RGB value.
Without it, the bridge preserves the historical `autoColor: 1`, so the device
chooses colours. The Platform API supports only this one colour, not a palette.

Note that Home Assistant can supply `rgb_color` without you asking for it.
Restoring a scene, or any script that replays a light's captured attributes,
sends the whole attribute set — so a snapshot taken while the light was in a
`Music:` effect replays `effect` *and* `rgb_color` together, which pins the
colour and stops the automatic cycling. Ordinary (non-`Music:`) scenes ignore
colour in the same command, exactly as before. To restore a cycling music
effect, send `effect` on its own.

### MQTT topics

The number entity is the supported interface; these are the topics behind it,
for driving the bridge directly over MQTT rather than through Home Assistant.

|Topic|Direction|Payload|
|-----|---------|-------|
|`gv2mqtt/<id>/set-music-sensitivity`|command|`0`-`100`. Decimals round, out-of-range values clamp.|
|`gv2mqtt/<id>/clear-music-sensitivity`|command|Any payload. Forgets the stored value; the next `Music:` effect uses the default of 100.|
|`gv2mqtt/<id>/notify-music-sensitivity`|state|The stored value, or `None` when nothing has been set yet. `None` is the entity's `payload_reset`, which Home Assistant reads on this topic to show the number as unknown.|

The LAN palette frames carry their own sensitivity byte, which is a different
setting from the Platform API one the number entity writes.

## HTTP Configuration

The web page and HTTP API have **no authentication** and can control devices.
Do not expose them beyond your local network.

In the Home Assistant app, **Open web UI** shows the page inside Home Assistant,
which asks you to sign in first. The port below is separate and stays open to
your network; set `http_addr` to `127.0.0.1` to close it.

|CLI|ENV|App Config|Default|Purpose|
|---|---|----------|-------|-------|
|`--http-addr`|`HTTP_ADDR`|`http_addr`|`0.0.0.0`|IP address the web page listens on. `127.0.0.1` limits it to the local machine; `::` listens on IPv6.|
|`--http-port`|`HTTP_PORT`|`http_port`|`8056`|Port the web page listens on. **Open web UI** in the add-on does not depend on it.|

## Device Events

Some Govee devices emit *events* rather than ongoing state. Examples include
the ice-maker-full and water-empty notifications on the H7172 ice maker, and
presence/absence on presence sensors such as the H5127.

When a Govee API Key is configured, `govee2mqtt` subscribes directly to Govee's
cloud MQTT broker (`mqtt.openapi.govee.com`) and republishes these events to
Home Assistant as
[MQTT `event` entities](https://www.home-assistant.io/integrations/event.mqtt/).
This removes the need for the manual Mosquitto bridge workaround described in
[issue #343](https://github.com/wez/govee2mqtt/issues/343); the relevant event
entities are created automatically as part of the normal discovery process.

This feature is enabled automatically when an API Key is present and a
device advertises an event capability. The connection is supervised and will
reconnect automatically (with exponential backoff) if it is dropped.

|CLI|ENV|App Config|Purpose|
|---|---|----------|-------|
|`--disable-platform-mqtt`|`GOVEE_DISABLE_PLATFORM_MQTT=true`|`disable_platform_mqtt`|Disable subscribing to Govee's cloud MQTT broker for device events.|
|`--platform-mqtt-ca`|`GOVEE_MQTT_PLATFORM_CA`||Path to a CA certificate file, or a directory of CA certificates, used to validate the TLS connection to Govee's cloud MQTT broker. Defaults to autodetecting the system CA bundle, which is correct for the official container image and Home Assistant Add-on.|

*The events delivered here are momentary. If `govee2mqtt` is restarted or
briefly disconnected, any events that occur during that window are missed; this
is a limitation of the Govee cloud and applies equally to the manual bridge
workaround.*
