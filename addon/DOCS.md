# Govee to MQTT (Revived)

A bridge between [Govee](https://govee.com) devices and Home Assistant, using
the [MQTT integration](https://www.home-assistant.io/integrations/mqtt/).

**This is a pre-release for testers.** Take a Home Assistant backup first, and
report problems in
[Issues](https://github.com/fleshinachair/govee2mqtt-revived/issues).

## Setup

1. Install the **Mosquitto broker** app, start it, and accept the MQTT
   integration under **Settings → Devices & services**.
2. On this app's **Configuration** tab, fill in your Govee email, password and
   [API key](https://developer.govee.com/reference/apply-you-govee-api-key).
   All three are optional, but without them you only get devices with the LAN
   API enabled.
3. Start it and check the **Log** tab.

Every setting is described in the
[configuration reference](https://github.com/fleshinachair/govee2mqtt-revived/blob/main/docs/CONFIG.md).

## Migrating from `wez/govee2mqtt` (Zero Entity Loss)

Your existing `light.*`, `switch.*`, `select.*`, `fan.*` and `sensor.*`
entities, with their automations and history, carry over. Home Assistant
identifies them by `unique_id`s derived from each device's hardware ID, and
this app keeps the same ones. It only adds entities.

**Warning: never run both at the same time.** Two bridges publishing the same
devices to the same MQTT broker will fight over every entity, and both want
port 8056.

1. Open the old add-on's **Configuration** tab, choose **⋮ → Edit in YAML**, and
   copy everything.
2. **Stop** the old add-on. **Do not uninstall it** yet. Also turn off its
   **Start on boot** and **Watchdog** switches.
3. Install **Govee to MQTT (Revived)**.
4. On its **Configuration** tab choose **⋮ → Edit in YAML**, paste, and save.
5. **Start** it and check the log.

To go back, stop this app and start the old one.

### Docker Compose

1. `docker compose down` (without `-v`).
2. Change the image to `ghcr.io/fleshinachair/govee2mqtt-revived:latest`. Keep
   your `.env` file and any volumes.
3. `docker compose up -d`

### What to expect

* **2FA accounts:** this app starts with an empty cache, so Govee may ask for a
  new verification code (see below).
* **Sensor templates:** some sensors, such as air quality and filter life, used
  to report `{"value": 6}` and now report `6`. Remove any template that
  unwrapped the old form.
* **New entities:** fans and air purifiers also get a `fan` entity, lights get
  scene next/previous buttons and a scene info sensor, and lights with music
  modes get a sensitivity slider.

## Two-factor authentication

1. Start with `govee_2fa_code` empty. Govee emails you a code.
2. Paste it into `govee_2fa_code`. Codes expire after about 15 minutes.
3. Restart. After login succeeds, remove the code from the configuration.

## Reporting a problem

Include your device model (such as H6159), the version shown on this app's
**Info** tab, and the log. Check the log for personal details before posting.

## Credits

Wez Furlong wrote [govee2mqtt](https://github.com/wez/govee2mqtt). Much of what
this app adds was developed in Florian Horner's
[govee2mqtt-extended](https://github.com/florianhorner/govee2mqtt-extended).
Everyone is credited in the
[README](https://github.com/fleshinachair/govee2mqtt-revived#credits).
