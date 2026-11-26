# Execution plan

Snapshot taken **2026-11-26**. Re-fetch before acting.

| Source | State |
|---|---|
| `origin/main`, `community-revival`, `upstream/main` | all at `6f5a844` |
| `extended/main` | `f0d699d`; 84 commits ahead of upstream (6 are merges), 1 behind |
| Upstream open PRs | 43 |
| Upstream open issues | 324 |

**How much was checked.** "Applies" below means git applied the patch without
conflict in a throwaway clone. **Nothing was compiled or tested.** Issue-to-fix
mappings come from reading issue text and code, not from reproducing bugs.

---

## Status: first pre-release prepared, not published

Done on `community-revival`: Phase 0, all 32 Phase 1 picks, upstream PRs #739,
#645 and #703, fixes for #732 and #722, the rebrand, CI, and the docs. The rest
of Phases 2 and 3 waits until testers have this build.

### To publish (each step needs the owner's go-ahead)

1. Rename the GitHub fork to `govee2mqtt-revived`, then
   `git remote set-url origin https://github.com/fleshinachair/govee2mqtt-revived.git`.
2. Enable Issues.
3. `git push origin community-revival`. This runs the tests on Linux and
   publishes nothing. It is the first Linux build; fix what it finds.
4. Fast-forward `main` and push it. This publishes the standalone image.
5. Make the `govee2mqtt-revived` package public on GHCR.
6. Tag from `main` (Git Bash):

   ```sh
   TAG=$(git -c core.abbrev=8 show -s --format=%cd-%h --date=format:%Y.%m.%d)
   git tag "$TAG"
   ./scripts/apply-tag.sh
   git commit -am "Tag $TAG"
   git push origin "$TAG"   # wait for the whole workflow to succeed
   git push origin main     # only then advertise the version
   ```

   Do not use `scripts/tag-release.sh` for now: it needs Docker and regenerates
   `addon/CHANGELOG.md`, replacing the hand-written entry.
7. Create a GitHub release for the tag, marked **Pre-release**, using
   `RELEASE_NOTES.md`.
8. Install it on a real Home Assistant, migrating from wez's add-on, before
   inviting testers.

Until step 6 finishes, `addon/config.yaml` names a version that has no image.

## Needs manual review

- 2FA login, Tap-to-Run and every device behaviour: nothing has been run against
  a real Govee account or hardware.
- The migration guide's entity claims come from comparing `unique_id` formats
  and the enumerator in code, not from a live Home Assistant.
- The workflows have never run. YAML parses and the action tags exist.
- Tests that build an MQTT client must not drop it: that hangs on Windows GNU
  builds (`ManuallyDrop` in `service/hass.rs`).
- #739 invalidates the account login when the shortcut call returns 401. If
  Govee rejects only that endpoint for an account, this causes repeated logins.
  Watch tester logs, especially on 2FA accounts.
- #722 treats only a 400 as "no scenes". Other errors still fail the whole
  list, deliberately, so that a partial list is never cached on a device.

---

## Phase 0: working baseline (done)

Verified on `6f5a844` with no source changes, on native Windows with MSYS2 and
the GNU toolchain (Rust 1.100.0). Commands are in `CLAUDE.md`. Linux behaviour
is not verified locally; the first CI run will be the first Linux build.

- [x] `cargo check`
- [x] `cargo test`: 31 passed, 0 failed
- [x] `cargo clippy --all-targets`: 26 existing warnings, no errors (12 large
  `Err` variant, 10 needless borrow, 2 manual checked division, 1 loop counter,
  1 `sort_by_key`)
- [x] `cargo fmt --all -- --check`: clean
- [x] Commit `CLAUDE.md` and `next_step.md`

---

## Phase 1: cherry-picks from `extended/main`

### What you get

