# Govee to MQTT (Revived): thirteenth pre-release

Based on [wez/govee2mqtt](https://github.com/wez/govee2mqtt) `2026.03.25-ab9deb66`.

**This is a build for testers.** It passes its automated tests, but very little
has been tried for real: earlier pre-releases ran on one setup with three lights
on the LAN, and one contributor has since signed in to a real Govee account with
two-step verification. On this build, what has been reported back is that
effects work on an H607C controlled over the LAN, and that the web page matches
the Home Assistant theme.
Take a Home Assistant backup first. Moving from wez's add-on: see
[the migration steps](README.md#migrating-from-wezgovee2mqtt-zero-entity-loss).
What to try, and how to report it: [TESTING.md](TESTING.md).

**Needs Home Assistant 2025.2 or newer.**

**Turn on the add-on's Watchdog switch.** The bridge now exits on purpose when it
cannot recover by itself, and relies on being restarted.

References such as `wez#739` are issues and pull requests in
[wez/govee2mqtt](https://github.com/wez/govee2mqtt).

## New since the twelfth pre-release

* **Music colour palettes on the H60B2** floor lamp: Bloom, Rhythm and
  Stippling, by [Paulo Silveira](https://github.com/peas)
  ([#6](https://github.com/fleshinachair/govee2mqtt-revived/pull/6)). Needs the
  [`music_palette`](docs/MUSIC_MODE.md) setting and LAN control. Paulo sent the
  same data for Bloom to the lamp by hand and all three sections took the
  colours; this build has not been tried on it, and Rhythm and Stippling were
  not re-run.

## New since the eleventh pre-release

Both fixes are by [Paulo Silveira](https://github.com/peas), who found the cause
on their own lamps
([#5](https://github.com/fleshinachair/govee2mqtt-revived/pull/5)). They matter
if you have **no Govee API key**: effects are then sent over your network. With
a key they go through Govee's cloud, which was not affected.

* **H607C: an effect played something other than the one chosen.** For example,
  "Fire" showed blue with a white band. Sent the new way, "Fire" and "Forest"
  looked right.
* **H60B0: the same fault.** Sent the new way, "Ocean" looked like Govee's
  cloud version. Only that scene was compared.

Paulo tried the new data with a script that builds the same bytes. The
maintainer has since reported effects working on their own H607C with
`2026.11.27.1931`; the H60B0 has not been tried with a released build.
Choose the effect again after updating. The H6020 lays out
its effects differently again and may have a similar fault; it has not been
tried.

Also: Paulo confirmed on a two-step verification account that upgrading from
govee2mqtt-extended and pressing **Purge Caches** did not ask for a new code
(tried on the ninth pre-release).

## New since the tenth pre-release

* **The web page matches your Home Assistant theme.** Opened inside Home
  Assistant, it takes your theme's background and text colours and changes with
  it. **Light** and **Dark** still override this, and the browser remembers your
  choice (if you picked one on the tenth, pick it again). Opened directly on port
  8056, it follows your device's setting. Confirmed since by the maintainer in
  their Home Assistant.

## New since the ninth pre-release

* **Light and dark themes on the web page.** A **Theme** picker offers Follow
  system, Light and Dark, and the browser remembers the choice. Follow system,
  the default, uses your device's setting, not the theme chosen in Home
  Assistant. If you do not see the picker, reload the page.

## New since the eighth pre-release

The first two changes are by [Paulo Silveira](https://github.com/peas), who runs
these lights every day.

* **Purge Caches no longer signs you out of Govee.** With two-step verification,
  pressing it could leave the bridge unable to start until you entered a new
  code. If you change the account email, the bridge signs in again by itself
  ([#1](https://github.com/fleshinachair/govee2mqtt-revived/issues/1),
  [#2](https://github.com/fleshinachair/govee2mqtt-revived/pull/2)).
* **Settings for more lights:** H6020, H607C, H60B2, H7025 and H8057 are added,
  and H60B0 gains its white range
  ([#3](https://github.com/fleshinachair/govee2mqtt-revived/pull/3)). The white
  range is 2200-6500 K on the H607C and 2700-6500 K on the others, so check any
  automation that asks for a warmer or cooler white than that. These models can
  now also be controlled over AWS IoT, with a Govee account, when they cannot be
  reached on your network. The ranges for H60B0 and H60B2 agree with what other
  users' devices reported to Govee; the rest rest on the contributor's lights.
* **[Field notes](docs/FIELD_NOTES.md)** from the same contributor, for anyone
  working on the bridge
  ([#4](https://github.com/fleshinachair/govee2mqtt-revived/pull/4)).

## New since the seventh pre-release

* **A light that is slow to answer at startup is no longer left unavailable.**
  The bridge waits 10 seconds for lights on your network. One found after that
  was never sent to Home Assistant, so it stayed unavailable until Home
  Assistant restarted or you pressed **Purge Caches**. It is now added by
  itself, 15 to 30 seconds after the bridge finds it. Other devices' entities
  are not sent again. A command sent before the light is found still fails.

## New since the sixth pre-release

* **The web page opens inside Home Assistant.** **Open web UI** now shows it
  behind your Home Assistant sign-in, whatever `http_port` is set to, with an
  optional **Govee** sidebar entry. The page on port 8056 is still there; set
  `http_addr` to `127.0.0.1` to close it to your network (wez#306). **This has
  had very little testing.** If it shows a blank page or "forbidden", use port
  8056 and please report it.
* **Ignore devices:** the new `ignore_devices` setting takes device IDs, models
  or names to leave out of Home Assistant, such as groups you no longer have.
  After a restart, delete them under the MQTT integration and they stay
  deleted (wez#333, wez#436, wez#721).

## New since the fifth pre-release

All 324 open issues in wez/govee2mqtt have been sorted into
[what is and is not fixed](docs/STATUS.md), and the ones that could be worked
on without the device have been. **None of the changes below has been tried on
the hardware it is for.** Each new setting is off until you set it.

### Fixes

| Problem | Upstream |
|---|---|
| Home Assistant rejects a number slider, or the value shown on it | wez#154, wez#171 |
| An H61C3 stays off when given a brightness, colour or effect | wez#521 (reported there for an H618C, which is unchanged) |
| H713C temperatures read in the wrong unit | wez#136, wez#307 |
| H600A and H61E6 get no controls | wez#191, wez#515 |

### Changes for your device

* **Older lights that accept commands and ignore them:** the new setting
  `no_iot_control` takes models or device IDs to control without AWS IoT. Needs
  the API key (wez#113, wez#125, wez#152, wez#319, wez#340, wez#364).
* **Heaters:** a new **Auto Stop** switch. While it is on, setting the target
  temperature from Home Assistant asks for Auto Stop too. It does not show
  what the Govee app is set to, as Govee does not report that (wez#84,
  wez#131).
* **H604A** now uses AWS IoT, as commands sent the other way were sometimes
  lost. If it gets worse, add `H604A` to `no_iot_control` (wez#740).

### Features

* A **Battery** sensor for devices whose Govee account entry has a level
  (wez#646, wez#668).
* `sensor_poll_interval` sets how often sensors are read, from 60 to 900
  seconds (wez#672).
* `mqtt_tls` and `mqtt_ca` connect to your MQTT broker over TLS. Tested against
  a simulated broker, not a real one. No client certificates (wez#257).
* The [MQTT topics](docs/MQTT_TOPICS.md) are documented (wez#102).

## New since the fourth pre-release

Only the version numbers changed, from `2026.11.27-b2a4425d` style to
date and time, such as `2026.11.27.1130`. Home Assistant could not tell which of
two releases made on the same day was newer, so it showed "Up-to-date" and
greyed out **Update**. The features are the same as the fourth.

## New since the third pre-release

* Entities now come back after a Home Assistant restart on setups that announce
  themselves on `hass/status` and not `homeassistant/status`. Before, the
  bridge never heard the restart and everything stayed `unavailable` until the
  add-on was restarted too.
* Light colour temperature is exchanged with Home Assistant in Kelvin, so limits
  are exact (wez#731). This is why Home Assistant 2025.2 is now needed.
* 35 light models seen on the LAN in upstream reports are now in the model
  table. Where those reports showed a 2700-6500 K range, it is used, which
  should stop LAN-only setups sending values that leave a light dark (wez#742).
* The web page has **Report a problem**: it builds a report without names, IDs
  or addresses for you to check and attach to a GitHub issue. Nothing is sent
  automatically.
* When Govee says there are too many requests, status polling now pauses
  instead of retrying every 30 seconds (wez#542).
* One Tap-to-Run entry that cannot be read no longer hides the others, and one
  with nothing that can be sent says so in the log.

## New since the second pre-release

A `transition` shorter than 2 seconds sent with a brightness delayed the change
by a second, and a light that was off came on dim first. Home Assistant can send
such values with an ordinary turn-on. They now apply at once. Found by reading
the code.

The README now states that Claude, Anthropic's AI model, primarily maintains
this repository.

## New since the first pre-release

**None of this has run against a real Govee device or account yet.**

### Fixes

| Problem | Upstream |
|---|---|
| Bridge stops at startup during a Govee outage | wez#76 |
| Crash when the cache database cannot be opened | wez#536 |
| One malformed device or capability fails the whole device list | wez#538 |
| Zone and DreamView switches always show unknown; they now show what you last set | wez#712, wez#719 |
| Offline devices still look controllable (new option, off by default) | wez#551 |
| Brokers that reject the MQTT client id | wez#659, wez#661 |

### Features

| Feature | Upstream |
|---|---|
| `transition` (fade) on lights controlled over the LAN | PR wez#726 by Jacob Hausler |
| Device events, such as water empty or presence | PR wez#693 by Gary T. Giesen (wez#343) |
| Local per-segment control on H6061 and H6066 | PR wez#752 by pixelowaty |
| H5086 plug energy monitoring | PR wez#717 by Sebastian Noack (wez#416, wez#186) |
| H717x kettle sensors | PR wez#734 |
| H1270 main and halo lights | PR wez#715 by Daniel Dubrovski |
| H6098 Video Mode switch | from PR wez#690 by kencarsick |
| H6006, H600D, H601B, H6039 and H616D | wez#621, wez#605, wez#577 |

Several pull requests were changed before being taken; each commit says how.

### New options

`hass_discovery_prefix` (wez#116), `offline_unavailable` and
`disable_platform_mqtt`. See [docs/CONFIG.md](docs/CONFIG.md).

## Fixes

### Startup, login and scenes

| Problem | Upstream |
|---|---|
| Tap-to-Run shortcuts rejected by Govee since early November 2026 | PR wez#739 by UPEngineer (wez#737, wez#745) |
| Startup failure when a device sends translated option names | PR wez#645 by Raul-7-7 (wez#549, wez#565, wez#578) |
| Startup failure on `pactType: -1` | wez#732 |
| All effects lost when a model has no scene list | wez#722 |
| Repeated logins, and possible 24-hour lockout, when the cache cannot be written | PR wez#703 by Nick Greenway (wez#702) |
| Scenes accepted but not applied on some models, such as the H6022 | PR wez#716 by knows-cloud (wez#324, wez#666) |

### Connections and availability

| Problem | Upstream |
|---|---|
| Everything stays `unavailable` after Home Assistant restarts | PR wez#720 by nathang21, PR wez#711 by carloscae (wez#572, wez#227, wez#110) |
| Bridge never reconnects after the MQTT broker goes away | PR wez#730 by PKizzle |
| Bridge hangs forever on an unanswered MQTT connection attempt | PR wez#687 by hartmood (wez#686) |
| Cloud (AWS IoT) control silently stops until a restart | PR wez#723 by paulsmitz1 |

### Devices

| Problem | Upstream |
|---|---|
| Light reports on but is dark at some color temperatures (H60C1) | PR wez#743 by abhishekgovande (wez#742) |
| Segment commands followed by a misleading whole-device update | PR wez#676 by aserper |
| Status sensor says Available for an offline device | PR wez#718 by snoack (wez#551, in part) |
| Air quality monitors H5140 and H5106 have no sensors | PR wez#648 by jj33 (wez#634, wez#561) |
| H605C takes 30-40 seconds to switch | PR wez#751 by TBE4034 |
| H61A9 has no LAN control | PR wez#595 by rutalreja-deloitte (wez#456) |

### New option

The web page's address and port can be set: `http_addr` and `http_port` in the
add-on, `HTTP_ADDR` and `HTTP_PORT` elsewhere (PR wez#697 by fpedrei,
wez#424, wez#200).

## Brought in from govee2mqtt-extended

32 commits by Florian Horner, Paulo Silveira and others. See
[Credits](README.md#credits).

* **Two-factor authentication:** the bridge requests the emailed code and
  accepts it in `govee_2fa_code` (aimed at wez#647, wez#682, wez#627, wez#633)
* **Fewer crashes:** a device missing from state no longer takes down the MQTT
  integration, and an unexpected exit is now reported as a failure (wez#617,
  wez#618)
* **Security:** login details are kept out of logs; vulnerable dependencies
  updated
* **Fans and purifiers:** native `fan` entities
* **Scenes:** next/previous buttons, a scene info sensor, icons and
  descriptions; the active scene stays selected while it animates
* **Effects:** options to disable or filter the list
* **Music:** sensitivity control; multi-colour palettes on H607C, H6020, H60B0
* **LAN:** fewer retries to devices that do not answer
* **Lights without color modes** are no longer dropped (wez#589, PR wez#663)
* **Device work mode** from AWS IoT (PR wez#704)
* **Web page:** status badges and room grouping
* **New model:** H60B0

## Changes you may notice

* The add-on has a new name and slug, so it installs alongside wez's. Run only
  one at a time.
* Entity `unique_id`s are unchanged. New entities are added.
* Sensors that reported `{"value": 6}` now report `6` (wez#369, wez#510).
* The new add-on starts with an empty cache; 2FA accounts may need a new code.
* Some lights offer a **narrower color temperature range**, now taken from
  Govee's data for that device. Check automations that set a very warm or very
  cool value.
* The Status sensor can now read **Offline**.
* If the MQTT broker is unreachable for **5 minutes**, the bridge exits so that
  it can be restarted.
* Scene data is now encoded differently for H6022, H607C, H60B0, H6065, H6066,
  H6052, H6092, H610A, H6079 and H70C4. Only the H6022, H607C and H60B0 changes
  are reported as tried on hardware. If scenes stopped working on one of these,
  please report it.

## Not yet confirmed

* Whether 2FA login works for the reporters of the issues above.
* Whether wez#722's reporter (H6038) gets effects back: the remaining sources
  decide that.
* Recovering a dead AWS IoT connection: only noticing that it died has been
  tested, against a simulated server on Windows. Replacing it has not run
  against Govee or on Linux.
* Air quality monitor and purifier readings (wez#634, wez#667, wez#369).

## Known problems still open

* The very first start still needs Govee to be reachable, and the saved device
  list lasts about a week without a successful fetch (wez#76).
* High CPU use after an MQTT broker restart has been reported (wez#749).
* Many missing device models are not added yet.
* A Home Assistant whose MQTT birth message uses a topic other than
  `<discovery prefix>/status` or `hass/status` is not heard restarting.

## Not in this build

Upstream pull requests wez#724 (scene speed) and wez#698 (H1310 fan), which
need testing on those devices first, and wez#736, for which a smaller fix was
made instead. What is and is not fixed is listed in
[docs/STATUS.md](docs/STATUS.md).

## Not taken from govee2mqtt-extended

Their continuous integration, release tooling, project documents and branding.
The CI setup here borrows their add-on build approach.

## How it was checked

The tests, formatting check and lint pass on Linux. The release workflow runs
them again and publishes nothing if they fail.
