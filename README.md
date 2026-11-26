# Govee to MQTT bridge for Home Assistant

This repo provides a `govee` executable whose primary purpose is to act
as a bridge between [Govee](https://govee.com) devices and Home Assistant,
via the [Home Assistant MQTT Integration](https://www.home-assistant.io/integrations/mqtt/).

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
|Segment Color|API Key|Find the `Segment 00X` light entities associated with your main light device in Home Assistant|

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