- 9 `.expect("device to exist")` panics removed; MQTT loop exit now returns 1 so
  HAOS restarts the add-on (#617, #618)
- 2FA login, verification-email request, v2 login, current app version
- Redaction of authenticated response bodies; lockfile security bumps
- H60B0 quirk (the only new SKU quirk in `extended`)
- Fan platform for fans and purifiers (H7124-specific handling)
- Scene next/previous buttons, scene catalog, scene persistence during animation
- Empty `supported_color_modes` fix; effect disable and allow-list options
- LAN retry bounds and per-device circuit breaker
- IoT work mode, music sensitivity, LAN music palettes (H607C, H6020, H60B0)
- Command-topic and route registry; device status badges in the web UI
- Real request UUIDs on Platform API calls
- Test functions go from 31 to 363 (`extended` reported 362 passing on default
  features and 363 with all features)

### Method

`extended/main` already contains everything in `upstream/main`: upstream's one
extra commit, `6f5a844`, is patch-identical to `extended`'s `645ddca`.

32 commits, in this order. With the helper below, all 32 applied with no manual
edits. The result differed from `extended/main` under `src/` only in two test
comments, one doc-comment typo, and one test name.

The helper takes only product paths (`src/`, `test-data/`, `assets/`, Cargo
files, add-on options, the 2FA test harness). It leaves docs, fork CI, branding,
and release metadata alone. It runs the commit gate before each commit.

**Order matters. Do not reorder or skip** without re-testing.

<!-- helper:start -->
```bash
KEEP='^(src/|test-data/|assets/|Cargo\.(toml|lock)$|addon/run\.sh$|addon/translations/|addon/config\.yaml$|scripts/(test_)?live_2fa\.py$)'
GATE=${GATE:-'cargo check && cargo test && cargo clippy --all-targets'}

# pick <sha> [ours|theirs|-] [upstream refs]
pick() {
  local sha=$1 opt=${2:--} refs=${3:-} f rc
  if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "STOP: tree not clean"; return 1
  fi
  [ "$opt" = - ] && opt=
  git cherry-pick -n ${opt:+-X "$opt"} "$sha" >/dev/null; rc=$?
  [ $rc -le 1 ] || { echo "STOP $sha: cherry-pick failed ($rc)"; return 1; }
  { git diff --name-only HEAD; git diff --name-only --diff-filter=U; } | sort -u | grep -Ev "$KEEP" |
    while IFS= read -r f; do
      if git cat-file -e "HEAD:$f" 2>/dev/null; then
        git restore --source=HEAD --staged --worktree -- "$f"
      else
        git rm -rfq --ignore-unmatch -- "$f"; rm -f -- "$f"
      fi
    done
  if [ -n "$(git diff --name-only --diff-filter=U)" ]; then
    echo "STOP $sha: resolve by hand, git add, then: finish_pick $sha '$refs'"; return 1
  fi
  if git diff --cached --quiet; then echo "empty $sha"; return 0; fi
  finish_pick "$sha" "$refs"
}

# finish_pick <sha> [upstream refs]: gate, then commit what is staged
finish_pick() {
  if ! eval "$GATE"; then
    echo "STOP $1: gate failed. Fix, git add, then: finish_pick $1 '$2'"; return 1
  fi
  git commit -q -C "$1" &&
    git commit -q --amend --no-edit \
      --trailer "Cherry-picked-from: florianhorner/govee2mqtt-extended@$1" \
      ${2:+--trailer "Refs: $2"} &&
    echo "ok $1"
}
```
<!-- helper:end -->

Paste the helper into a shell, then run one batch at a time. Chain with `&&` so
a stop halts the batch. After each batch, run `cargo fmt --all -- --check` and
`cargo test --all --all-features`.

Git often prints `error: could not apply ...` for conflicts in excluded docs
files. The helper discards those. Act only on a `STOP` line.

<!-- picks:start -->
- [x] **A. Crash fixes and H60B0**

```bash
pick f41ac85 &&                                   # H60B0 quirk
pick 261eb48 - "wez/govee2mqtt#617, wez/govee2mqtt#618"   # panics, exit(1)
```

- [x] **B. Scene cycling and lint cleanup**

```bash
pick 0666c35 &&          # scene quick-cycle
pick 2b87f42 ours &&     # clippy pass (conflicts with 6f5a844 in quirks.rs, work_mode.rs)
pick e4542b5 &&          # scene hints
pick 802c90d &&          # state.rs tests
pick ec1afeb ours        # clearer add-on text and messages; keep our config.yaml identity
```

- [x] **C. Auth, 2FA, security, effects**

```bash
pick ee0cfb4 - "wez/govee2mqtt#647" &&            # v2 login, 454 detection
pick f4351b8 ours &&                              # lockfile security bumps
pick d5f7e35 - "wez/govee2mqtt#647, wez/govee2mqtt#682" &&  # 2FA code
pick b0d3c71 &&                                   # clippy
pick 6049c16 - "wez/govee2mqtt#663, wez/govee2mqtt#662, wez/govee2mqtt#589"
```

- [x] **D. Scene correctness, request IDs, web UI**

```bash
pick b84d79d &&   # keep active scene during animation
pick 7d993f8 &&   # cache scene list
pick 48843cf &&   # scene icons and hints
pick b2c7737 &&   # real request UUIDs
pick e550412      # status badges, room grouping
```

- [x] **E. LAN resilience, IoT mode, music**

```bash
pick fbbbae4 &&   # LAN retry bound and circuit breaker
pick 5759ae0 - "wez/govee2mqtt#704" &&   # IoT work mode
pick 1962321 - "wez/govee2mqtt#704" &&   # work mode timestamp
pick 9e3d986 &&   # H60B0 is a floor lamp
pick d4819f1 - "wez/govee2mqtt#705" &&   # LAN music palette
pick 4f36df1 - "wez/govee2mqtt#705"
```

- [x] **F. Auth hardening**

```bash
pick 3c87648 - "wez/govee2mqtt#682" &&   # request the 2FA email
pick 0d459aa &&   # live 2FA harness (feature-gated)
pick 67f5ae0 &&   # withhold authenticated response bodies
pick 3c6c071      # lint
```

- [x] **G. Home Assistant entities**

```bash
pick 571e392 &&          # music sensitivity, palette option
pick e2b556e &&          # command-route parity
pick ec1a94a theirs "wez/govee2mqtt#369, wez/govee2mqtt#510, wez/govee2mqtt#667" &&  # fan platform
pick ca47245 &&          # H7124 nightlight, empty scenes
pick ad9756f &&          # only the async-trait bump survives the filter
git commit --amend -m "build: bump async-trait to 0.1.92" \
  -m "Cherry-picked-from: florianhorner/govee2mqtt-extended@ad9756f"
```
<!-- picks:end -->

### After the picks

- [x] Bring docs over and edit out `florianhorner` branding:
  `git checkout extended/main -- docs/CONFIG.md docs/LAN.md docs/MUSIC_MODE.md docs/SKUS.md docs/FAQ.md docs/LIVE_2FA_TEST.md`
- [x] Fix `LAN_HELP_URL` in `assets/components/devices.js`; it arrives pointing
  at `florianhorner/govee2mqtt-extended`.
- [x] Update `CLAUDE.md` for the new modules.
- [ ] Test 2FA against a real account (`docs/LIVE_2FA_TEST.md`).
- [ ] Smoke test on real hardware before releasing. `extended` states that
  several changes were not hardware tested.

### Deliberately not picked

| Commits | Reason |
|---|---|
| `6f2f5cc`, `2919b0e`, `edf2b54`, other `docs:` commits | Their fork's branding and docs |
| `46fe2c4`, `e15a58b`, `a83fb35`, `453edd7`, `6be08fe`, `71225fc`, `64942a0`, `b1cdd98` | Their version stamps and release tags |
| `c0a5985`, `3da8422` | Became empty in the trial (duplicate `261eb48` and `802c90d`) |
| `645ddca` | Already upstream |
| `da4aeb1`, `41cebe6`, `9b69eef`, `a97b0bd`, `cc56e8a` | Agent and workspace config |
| `6d4da34`, `715955a`, `37adfc4`, `5db31bb`, `08e7f7d`, `4d8cb4d`, rest of `ad9756f`, `f0d699d`, `e05c797`, `8df4bce`, `3f28296`, `f90e86b` | Their commit policy, proof logs, content guard, release notes |
| `bf75797`, `d1c23f7`, `3172c08`, `2dad7cf` | CI. Port by hand in Phase 4 |

**Cheaper alternative.** `git merge -X theirs extended/main` yields exactly the
`extended/main` tree in one step. You would then have to remove their branding,
policy workflows, proof logs, and release machinery by hand. Take it only if you
intend to track `extended` closely.

Their unmerged branch `feat/conditional-mqtt-route-guard` (2 commits, a test
that catches MQTT routes skipped at runtime) is worth a look later.

---

## Phase 2: upstream PRs

Apply one PR per commit, keeping the author:

```bash
N=739
git fetch upstream pull/$N/head
AUTHOR="$(git log -1 --format='%an <%ae>' FETCH_HEAD)"
git diff --binary "$(git merge-base upstream/main FETCH_HEAD)" FETCH_HEAD | git apply --3way --index
cargo check && cargo test && cargo clippy --all-targets &&
  git commit --author="$AUTHOR" -m "fix: <summary> (wez/govee2mqtt#$N)"
```

### 2a. Apply after Phase 1, in this order

All eight applied stacked on the Phase 1 result.

- [x] **#739** Tap-to-Run with the account token. Fixes #737 and #745; Govee has
  rejected the old token since early November 2026 and `extended` still uses it.
  **Highest priority.**
- [x] **#645** Accept localized (map) enum option names. Fixes #549, #565, #578:
  startup failure with H66A1 and similar. Needs `cargo fmt`. Its fallback
  stringifies unknown shapes; add a test with a fixture.
- [ ] **#751** H605C quirk, enabling IoT control.
- [ ] **#595** H61A9 LAN quirk. Closes #456.
- [ ] **#687** Timeout around the MQTT connect. Fixes #686.
- [ ] **#676** No follow-up poll after segment commands.
- [ ] **#720** Re-publish availability every 60 s. Targets #572, #227, #110.
- [ ] **#716** Per-model scene prefixes (H6022 and others). Targets #324, #666;
  check whether it helps #518. Its table is derived from a third-party reference,
  so only some models are likely to have been tested on hardware.

### 2b. Worth having; needs a manual port

- [x] **#703** Keep results when the cache write fails (#702: repeated logins
  led to a 24-hour account lockout). Conflicts in `cache.rs`, which `extended`
  rewrote. Port first.
