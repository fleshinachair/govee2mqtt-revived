# Govee to MQTT (Revived)

Rust bridge between Govee devices and Home Assistant over MQTT. Ships as a
standalone container image and as a Home Assistant OS (HAOS) add-on. Binary and
crate name: `govee`.

The roadmap and current execution checklist are in `next_step.md`.

| Remote | Repository | Role |
|---|---|---|
| `origin` | `fleshinachair/govee2mqtt-revived` | This fork |
| `upstream` | `wez/govee2mqtt` | Original project |
| `extended` | `florianhorner/govee2mqtt-extended` | Sibling fork we cherry-pick from |

The tree is upstream `6f5a844`, plus 32 commits picked from `extended`, plus
open upstream pull requests and fixes made here. See `next_step.md` for release
status.

## Engineering rules

These are strict. A change that breaks one is not ready to commit.

### 1. Never panic on runtime data

- Never introduce `.unwrap()` or `.expect()` on anything derived from the
  network or from JSON: HTTP responses, LAN UDP packets, MQTT topics and
  payloads, AWS IoT messages, cache contents, or user configuration.
- Propagate with `anyhow::Result` and add context (`.context(...)`), or log and
  skip the one bad item. One malformed device must not take down the others.
- The same applies to `panic!`, `unreachable!`, `todo!`, slice indexing, and
  string byte-slicing on external text. Device and preset names are often
  non-ASCII; byte-slicing them has crashed this bridge before.
- `.unwrap()` and `.expect()` are fine in `#[cfg(test)]` code.
- The only non-test `expect` left is the last-resort in-memory cache in
  `cache.rs`. It is not precedent.

### 2. Gate every commit

Run all three before every commit, and do not commit if any fails:

```sh
cargo check
cargo test
cargo clippy --all-targets
```

CI (`.github/workflows/pr.yml`) also runs `cargo fmt --all -- --check` and the
all-features test and clippy runs, so run
`cargo fmt --all` before pushing. If the gate cannot run on your machine, say so
and stop. Do not commit unverified Rust.

### 3. Small, atomic, traceable commits

- One logical change per commit. Keep refactors and formatting separate from
  behaviour changes.
- Reference the upstream PR or issue in full form: `wez/govee2mqtt#739`. A bare
  `#739` links to the wrong thing in this fork.
- For code taken from elsewhere, keep the original author
  (`git cherry-pick`, or `git commit --author=...`) and record the source
  (`Cherry-picked-from: florianhorner/govee2mqtt-extended@<sha>`).
- Pushing a commit that mentions `wez/govee2mqtt#N` adds a visible backlink on
  that upstream item.

### Also

- Never log passwords, API keys, tokens, 2FA codes, IoT keys, or raw
  authenticated response bodies. `addon/run.sh` redacts env vars by name
  pattern; extend the pattern when you add a secret.
- A new setting must be wired through every layer: clap arg or env var in Rust,
  `addon/config.yaml` schema, `addon/run.sh`, `addon/translations/en.yaml`,
  `docs/CONFIG.md`.
- Keep `LICENSE.md` and upstream attribution intact.

## Build and test

```sh
cargo build
cargo test                    # includes k9 snapshot tests over test-data/
cargo run -- serve            # needs MQTT settings; see docs/CONFIG.md
cargo run -- lan-disco        # other subcommands: list, lan-control, undoc, ...
```

- Native code is compiled from source (vendored OpenSSL via `mosquitto-rs`,
  `aws-lc-sys`, bundled SQLite, libmosquitto). A Rust install alone is not
  enough. A first build can take 25 minutes or more.
- `make test` uses `cargo nextest` and `make fmt` uses nightly rustfmt. Neither
  is required; the commands above are enough. Stable `cargo fmt` warns that it
  cannot set `imports_granularity`; that is harmless.
- `.gitattributes` forces LF. With CRLF line endings the snapshot tests fail. A
  checkout made before that file existed needs its files checked out again.

### Linux and WSL2

Closest to CI. Install a stable Rust toolchain, a C toolchain, `cmake`, `perl`
and `pkg-config`, then run `cargo` directly.

