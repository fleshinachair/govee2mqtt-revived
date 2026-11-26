# Govee to MQTT (Revived)

This repo provides a `govee` executable whose primary purpose is to act
as a bridge between [Govee](https://govee.com) devices and Home Assistant,
via the [Home Assistant MQTT Integration](https://www.home-assistant.io/integrations/mqtt/).

It is a community fork of [wez/govee2mqtt](https://github.com/wez/govee2mqtt)
that builds on
[florianhorner/govee2mqtt-extended](https://github.com/florianhorner/govee2mqtt-extended).
See [Credits](#credits).

> **Pre-release.** This is an early build for testers. It passes its automated
> tests, but most changes have not been confirmed on real hardware by this
> fork. Take a Home Assistant backup first, and please report what breaks in
> [Issues](https://github.com/fleshinachair/govee2mqtt-revived/issues).
> [What changed](RELEASE_NOTES.md) · [What to test](TESTING.md)

## Quick Install

### Home Assistant add-on (HAOS and Supervised)

[![Add this repository to Home Assistant](https://my.home-assistant.io/badges/supervisor_add_addon_repository.svg)](https://my.home-assistant.io/redirect/supervisor_add_addon_repository/?repository_url=https%3A%2F%2Fgithub.com%2Ffleshinachair%2Fgovee2mqtt-revived)

Or add it by hand:

1. Go to **Settings → Apps → App store → ⋮ → Repositories**.
2. Add `https://github.com/fleshinachair/govee2mqtt-revived`.
3. Install **Govee to MQTT (Revived)**, configure it, and start it.

Coming from `wez/govee2mqtt`? Read [the migration steps](#migrating-from-wezgovee2mqtt-zero-entity-loss)
**before** starting it. First-time setup, including the MQTT broker, is covered
in [Installing the add-on](docs/ADDON.md).

### Docker

```
ghcr.io/fleshinachair/govee2mqtt-revived:latest
```

See [Running it in Docker](docs/DOCKER.md).

## Migrating from `wez/govee2mqtt` (Zero Entity Loss)

Your existing `light.*`, `switch.*`, `select.*`, `fan.*` and `sensor.*`
entities, with their automations and history, carry over. Home Assistant
identifies them by `unique_id`s derived from each device's hardware ID, and
this fork keeps the same ones. It only adds entities.

> **Warning: never run both at the same time.** Two bridges publishing the same
> devices to the same MQTT broker will fight over every entity, and both want
> port 8056.

### Home Assistant add-on

1. Open the old add-on's **Configuration** tab, choose **⋮ → Edit in YAML**, and
   copy everything.
2. **Stop** the old add-on. **Do not uninstall it** yet. Also turn off its
   **Start on boot** and **Watchdog** switches so it cannot come back by itself.
3. Add this repository (see [Quick Install](#quick-install)) and install
   **Govee to MQTT (Revived)**.
4. On its **Configuration** tab choose **⋮ → Edit in YAML**, paste, and save.
5. Turn on **Start on boot** and **Watchdog**. The bridge exits when it cannot
   recover by itself, and the Watchdog restarts it.
6. **Start** it and check the log.

To go back, stop this add-on and start the old one.

### Docker Compose

1. Stop the old container: `docker compose down` (without `-v`).
2. In `docker-compose.yml`, change the image to
   `ghcr.io/fleshinachair/govee2mqtt-revived:latest`. Keep your `.env` file and
   any volumes.
3. Start it: `docker compose up -d`

### What to expect

* **2FA accounts:** the new add-on starts with an empty cache, so Govee may ask
  for a new verification code. See
  [two-factor authentication](docs/CONFIG.md#two-factor-authentication-2fa).
* **Sensor templates:** some sensors, such as air quality and filter life, used
  to report `{"value": 6}` and now report `6`. Remove any template that
  unwrapped the old form.
* **New entities:** fans and air purifiers also get a `fan` entity, lights get
  scene next/previous buttons and a scene info sensor, and lights with music
  modes get a sensitivity slider.
* **Color temperature:** some lights now offer a narrower range, taken from
  Govee's data for that device.

## Features

* Robust LAN-first design. Not all of Govee's devices support LAN control,
  but for those that do, you'll have the lowest latency and ability to
  control them even when your primary internet connection is offline.
* Support for per-device modes and scenes.
* Support for the undocumented AWS IoT interface to your devices, provides
  low latency status updates.
* Support for the new [Platform
  API](https://developer.govee.com/reference/get-you-devices) in case the AWS
  IoT or LAN control is unavailable.

|Feature|Requires|Notes|
|-------|--------|-------------|
|DIY Scenes|API Key|Find in the list of Effects for the light in Home Assistant|
|Music Modes|API Key|Find in the list of Effects for the light in Home Assistant|
|Tap-to-Run / One Click Scene|IoT|Find in the overall list of Scenes in Home Assistant, as well as under the `Govee to MQTT` device|
|Live Device Status Updates|LAN and/or IoT|Devices typically report most changes within a couple of seconds.|
|Light Transitions|LAN|Set `transition` (seconds) on `light.turn_on` together with a brightness. The fade is made of stepped brightness commands; any newer command ends it|
|Energy Monitoring|IoT|Power, energy, voltage, current and power factor sensors for the H5086 smart plug|
|Segment Color|API Key|Find the `Segment 00X` light entities associated with your main light device in Home Assistant|
|Local Segment Color|LAN|H6061 and H6066 segments work without an API key|
|Device Events|API Key|Ice maker full, water empty, presence and similar, as event entities|

* `API Key` means that you have [applied for a key from Govee](https://developer.govee.com/reference/apply-you-govee-api-key)
  and have configured it for use in govee2mqtt
* `IoT` means that you have configured your Govee account email and password for
  use in govee2mqtt, which will then attempt to use the
  *undocumented and likely unsupported* AWS MQTT-based IoT service
* `LAN` means that you have enabled the [Govee LAN API](https://app-h5.govee.com/user-manual/wlan-guide)
  on supported devices and that the LAN API protocol is functional on your network

## Usage

* [Installing the HASS Add-On](docs/ADDON.md) - for HAOS and Supervised HASS users
* [Running it in Docker](docs/DOCKER.md)
* [Configuration](docs/CONFIG.md)

## Have a question?

* [Is my device supported?](docs/SKUS.md)
* [Check out the FAQ](docs/FAQ.md)
* [LAN API troubleshooting](docs/LAN.md)
* [Music mode with custom palettes](docs/MUSIC_MODE.md)

## Credits

This is a community fork, and most of the work in it was done by others.

* **[Wez Furlong](https://github.com/wez)** created
  [govee2mqtt](https://github.com/wez/govee2mqtt) and wrote the bridge itself.
  It is released under the [MIT license](LICENSE.md).
* **[Florian Horner](https://github.com/florianhorner)** maintains
  [govee2mqtt-extended](https://github.com/florianhorner/govee2mqtt-extended),
  where this fork's 2FA login, crash fixes, fan entities, scene controls, music
  modes and much more were developed. They were brought over commit by commit,
  keeping their authors.

Authors of open upstream pull requests applied here:

* [UPEngineer](https://github.com/UPEngineer): Tap-to-Run shortcuts
  ([wez/govee2mqtt#739](https://github.com/wez/govee2mqtt/pull/739))
* [Raul-7-7](https://github.com/Raul-7-7): translated option names
  ([wez/govee2mqtt#645](https://github.com/wez/govee2mqtt/pull/645))
* [Nick Greenway](https://github.com/nickgreenway): surviving cache write
  failures ([wez/govee2mqtt#703](https://github.com/wez/govee2mqtt/pull/703))

* [TBE4034](https://github.com/TBE4034): H605C
  ([#751](https://github.com/wez/govee2mqtt/pull/751))
* [rutalreja-deloitte](https://github.com/rutalreja-deloitte): H61A9
  ([#595](https://github.com/wez/govee2mqtt/pull/595))
* [hartmood](https://github.com/hartmood): MQTT connect timeout
  ([#687](https://github.com/wez/govee2mqtt/pull/687))
* [aserper](https://github.com/aserper): segment commands
  ([#676](https://github.com/wez/govee2mqtt/pull/676))
* [nathang21](https://github.com/nathang21): availability after Home Assistant
  restarts ([#720](https://github.com/wez/govee2mqtt/pull/720))
* [knows-cloud](https://github.com/knows-cloud): per-model scene encoding
  ([#716](https://github.com/wez/govee2mqtt/pull/716))
* [PKizzle](https://github.com/PKizzle): MQTT reconnect
  ([#730](https://github.com/wez/govee2mqtt/pull/730))
* [paulsmitz1](https://github.com/paulsmitz1): AWS IoT recovery
  ([#723](https://github.com/wez/govee2mqtt/pull/723))
* [abhishekgovande](https://github.com/abhishekgovande): color temperature
  ranges ([#743](https://github.com/wez/govee2mqtt/pull/743))
* [carloscae](https://github.com/carloscae): re-registration handling
  ([#711](https://github.com/wez/govee2mqtt/pull/711))
* [snoack](https://github.com/snoack): Offline status
  ([#718](https://github.com/wez/govee2mqtt/pull/718))
* [jj33](https://github.com/jj33): air quality monitors
  ([#648](https://github.com/wez/govee2mqtt/pull/648))
* [fpedrei](https://github.com/fpedrei): web listen address
  ([#697](https://github.com/wez/govee2mqtt/pull/697))

Others whose work arrived through `govee2mqtt-extended`:

* [Paulo Silveira](https://github.com/peas): IoT work mode and LAN music
  palettes
* [jasonjrowe](https://github.com/jasonjrowe): lights without color modes
  ([wez/govee2mqtt#663](https://github.com/wez/govee2mqtt/pull/663)) and effect
  filtering
* [adaofeliz](https://github.com/adaofeliz): effect list options
  ([wez/govee2mqtt#662](https://github.com/wez/govee2mqtt/pull/662))
* [sake1011](https://github.com/sake1011): 2FA login support
* [theg1nger](https://github.com/theg1nger): UTF-8 fix
  ([wez/govee2mqtt#606](https://github.com/wez/govee2mqtt/pull/606))

Foundations:

* [Govee LAN Control](https://github.com/wez/govee-lan-hass/), Wez's earlier
  work that the bridge builds on
* [homebridge-govee](https://github.com/bwp91/homebridge-govee/) by
  [bwp91](https://github.com/bwp91), which made the AWS IoT support possible

## Support the original author

Building and supporting software like this takes significant effort. To say
thanks to Wez, who wrote the bridge:

* [Sponsor Wez on GitHub](https://github.com/sponsors/wez)
* [Sponsor Wez on Patreon](https://patreon.com/WezFurlong)
* [Sponsor Wez on Ko-Fi](https://ko-fi.com/wezfurlong)
* [Sponsor Wez via Liberapay](https://liberapay.com/wez)