- [ ] **#730** Exit if MQTT does not reconnect within 5 minutes. `hass.rs`.
- [ ] **#723** Recover when the IoT connection dies after a TLS EOF. `iot.rs`.
- [ ] **#743** Prefer the Platform API color temperature range (#742, and
  possibly #731, #558, #372). `device.rs`.
- [ ] **#711** Re-register only on HA `online`, debounced (#572). `hass.rs`.
  Review together with #720.
- [ ] **#718** Show Offline in the Status sensor (partial fix for #551).
  `sensor.rs`.
- [ ] **#648** Air quality monitor type, H5140 and H5106, CO2 sensor (#634,
  #561). Take only the device-type, quirk, and sensor parts. The PR also changes
  branding and panic handling.
- [ ] **#697** Configurable HTTP listen address (#424, #200). The code applies;
  expect a docs conflict if the Phase 1 docs are in.

### 2c. Features: decide after Phases 1-3

None of these was reviewed line by line. All conflict with the Phase 1 result.

| PR | Feature | Note |
|---|---|---|
| #736 | Persist device identity across outages (#76) | Large; best answer to offline startup |
| #752 | Local segment control, H6061 Hexa | Relates to #706 |
| #717 | H5086 energy monitoring | #416, #186 |
| #715 | H1270 control over IoT | |
| #734 | Kettle sensors | |
| #698 | H1310 fan speed and toggles | #713; overlaps the new fan platform |
| #693 | Cloud device events | #343, maybe #708 |
| #726 | Light `transition` support | #684 |
| #724 | Scene effect speed | Author says it is AI generated |
| #690 | H6098 video mode | Stacked on #689; rebase onto Phase 1 auth |