### Native Windows (verified: Rust 1.100.0, MSYS2 GCC 16.2)

Uses MSYS2 MinGW-w64 and the GNU toolchain. Visual Studio is not needed.

One-time setup, from PowerShell in the repository root:

```powershell
winget install MSYS2.MSYS2
C:\msys64\usr\bin\bash.exe -lc "pacman -S --needed make perl mingw-w64-x86_64-{gcc,cmake,nasm,pkgconf,make}"
rustup toolchain install stable-x86_64-pc-windows-gnu -c clippy -c rustfmt
rustup override set stable-x86_64-pc-windows-gnu
```

Then run every `cargo` command through the wrapper:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\win-gnu-cargo.ps1 check
powershell -ExecutionPolicy Bypass -File scripts\win-gnu-cargo.ps1 test
powershell -ExecutionPolicy Bypass -File scripts\win-gnu-cargo.ps1 clippy --all-targets
```

The wrapper sets `PATH`, forces the MinGW `gcc` linker, and adds the
`liblibssl.a` and `liblibcrypto.a` aliases that `libmosquitto-sys` asks for.
Use it consistently: switching the launch environment rebuilds OpenSSL. On a
clean tree the first run fails once at the link step and retries by itself.

A Windows build says nothing about Linux-only behaviour. CI is the check for
that.

### Test data

- Snapshot tests live in `src/__k9_snapshots__/`. Review snapshot diffs before
  accepting them (`K9_UPDATE_SNAPSHOTS=1 cargo test`).
- For a new payload shape, add a sanitized fixture to `test-data/` and a test.
  Strip device IDs, tokens, IPs, and email addresses.
- A test that builds a mosquitto client must wrap it in `ManuallyDrop`.
  Destroying one hangs on Windows GNU builds.
- A passing run must show passing tests. A killed `cargo` has exited 0.

## Architecture

### Data flow

```
Govee Platform API (HTTPS, API key) ─┐
Govee account API (HTTPS, login)  ───┤
AWS IoT (MQTT/TLS 8883)           ───┼─> service::State ─> hass_mqtt entities ─> local MQTT broker ─> Home Assistant
LAN API (UDP)                     ───┘          └──────── MQTT command router <──────┘
                                                └──> HTTP API + web UI (port 8056)
