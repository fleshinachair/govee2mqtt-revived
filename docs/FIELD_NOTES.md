# Field notes from a long-running install

Contributed by [Paulo Silveira](https://github.com/peas) in pull request
[#4](https://github.com/fleshinachair/govee2mqtt-revived/pull/4).

Observations from one home that has run this code line daily since July 2026
(first `wez/govee2mqtt`, then `florianhorner/govee2mqtt-extended`, in Docker
next to Home Assistant, on an account with two-step verification). Lights:
H6009, H6020, H607C, H60B0, H60B2, H61E5, H7025, H8057. The `set-music-palette`
topic and the IoT work-mode tracking in this tree came out of that install.

Each item was seen on that hardware. None is confirmed on this fork unless it
says so, so treat these as leads to check rather than facts. The author's
byte-level write-up is at
<https://paulo.com.br/skills/govee-lan-protocol/SKILL.md>.

## Login and 2FA

- The emailed code had **4 digits**, with the code in the subject line. Do not
  validate or document a fixed length of 6.
- On a phone Govee already trusts, the app signs in with no challenge and sends
  no email. "Sign in to the app to get a code" leads nowhere; only an explicit
  verification request sends one.
- A code from the web store login belongs to a different client and always
  fails with 455.
- A consumed code left in `GOVEE_2FA_CODE` is replayed whenever the cached
  login is lost, fails with 455, and aborts startup. Purge Caches was the
  trigger (florianhorner/govee2mqtt-extended#70); here it now keeps the login.
  Anything that drops
  `undoc-api`/`account-info` costs the user a trip to their inbox, so treat
  that entry as user state, not as a cache.

## A success reply is not proof

- The Platform API can answer `success` for a command the device never
  receives. On the H8057, `musicMode` value 0 (Rolling) does that every time,
  while value 1 works at once. Confirm with the IoT echo or a LAN read-back
  before advertising a mode.
- LAN colour and brightness commands produce no IoT push; the new state shows
  up only on the next LAN poll. An automation that reads state right after a
  command reads the old one.
- A reported `mode` can be old data with a fresh timestamp. That is why the
  work-mode observation time is tracked separately.
- On one H6020, turning off over the LAN left the state stale for hours. Over
  the Platform API it came back by poll and IoT.

## Music mode over the LAN

- Style ids and the parameter bytes after the colours are **per SKU**. The same
  id is a different style on another model, so a profile in `src/music.rs`
  must come from that model: a capture, or the id the cloud echoes.
- `a3 41 <style>` is one frame with three uses: the cloud echo that reveals a
  style id, the LAN delivery receipt, and the BLE upload ack.
- The H7025 takes 17-byte `a3` fragments. The 8-byte slicing seen in app
  captures is the app's habit, not a device limit.
- LAN and Platform music sensitivity are separate slots on the device.
  Setting one does not change the other.

## Scenes over the LAN

From pull request
[#5](https://github.com/fleshinachair/govee2mqtt-revived/pull/5), whose H607C
and H60B0 rules are in `src/ble.rs`.

- How a scene's parameters must be framed depends on the model. A lamp can play
  a different animation, or ignore the upload, without any error.
- The H607C wants parameters starting `0a` sent as they are. Its scenes with no
  parameters work with the generic `02`. Confirmed on this fork: the maintainer
  reported effects working on a second H607C with `2026.11.27.1931`.
- The H60B0 wraps them as `51`, a two-byte length, an `0a` block, then a second
  length-prefixed block. Dropping the first three bytes matched the cloud by eye
  on one scene. What the second block does is not known.
- On four `sceneType` 2 models the generic `02` looks right: sending the same
  scene through the cloud afterwards made no visible change (H61E5 "Forest" and
  "Fire", H60B2 "Forest", H8057 "Ocean", H7025 "Christmas"; judged by eye).
- Some libraries give one name two `lightEffects`, which the Platform API lists
  as two scenes (H7025 "Christmas-A" and "Christmas-B"). Choosing by name over
  the LAN gives the first.
- Not tried: the H6020 (`sceneType` 5, starting `50`) and the H6009 (type 1).

## Devices

- A device added to the Govee account while the bridge was running (H61E5,
  then H8057) got no entities until a restart, on a build without
  `watch_for_late_devices`. Not retested here.
- White ranges outside what the Platform API declares for a model black the
  light out instead of clamping (H60B0). Prefer the declared range in a quirk
  over the generic 2000-9000 K.