### Do not merge: superseded

| PR | Superseded by |
|---|---|
| #650, #652, #656, #689 | Phase 1 auth (batches C and F) |
| #663, #664 | Phase 1 |
| #704 | Phase 1 (`5759ae0`, `1962321`) |
| #651 | Phase 1 (`802c90d`) |
| #263 | Phase 1 fan platform |
| #564 | #645. #564 changes the field type without updating its users, so it likely does not compile |
| #548 | #736 (refreshed version) |
| #640, #641, #679, #680, #681 | Dependabot action bumps; handled in Phase 4 |
| #688 | Dependabot cargo bump; regenerate after Phase 1 |

---

## Phase 3: unaddressed high-impact issues

Neither Phase 1 nor a clean Phase 2a PR resolves these. Each needs a test.

### 3a. Startup crashes and parsing

- [x] **#732** `pactType: -1` fails the whole device list. `DeviceSettings` and
  two sibling structs in `undoc_api.rs` use `Option<u32>`; use a signed or
  lenient type. Small, and fatal for affected users.
- [x] **#722** A 400 from the scene API removes all effects and snapshots.
  `get_scene_caps` uses `?` on both scene calls; degrade per source, as it
  already does for the undocumented source.
- [ ] **#538** Malformed `segmentedColorRgb` schema breaks discovery. First
  check it still reproduces. Make one bad capability skip, not fail the list.
