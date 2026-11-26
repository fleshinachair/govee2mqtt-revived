# Govee to MQTT (Revived): first pre-release

Based on [wez/govee2mqtt](https://github.com/wez/govee2mqtt) `2026.03.25-ab9deb66`.

**This is a build for testers.** It passes its automated tests, but this fork has
not confirmed any change against a real Govee account or device. Take a Home
Assistant backup first. Moving from wez's add-on: see
[the migration steps](README.md#migrating-from-wezgovee2mqtt-zero-entity-loss).

References such as `wez#739` are issues and pull requests in
[wez/govee2mqtt](https://github.com/wez/govee2mqtt).

## Fixed on top of govee2mqtt-extended

| Problem | Upstream |
|---|---|
| Tap-to-Run shortcuts rejected by Govee since early November 2026 | PR wez#739 by UPEngineer (wez#737, wez#745) |
| Startup failure when a device sends translated option names | PR wez#645 by Raul-7-7 (wez#549, wez#565, wez#578) |
| Startup failure on `pactType: -1` | wez#732 |
| All effects lost when a model has no scene list | wez#722 |
| Repeated logins, and possible 24-hour lockout, when the cache cannot be written | PR wez#703 by Nick Greenway (wez#702) |

## Brought in from govee2mqtt-extended

32 commits by Florian Horner, Paulo Silveira and others. See
[Credits](README.md#credits).

* **Two-factor authentication:** the bridge requests the emailed code and
  accepts it in `govee_2fa_code` (aimed at wez#647, wez#682, wez#627, wez#633)
* **Fewer crashes:** a device missing from state no longer takes down the MQTT
  integration, and an unexpected exit now restarts the add-on (wez#617, wez#618)
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

## Not yet confirmed

* Whether 2FA login works for the reporters of the issues above.
* Whether wez#722's reporter (H6038) gets effects back: the remaining sources
  decide that.
* Purifier PM2.5 and air quality readings (wez#667, wez#369).

## Known problems still open

* The bridge exits at startup if Govee's servers cannot be reached (wez#76).
* It crashes at startup if the cache database cannot be opened (wez#536).
* Most missing device models are not added yet.

## Held for the next release

Open upstream pull requests: wez#751, wez#595, wez#687, wez#676, wez#720,
wez#716, wez#730, wez#723, wez#743, wez#711, wez#718, wez#648, wez#697, and the
feature requests listed in [next_step.md](next_step.md).

## Not taken from govee2mqtt-extended

Their continuous integration, release tooling, project documents and branding.
The CI setup here borrows their add-on build approach.

## How it was checked

`cargo test` and `cargo clippy --all-targets` ran before every code commit, on
Windows. The release workflow runs the tests again on Linux and publishes
nothing if they fail.