```

Startup order, in `src/commands/serve.rs`: Platform API discovery, then account
("undoc") discovery plus IoT, then LAN discovery (10 s wait), then the poll
loop, then the Home Assistant MQTT integration, then the HTTP server. Each API
is optional. If Govee cannot be reached (no connection, timeout or 5xx), the
last device list fetched in the past week is used, and AWS IoT starts once a
login succeeds. Otherwise, such as a wrong password or a 2FA prompt, a failed
first discovery exits the process (upstream #76). Cloud discovery repeats every 600 s.

### Source map

| Path | Responsibility |
|---|---|
| `src/main.rs` | CLI (clap), logging, `opt_env_var` with `*_FILE` secret support |
| `src/commands/` | Subcommands; `serve.rs` is the service |
| `src/service/state.rs` | Shared `State`: device map, clients, control dispatch, scenes |
| `src/service/device.rs` | `Device`: per-source info and state, capability decisions |
| `src/service/quirks.rs` | Per-SKU overrides |
| `src/service/hass.rs` | Local MQTT client, command router, HA registration |
| `src/service/iot.rs` | AWS IoT client and status subscriber |
| `src/service/coordinator.rs` | Per-device serialization of control, follow-up poll |
| `src/service/http.rs`, `assets/` | HTTP API and web UI |
| `src/hass_mqtt/` | One module per HA entity type, plus `enumerator.rs` |
| `src/platform_api.rs` | Official Platform API client and schema types |
| `src/undoc_api.rs` | Undocumented account API: login, rooms, IoT cert, one-clicks |
| `src/lan_api.rs` | LAN UDP discovery and control |
| `src/ble.rs` | BLE-format packet codec (not a BLE transport) |
| `src/rest_api.rs` | Client for the older REST API; nothing calls it |
| `src/cache.rs` | SQLite-backed response cache, with an in-memory fallback for failed writes |
| `src/music.rs` | Per-SKU music palette profiles for LAN |
| `src/sensitive.rs` | Redaction of secrets in logs and errors |
| `src/hass_mqtt/command_routes.rs` | The single list of command topics and routes |
| `src/service/platform_mqtt.rs` | Govee's cloud MQTT broker, for device events |
| `src/temperature.rs` | Temperature units and conversion |

### MQTT bridge

- `spawn_hass_integration` connects with `mosquitto-rs`; `run_mqtt_loop`
  dispatches commands and rebuilds subscriptions after reconnect.
- `rebuild_router` registers command topics under `gv2mqtt/`, plus the HA status
  topic `<discovery_prefix>/status`. Only `online` there re-registers entities.
- Availability is not retained, so it is re-published every 60 s.
- With `GOVEE_OFFLINE_UNAVAILABLE=true`, device entities also depend on a
  retained `gv2mqtt/<id>/availability`.
- The energy counter of a monitoring plug is saved in a retained
  `gv2mqtt/energy/<id>/counter`, which the bridge subscribes to itself.
- If the broker connection is not restored within 5 minutes the process exits
  with status 1 and relies on a supervisor to restart it.
- Entities implement `EntityInstance` (`publish_config`, `notify_state`).
  `enumerator.rs` builds them from device capabilities and quirks.
- Discovery configs go to `<discovery_prefix>/<component>/<unique_id>/config`.
  Availability is `gv2mqtt/availability`.
- Add command topics through `hass_mqtt/command_routes.rs`, which ties each
  advertised topic to a route. Tests enforce this.
- Fans and air purifiers also get a `fan` entity (`hass_mqtt/fan.rs`). It is
  additive: switches and work-mode entities remain.
- `unique_id` and topic shapes are user-facing API. Changing them breaks
  existing dashboards and automations.

### Govee clients

**Platform API** (`platform_api.rs`). Official, rate limited, authenticated by
API key. Source of capability metadata, scenes, segments, music modes, and,
over its own MQTT broker, device events. The API key is that broker's topic
name, so never log topics there.
Responses vary by SKU and change without notice, so parse leniently.

**Account API** (`undoc_api.rs`). Reverse engineered, authenticated by email and
password. Provides room names, device topics, the IoT certificate (PKCS#12), and
Tap-to-Run shortcuts. Govee changes it unilaterally: app version checks, 2FA,
and token scopes have each broken it.

**AWS IoT** (`service/iot.rs`). MQTT over TLS using the account certificate.
Pushes status and accepts control. Needs the account login. libmosquitto can
stop reconnecting, so a supervisor replaces a client that stays disconnected
for 60 s, backing off from 5 s to 5 min.

**LAN API** (`lan_api.rs`). UDP: multicast scan to `239.255.255.250:4001`,
replies on `4002`, commands to device port `4003`. Must be enabled per device in
the Govee app, and needs host networking. Status queries retry a bounded number
of times, and a per-device circuit breaker backs off unresponsive devices.

**2FA.** On login status 454 the bridge asks Govee to email a code, which the
user supplies as `GOVEE_2FA_CODE`. Do not negative-cache those failures
(`NoCacheError`).

**BLE**. There is no BLE client, and BLE-only devices are not controllable.
`ble.rs` encodes and decodes BLE-format frames that are carried over LAN or IoT
as `ptReal` commands (scenes, humidifier settings).

### State and control

- `Device` keeps LAN, IoT, and Platform data separately.
  `Device::device_state()` returns whichever has the newest `updated` time.
- Basic light and power control prefers **LAN, then IoT, then Platform**. Check
  the specific method in `state.rs`; not every operation follows this.
  - Scenes try the Platform API first unless `avoid_platform_api()`.
  - Target temperature is Platform only.
- The poll loop ticks every 30 s. It queries LAN status each tick, and IoT or
  Platform only when state is older than the poll interval (900 s; 60 s for a
  kettle that is on).

- A light `transition` runs as a background task. Each control request bumps a
  per-device generation, which ends the ramp. Never hold the device permit
  across a long wait: it blocks that device's command queue.

### SKU and quirks system

`service/quirks.rs` holds a static table keyed by SKU. It corrects or fills in
what Govee's metadata gets wrong.

- Constructors: `Quirk::light` (RGB, brightness, 2000-9000 K, IoT on),
  `Quirk::lan_api_capable_light` (the same plus LAN), `Quirk::device`,
  `humidifier`, `space_heater`, `thermometer`, `ice_maker`.
- Modifiers include `with_color_temp_range`, `with_iot_api_support`, and
  `with_platform_temperature_sensor_units`; see the file for the rest.
- A device with no quirk gets **no IoT control** (`iot_api_supported()` is
  false), so commands go over LAN or the slower Platform API.
- A quirk's explicit color temperature range overrides the Platform API. The
  generic `DEFAULT_COLOR_TEMP_RANGE` yields to a usable Platform API range,
  unless the quirk avoids the Platform API.
- Some model-specific behaviour is keyed on SKU strings elsewhere, for example
  in `hass_mqtt/work_mode.rs`.

To add a SKU: add the entry, base each flag on evidence (device metadata or a
reporter's log), reference the issue, and update `docs/SKUS.md` if it lists the
device family.

### Cache

SQLite file `govee2mqtt-cache.sqlite` in `GOVEE_CACHE_DIR` or the user cache
directory (`/data` in containers). It holds login tokens and API responses with
soft, hard, and negative TTLs, so treat it as secret. If it cannot be opened
the bridge logs an error and uses an in-memory cache. If a write fails the value
is kept in memory, and memory is read before disk.

### HTTP server

Port 8056 on all interfaces by default (`HTTP_ADDR`, `HTTP_PORT`), **no
authentication**, and it can control devices. Do not expose it
beyond the local network.

## Build and release pipeline

### Standalone image

1. `scripts/build-cross.sh <platform>` uses `cross` to build a static musl
   binary for `linux/amd64` or `linux/arm64` into `docker-target/<platform>/`.
2. `Dockerfile` copies that prebuilt binary into `distroless/cc-debian12` with
   `assets/` and `AmazonRootCA1.pem`. It does not compile Rust, so
   `docker build .` alone fails without step 1.

### HAOS add-on

- `addon/Dockerfile` copies the binary out of a **published standalone image**
  (`ARG GOVEE_IMAGE`) into a Home Assistant Debian base. A release pins it to
  that release; a local build uses `:latest`, not your working tree.
- `addon/config.yaml` defines the image name, version, options schema, and
  `host_network`. Supervisor pulls the exact `version` string as the image tag.
- `addon/run.sh` maps add-on options to `GOVEE_*` env vars, waits for the MQTT
  broker, then runs `govee serve`.
- `addon/DOCS.md` is what users see on the add-on's Documentation tab.
- `repository.yaml` makes this repo installable as an add-on repository.

### CI and releases

| Workflow | Does |
|---|---|
| `pr.yml` | Format, tests, clippy. Called by `build.yml`; also runs on pushes to `community-revival` |
| `build.yml` | After tests: cross-compiles and publishes images; builds the add-on on tags |
| `no-response.yml` | Closes issues awaiting a reply |

Image names under `ghcr.io/fleshinachair/`:

- `govee2mqtt-revived:latest`, `:main`, `:standalone-<tag>`: standalone
- `govee2mqtt-revived:<tag>`: multi-arch add-on, which Supervisor pulls
- `{amd64,aarch64}-govee2mqtt-revived`: per-arch add-on builds

Never give a standalone image a bare release tag; it would collide with the
add-on's. Release steps are in `next_step.md`.

`scripts/tag-release.sh` creates a `YYYY.MM.DD-<sha8>` tag, writes that version
into `addon/config.yaml` (`apply-tag.sh`), regenerates `addon/CHANGELOG.md` with
git-cliff, and commits.