- [ ] **#76** A cloud outage at startup exits the process. Evaluate #736.
- [ ] **#536** Reports a panic; commenters show the cache failing to open.
  `cache.rs` still uses `expect` and `panic!` there. Return an error or fall back
  to an in-memory cache.
- [ ] Audit the 15 remaining non-test `expect` sites after Phase 1: `cache.rs`
  (3), `hass.rs` (4), `lan_api.rs` (2), `undoc_api.rs` (2), `iot.rs`,
  `music.rs`, `commands/lan_control.rs`, `commands/undoc.rs`.

### 3b. 2FA and auth

- [ ] Confirm with a live account that Phase 1 resolves #682, #647, #627, #633.
- [ ] Find out what happens when the account token expires on a 2FA account.
  `refresh_token` is parsed but never used. **Unverified risk:** a later
  re-login could demand a new emailed code.
- [ ] Finish #702 (account lockout) if the #703 port does not cover it.
- [ ] The add-on 2FA flow needs a restart within about 15 minutes. Consider a
  better one.

### 3c. LAN

- [ ] **#707** "timed out waiting for status". Gather logs; check whether the
  Phase 1 retry changes affect it.
- [ ] **#742** H60C1 goes dark on color temperature. Covered if #743 is ported.
- [ ] **#667** Purifier PM2.5 and AQI missing. Phase 1 adds air-quality sensor
  handling; retest, since PM2.5 may still be absent.
- [ ] **#735**, **#729** Scenes accepted but not applied. Retest after #716.
- [ ] **#623**, **#542** Slow cloud control and 429s. The H6006, H600D, and
  H601B quirks below should reduce Platform API use.

### 3d. MQTT and state stability

- [ ] **#749** 100% CPU spin in the libmosquitto thread after a broker restart.
  No PR. Likely in `mosquitto-rs`; reproduce first. #730 does not cover it.
- [ ] IoT subscriber failure only logs `IoT loop failed` and the process carries
  on without IoT (`iot.rs`). Restart it with backoff, or exit. Check #723 and
  **#696** against this.
- [ ] **#631**, **#486**, **#408**, **#142**, **#402** "Please specify the mqtt
  broker" or broker unavailable at boot. `run.sh` now waits for the broker;
  check whether blank-config startup still fails.
- [ ] **#508** State does not update after a command.
- [ ] **#551**, **#509**, **#563**, **#465** Offline devices still look on.
  Needs per-device availability; #718 only changes the Status sensor.
- [ ] **#719**, **#712** Zone and DreamView toggles have no state.
- [ ] Optional hardening: the client ID `govee2mqtt/<uuid>` contains `/`. The
  Mosquitto add-on fixed its rejection in 7.0.1 (#659, #661), but other brokers
  may be strict.

### 3e. Small quirk and option wins

- [ ] **#621** IoT quirks for H6006, H600D, H601B
- [ ] **#605** H6039 LAN quirk (confirm LAN support; reporters have different hardware)
- [ ] **#577** H616D is "Unknown device type" (needs device metadata)
- [ ] **#731**, **#558**, **#591**, **#567** Wrong color temperature ranges.
  9009 K is integer mired rounding (`1000000/9000 = 111`, `1000000/111 = 9009`).
- [ ] **#116** Expose the discovery prefix as an add-on option

---

## Phase 4: CI/CD, Docker, and rebrand (done, unpublished)

Add-on `govee2mqtt_revived`, image `ghcr.io/fleshinachair/govee2mqtt-revived`.
Publishing steps are at the top of this file.

Left for later:

- Dependabot action and cargo bumps.
- Run the Python test for the 2FA harness in CI.
- `scripts/cliff.toml` still links `#N` to `wez`, which is right for upstream
  numbers and wrong for this fork's own.
- `.github/FUNDING.yml` keeps wez's sponsor links on purpose.

---

## Do not spend time on these

Nothing here was closed or commented on upstream. This is local triage.

### Fixed upstream already (in release 2026.03.25 or later)

- #607, #609: UTF-8 panic on non-ASCII preset names
- #603: reported as the same panic (likely; the thread also shows a cache error)
- #323: byte-slicing panic of the same class (**unverified**, 2024 report)

### Expected fixed by Phase 1: verify, then ignore

- #617, #618: panics and exit code
- #647, #682, #627, #633: 2FA / status 454
- #626, #637, #649: "app version is too low" (one #626 commenter says release
  2026.03.25 already fixed it)
- #589, #748: empty `supported_color_modes`
- #510, #555: sensor values wrapped in `{"value": ...}`
- #369: value format only; the air quality index mapping is still unconfirmed
- #705: music palette notes (implemented)
- #87, #237, #242, #262, #438, #620: fan entities (likely; no hardware check)

### Duplicates: work the primary only

| Duplicate | Primary |
|---|---|
| #745 | #737 |
| #565, #578 | #549 |
| #637, #649 | #626 |
| #647, #627, #633 | #682 |
| #609 | #607 |
| #227, #572 | #110 (likely one cause) |
| #408, #486, #631 | #142 (likely one cause) |
| #661 | #659 |
| #692 | #192 |
| #750 | #747 |
| #416 | #186 |

### Won't fix: Govee, firmware, or environment

- Labelled `responsible:govee`: #35, #40, #49, #79, #96, #112, #178, #239, #245,
  #249, #316, #346
- Maintainer attributes it to Govee's API or the device: #53, #62, #153, #202
- Firmware (Govee rolling out an update): #747, #750
- User network setup, see `docs/LAN.md`: #175, #251, #360
- User's broker: #593
- Fixed outside this project (Mosquitto add-on 7.0.1): #659, #661
- Platform dropped by Home Assistant (armv7): #517

### Out of scope: needs BLE or major reverse engineering

- #99 (umbrella), #61, #118, #220, #296, #560, #569, #630

### Meta, not actionable

- #557 (compliment), #700 and #746 (maintenance status; see Phase 4)

### Stale: skip unless someone reproduces on a current build

A heuristic by age, not a judgement that these are invalid. The list is every
issue with **no update since before 2025** that is not named elsewhere in this
file.

#65, #67, #83, #84, #100, #102, #107, #109, #114, #119, #128, #130, #134, #136,
#139, #140, #145, #149, #154, #155, #157, #165, #166, #169, #171, #185, #191,
#193, #201, #203, #205, #207, #210, #221, #231, #258, #259, #260, #264, #276,
#281, #282, #284, #285, #286, #288, #290, #293, #298, #300, #301, #303, #304,
#307, #311, #312, #317, #318, #320, #325, #327, #329, #330, #335, #338

**Not covered by this plan:** 136 of the 324 open issues are not named anywhere
in this file. 65 of them were last updated between 2025-01-01 and 2025-11-25,
and 71 since. Absence here says nothing about their importance.
