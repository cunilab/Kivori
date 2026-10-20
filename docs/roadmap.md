# Kivori Roadmap

The order Kivori is built in. What it must be is in [product.md](./product.md); how it works is in
[architecture.md](./architecture.md); hardware checks are in [validation.md](./validation.md).

**Direction:** a desk buddy you control your computer with. Two equal pillars, **buddy** (the
reason to buy) and **control** (the daily value), with **monitoring** in support. Next work must
serve at least one pillar and never make the buddy smaller to fit more text.

**Sequence:** working foundation → useful device → configurable device → paid beta → customer
feedback → v1 investment. The aim is to learn whether people pay for Kivori before building every
long-term concern. Engineering rigor stays; only the order changes.

**Beta platform:** Windows is supported. macOS is later or experimental. Linux is later.

## Rules

1. A milestone is done only when every box is ticked, including its hardware rows in
   [validation.md](./validation.md). Host-sim and other host tests never replace a physical check.
2. Don't start the next milestone while the current one has open hardware rows. Code may run ahead,
   but it doesn't count as done.
3. Every milestone keeps all [product acceptance gates](./product.md#product-acceptance-gates) intact.
   Moving a feature later never relaxes a gate for what does ship.
4. Until v1, a feature is done when it works on Windows. macOS code may land but is not a blocker.
5. Anything not listed waits in [Later](#later). Move something earlier only if it blocks the beta.

---

## M0: Kivori Core (mostly done)

Software for the device link, mascot, rotary volume and flashing is built and green in CI
(about 300 workspace and 80 firmware tests; RISC-V clippy on every profile; Windows CI).

- [x] ESP32-C3 ↔ desktop protocol: postcard + CRC frames, handshake, version and capability negotiation, nonce as session identity
- [x] Automatic discovery, reconnect state machine, heartbeat, offline-first boundary
- [x] ST7789 rendering: deterministic renderer, 36-tile DMA path, Device Studio preview, golden-frame tests
- [x] Rotary input: HW-040 decoder (detent only on return to rest), 250 ms gesture boundary
- [x] Windows volume control: Core Audio backend, fixed-step volume, preview → confirmed overlay
- [x] Mascot: keycap buddy, social reactions, companion personality, typed session-only activity log
- [x] Firmware flashing of the bundled build from Device Studio
- [x] Hardening before the hardware session (2026-10-02): interrupt-captured encoder and switch edges
  (no lost quarter-steps during a frame flush), render on the tick feedback arrives, a desktop device
  thread that wakes on serial bytes instead of a fixed 50 ms sleep (row 3.14 remedies 1 and 2), and a
  `StateReport` answer to every `SetState` (row 1.16). Flashing and the session handshake verified on
  the real board from macOS; every row below still needs the human session
- [ ] Hardware sanity checks needed to build on safely (Windows only), in [validation.md](./validation.md):
  - wiring and rotary fidelity: 3.15, 3.1–3.4
  - discovery and recovery: 1.5–1.8, 1.15–1.17
  - volume truth and bounds: 3.5, 3.6, 3.11–3.13
  - detent → feedback under 50 ms: 3.14
  - panel and flashing: 2.1, 2.9
  - gates G1 (no false confirmation) and G2 (no stale replay)

**Exit:** the foundation is reliable enough to build product features on. The remaining Phase 1–3
rows move to M3 hardware QA.

## M1: Useful desk device

**Outcome:** Kivori gives daily value beyond being a volume knob.

Software is complete and host-tested (2026-10-02); the hardware rows are Phase 4 in
[validation.md](./validation.md). Built for Windows and macOS: macOS is the daily test bed, Windows
stays the beta platform and its rows are tracked in a GitHub issue.

- [x] Button press / hold input: short press released within 500 ms, mapped Hold released between 500 ms and ~2 s, one action per press, no auto-repeat
- [x] Recovery hold: ~10 s reboots the MCU, with visible progress and without the desktop app (gate 8)
- [x] Media play/pause, master mute (Press and Hold by default)
- [x] Keyboard shortcut action (shown as Unverified, never Success); runs from Device Studio until M2 bindings
- [x] Launch application action (Execution Confirmed); runs from Device Studio until M2 bindings
- [x] CPU and RAM monitoring
- [x] Display modes: buddy, clock, volume, media, CPU / RAM (picked in Overview)
- [x] Buddy reacts to real state: volume change → the keycap shrinks and lifts for the volume bar; muted → mute indicator; high load → heat cue (distinct from the Busy job face); media playing → bobbing note (Windows; macOS cannot observe playback); action outcome → badge; disconnected → offline
- [x] Every result shown as State Confirmed, Execution Confirmed or Unverified
- [x] Buddy home names the controls: `TURN` / `PRESS` / `HOLD` labels under the keycap, sent by
  Desktop from its live bindings (`ControlLabels`, tag 18), never guessed by the device (#9)
- [ ] Hardware: press/hold timing on the real switch; mute and media state stay in sync with Windows; reboot works without the desktop app (Phase 4 rows)
- [ ] Gates 1, 5, 6, 8, 9

## M2: Configurable product

**Outcome:** someone who did not build Kivori can configure and use it without editing code.

Software complete and host-tested (S1–S7); the hardware rows and gates below remain.

- [x] Three contextual buttons (#9), software done and host-tested end to end:
  - pins GPIO9, GPIO8, GPIO10 to GND (moved to match the enclosure's wiring, #123), internal pull-ups, no resistors; GPIO21 stays the UART0 boot
    log; B1 on GPIO9 doubles as BOOT (held at reset: download mode)
  - same edge interrupt as the encoder (ADR-0007); each button gives Press and Hold, never recovery;
    one gesture owns all input, so no chords and nothing during a recovery hold
  - wire: `ControlId::ContextButton(0..3)`, `CONTEXT_BUTTONS_V1`; labels in `ControlLabels.buttons`
  - defaults Previous / Play-Pause / Next (Unverified media keys); Hold unbound by default, and
    bindable (the middle button's Hold stays reserved for the profile pin)
  - screen: button labels under the keycap with ticks toward the buttons, knob label top left; the
    buddy keeps its full size. Final layout review pending (owner)
- [ ] Hardware: Phase 5 rows in [validation.md](./validation.md), starting with 5.1 (no 32 kHz crystal)
- [x] Config UI for Rotate / Press / Hold and the three buttons
- [x] Bindings are profile-shaped from day one: General is a profile with no app rule, so app-aware
  profiles add match rules, not a new model. Every binding change re-sends the device labels
- [x] Action catalog with explicit scope (System Volume ≠ App Volume)
  - the catalog, `ActionToken` and a production Test Action that respects Protected
  - App Volume and App Mute through WASAPI audio sessions on Windows; on macOS they are listed as unsupported and never fall back to system volume
- [x] Simple ordered macros (a macro reports its least-confirmed step; no rollback)
- [x] Choose what the display shows (default and secondary modes)
- [x] Buddy settings: reactions on/off, intensity
- [x] Config stored locally per OS user and machine; survives restarts and updates; reset to defaults
- [x] Basic device status and diagnostics (versions, connection, health)
- [ ] Hardware: rebind → use → restart → binding still there (Phase 5b rows in [validation.md](./validation.md))
- [ ] Gates 3, 4, 5

Example config:

```text
Control   Rotate: Volume   Press: Play / Pause   Hold: Open Spotify
Display   Default: Buddy   Secondary: CPU / RAM
Buddy     Reactions: On    Intensity: Normal
```

## M2.5: Contextual profiles (#9)

**Outcome:** Kivori changes what its controls do with the app in front of you, and says so on the
screen. Moved before the paid beta because the beta must test the contextual product, not a
volume knob; capped to keep the beta reachable.

Software done and host-tested (#12); macOS works the same way for daily testing.

- [x] Foreground app: Windows executable name, macOS bundle id; no window titles, no sub-app guessing
- [x] Focus polled every 100 ms, commits after 400 ms stable; deliberate input commits a pending
  app first (invariant 8), never a pending Unknown
- [x] Protected (UAC / secure desktop, lock and login screens, credential prompts, elevated apps)
  commits at once and beats a pin: shortcuts and launches are suspended (empty labels, Error if
  pressed), system volume, media keys and mute keep working (invariant 9, gate 3)
- [x] A knob gesture keeps the binding it began with; if focus moves mid-gesture its keys stop and
  it ends as Error / Context Lost, shown once (invariant 12)
- [x] Profile name top left; labels and bindings come from one context and switch together
- [x] Hold the middle button to pin: Auto → General → Browser → Code → Media → Zoom → Teams → Auto
- [x] Built-in profiles: General, Browser (knob = tabs), Code, Media, Zoom and Teams (Meeting mood;
  their buttons are shortcuts and show as Unverified). Knob shortcuts show no badge per detent
- [x] Buddy faces from observed truth (#13): error, confirmed-only celebration, strained, muted,
  listening, attentive; offline stays distinct from sleeping
- [x] Home layout (#14): profile top left, knob label under the clock, button labels over ticks at
  the bottom, press / hold while the switch is held; the buddy keeps its full size
- [ ] Hardware: Phase 6 rows in [validation.md](./validation.md)
- [ ] Gates 3, 4, 6
- Known limits: Windows Store apps all report `applicationframehost.exe` (they get General);
  `` ` `` and `[` shortcuts are untested on non-US keyboard layouts

## M2.6: Beta hero features

**Outcome:** the beta tests what people would pay for, not only a well-behaved knob. Kivori answers
"am I muted?" with certainty, works with whatever app is in front of you, and the buddy looks alive
even when nothing is happening. Moved before the paid beta for the same reason as M2.5: a beta that
only proves volume control cannot tell us whether people pay.

Software done and host-tested (#70; `MIC_STATE_V1`, `IDLE_LIFE_V1`, `DEVICE_INTERNALS_V1`); macOS works too. The
hardware rows below remain. Rules in [product.md](./product.md), design
in [architecture.md](./architecture.md) (ADR-0011 to ADR-0013); hardware rows are Phase 13 in
[validation.md](./validation.md).

- [x] **Truthful mic mute** (ADR-0011):
  - `Mic Mute` action: mutes every active capture device at the OS level and reads each one back;
    State Confirmed only when all of them read muted, otherwise Error with what stayed live
  - a mic indicator in the top indicator slot (mic outranks everything, as product.md already
    says) and a muted-mic face distinct from master mute; new art must fit the 128 KiB asset blob
  - "Live" cue when an app is using an unmuted mic, only where Windows reports it; unknown shows
    nothing, never "not live"
  - default binding: the left button in the Zoom and Teams profiles; offered in onboarding
  - wire: mic state behind a new capability (`Status` is a fixed struct, ADR-0008)
- [x] **Profiles for any app:**
  - "Add the app in front of me" (or pick a running app) creates a profile matched by executable
    or bundle id, starting as a copy of General
  - add an app to an existing profile (e.g. `idea64.exe` to Code); one app belongs to one profile
  - up to 16 custom profiles; rename, delete, reset to General; app icons in the profile list
  - custom profiles join the middle-button pin cycle only when marked "In pin cycle" (off by default)
  - wider built-in matches: Edge, Firefox, Brave, Arc for Browser; Cursor, JetBrains IDEs, Visual
    Studio, Windows Terminal for Code
  - config stays schema version 1 with serde defaults (the pre-beta rule in the schema); the damaged
    and newer-config paths stay
- [x] **A buddy that is alive when idle** (ADR-0012):
  - self-play moves from the desktop companion director into the firmware, so the buddy keeps
    moving through host stalls; the desktop only sends personality and on/off
  - new idle moments: glance left/right, look at a control when it is touched, slower blinks late in
    the evening (local clock from Desktop); the eyes never fully close outside Sleeping
  - deterministic: driven by the integer timebase and a per-unit seed, so golden frames stay exact
  - a brightness setting: backlight PWM on GPIO0 (GPIO8 before #123); no automatic night dim
- [x] **Quiet Unverified:**
  - a trigger-only action (media key, shortcut, or a macro whose weakest step is one) gets a neutral acknowledgement:
    the keycap press and a label pulse, no amber "?" badge and never a success look (gate 1 holds)
  - the amber badge stays where a state action could not read its result back (mute, mic, app
    volume) because there the user relies on the answer
  - the app's Activity log and Test still say "Unverified" in words
- [x] **Device internals in developer mode** (ADR-0013):
  - a "Device internals" card on the Device page, shown only with developer mode on
  - flash: app image size of the app partition, asset blob of its 128 KiB cap, total flash
  - RAM: static use, free SRAM (already sent), lowest free stack since boot
  - CPU and display: run-loop busy %, frame compose and flush time, frames per second
  - chip: model and revision, internal temperature where the chip reports it, uptime, last reset
    reason
  - input and link: edge-queue high-water mark and drops, invalid encoder transitions, frames in,
    out and rejected
  - asked for once a second while the card is visible, never streamed otherwise; included in
    "Copy diagnostics" only with developer mode on
- [x] **Splash screen** on app start:
  - a small window with the buddy, the app version and one honest status line ("Starting",
    "Looking for your Kivori"); indeterminate motion, never a made-up percentage
  - shown only while the app is really starting; skipped when it is ready in under ~300 ms, and
    never shown on a hidden start at login
  - local assets only (offline rule), light and dark themes, still under reduced motion
- [ ] Hardware: Phase 13 rows in [validation.md](./validation.md)
- [ ] Gates 1, 3, 5, 6, 9

## M2.7: Product identity, versioning and updates

**Outcome:** one Kivori app can serve more than one hardware product for years, and a beta user
always gets the latest app and firmware without hunting for installers. Moved before the paid beta
because every shipped unit must already say what it is, and a beta without updates cannot be
supported. Design: ADR-0014 to ADR-0016 in [architecture.md](./architecture.md); rules in
[product.md](./product.md#updates-us10-us11); hardware rows are Phase 14 in
[validation.md](./validation.md).

Software done and host-tested (#72); owner key and hardware rows remain.

- [x] **Product identity** (ADR-0014):
  - `products/registry.toml` hands out fixed machine ids and wire numbers for products and boards
    (`pala` = 1, `c3-supermini` = 1); never reused, never renamed; display names live in the
    catalog
  - the device reports product, board and hardware revision in a new `Identity` message after the
    handshake (capability `PRODUCT_IDENTITY_V1`); a device without it is the legacy Kivori Pip on
    the C3 SuperMini, revision unknown
  - each firmware image carries an identity record (product, board, firmware version, minimum
    protocol) next to its version marker; the app refuses to flash an image onto a different
    product or board, and the firmware shows an error screen if it boots on the wrong one
  - `just provision --product --board --rev --burn-identity` writes the identity into chip one-time memory (C3 user
    eFuse block) for units that are sold; it shows the record and needs a typed confirmation
- [x] **Product catalog in the app:** `products/<id>/product.toml` per product (display name, image,
  boards, flash method per board, bundled firmware with version, SHA-256 and minimum protocol,
  required capabilities); the app's device code reads the catalog instead of assuming one board.
  `firmware/esp32-c3` moves to `firmware/desk-buddy` (done), renamed `firmware/pala` with the
  product codename (2026-10-12)
- [x] **Release tooling:** a Release button (manual workflow, run only when the owner wants a
  release) bumps the app version and, when asked, the firmware version, and builds a draft whose
  notes come from the conventional commits since the last release; the app tags `vX.Y.Z` and gets
  a normal (not pre-release) GitHub Release titled with its firmware ("Kivori 0.1.0 · Pip firmware
  0.1.0"); firmware tags `fw-<product>-vX.Y.Z` with no GitHub Release of its own
- [x] **App updates** (ADR-0015): Tauri updater against the latest GitHub Release's `latest.json`,
  every update verified with the updater key built into the app; checked at start and daily, on by
  default, a setting to turn it off; tray badge, then a dialog when the window opens ("Install &
  restart", "Later" for 3 days, "Skip this version"); never during a flash or a gesture
- [x] **Firmware update offer:** a few seconds after a device connects and is idle, if its firmware is
  older than the bundled one for its product: "Update now" or "Later" (asked again on the next
  connect; never skipped for good); app update first when both are due; an incompatible device gets
  the required update; a device newer than the app gets "Update the Kivori app"
- [x] **Locking, first tier** (ADR-0016): wrong-product guard in the app and the firmware boot check
  (above); secure boot waits for v1
- [ ] Owner, once: create the updater signing key, back it up in two places, add the two GitHub
  secrets; publish beta releases as normal releases
- [ ] Hardware: Phase 14 rows in [validation.md](./validation.md)
- [ ] Gates 5, 8, 10

## M2.8: Desktop organization and product contract

**Outcome:** support tools are easy to find, unsupported device controls are absent, and another
product using existing features can be added without product-specific branches in shared Desktop
code. Design: [ADR-0017](./adr/0017-product-contract-and-capability-driven-desktop.md), reviewed from
[issue #76](https://github.com/cunilab/Kivori/issues/76).

The software checklist below is implemented and merged into `main` through PRs #77–#103 on
2026-10-19. Post-merge host, firmware and determinism CI passed. M2.7 hardware rows and M3
owner/hardware blockers remain open; Windows remains the beta acceptance platform. The
[closure audit](./validation.md#m28-software-closure-audit) records the merge and CI evidence and
remaining limits. No release or hardware acceptance is claimed.

- [x] **Developer page:** always visible in the sidebar, master toggle off by default; consolidate
  Activity, protocol details, internals and direct reaction tools. Device Studio requires both its
  build gate and the toggle. Turning the toggle off stops internals polling and removes developer
  controls everywhere. Preserve release-surface checks and keyboard/focus behavior.
- [x] **Product contract:** catalog-driven names, optional art, firmware paths, provisioning and
  packaging/release selection; remove shared-app hardcoded product defaults and the manually
  maintained embedding list. Missing art uses a neutral frame. New transports, flash methods or
  interaction models need explicit feature work; this is not support for arbitrary future hardware.
  Active catalog entries are discovered, validated and embedded at build
  time without a maintained source list; registry reservations without metadata remain inactive,
  and fixture products stay outside shipping roots. Image build checks resolve each product/board by image identity rather
  than catalog order. Provisioning/tooling and image-set work are detailed below; generated catalog
  discovery is supplemented by the provisioning, packaging and release checks below.
  Customer presentation now exposes catalog name/model for the current known product/board, with
  product figures on Home, Welcome and Tour. Fixed hardware-body artwork is removed. Declared static
  PNGs are now validated and embedded at build time, delivered by a cached allowlisted lookup, with
  neutral fallback and stale-load protection. Pip still has no artwork; fixture images stay outside
  shipping roots. Provisioning QA now selects requirements/input groups/discovery hints from the
  bundled image’s catalog identity, rejects identity-argument mismatches before device I/O, and
  requires matching post-flash identity. Identity burns are limited to the implemented C3 backend.
  Firmware build target/features now live in the catalog; local build/bundle/provision recipes,
  PowerShell and release firmware CI share validated plans; targeted builds select an explicit or
  unambiguous pair, and desktop packaging includes every pair.
  Recovery now refuses missing images, invalid/stale picks and absent candidates
  before port reservation; image availability cannot decide hardware identity. The image-set foundation now validates/emits deterministic
  multi-image indexes with exact lookup. Native flash, status, offers, recovery and provisioning now
  select exact per-device images; the runtime build gate is removed. Pair-level recovery choices and release metadata are implemented below. Desktop development/bundle recipes and
  release CI now build/package every active pair, snapshot shared outputs and recreate host-local
  manifests after transfer. Native manifest embedding independently enforces the complete identity
  set. Catalog release planning now guards all products/crates before writes, supports independent
  override maps, refreshes every lockfile and atomically pushes all lineage tags. Titles/notes cover
  every bundled product; two-product fixtures exercise the actual workflow against local remotes.
- [x] **Capability-driven UI:** document the capability-to-surface mapping; backend effective support
  combines current-session negotiation, resolved identity and host support, not catalog minimums.
  UI and command handling deny ordinary product actions while identity is pending/unknown. Cover
  navigation, cards, display views and controls. Keep offline binding/display configuration editable
  without authorizing device actions from retained metadata; preserve stored bindings for absent
  controls when products switch, without adding per-product profile stores. Define incompatible
  and known identity-only recovery states; move focus when a live page becomes unavailable.
  Session effective capabilities wait for resolved identity, gate feature
  traffic/input and live display commands, and reach the UI through `ConnectionStatusDto`.
  Saved display/binding configuration stays editable offline. The main-window
  [surface mapping](./desktop-capabilities.md) now covers live actions, neutral session states,
  saved-only control annotations, offline Studio preview, focus recovery and control-specific input
  guards. Setup now gates Plug-in readiness on known usable sessions, shows non-blocking guidance
  only for supported controls, and annotates offline/absent-feature settings without changing saved
  configuration on product switches. The old host-status completion heuristic is removed because it
  cannot verify device input. Cross-surface acceptance is verified by Product acceptance tests below;
  the closure audit verifies this surface mapping against the current implementation and checks.
- [x] **Hardware tiles:** reorganize the existing formatted metrics and usage bars into readable
  flash, RAM and run-loop/frame tiles. Label app image versus app partition, static RAM, stack
  headroom and run-loop busy time accurately; do not imply heap usage or whole-chip CPU load.
  Preserve unavailable values and visible-only 1 Hz polling (ADR-0013). No new firmware metrics,
  protocol changes or trend-history feature in this item.
- [x] **Support bundle:** basic Copy diagnostics works with developer mode off and is also exposed
  on Developer. Optional internals come only from visible instrumentation in the current session;
  mark unavailable/stale data, never copy another device's snapshot, and never start polling just
  to copy. Correlate snapshots with connection generations, clear on session boundaries and reject
  late replies; test reconnect while the card stays mounted. Preserve privacy and clipboard errors.
- [x] **Home health summary:** developer-only link state, consecutive retries and total reconnects,
  reusing existing status/diagnostics data; no internals polling or event-history dashboard on Home.
- [x] **Product acceptance tests:** Pip, a minimal fake product and mixed capabilities; supported
  controls still work, unsupported controls/art are absent. Exercise missing art, old firmware,
  pending/unknown identity, known identity-only firmware, offline configuration and session switches.
  Test backend action refusal before identity resolves. Catalog/build tests prove additions using
  supported features require no shared-app product-list edits and firmware selection ignores order.
  Isolate fake products/images from shipping discovery and test preservation of stored bindings.
  App-shell acceptance now exercises Pip, a synthetic dial and mixed capabilities through real
  navigation/hooks and browser IPC, including supported actions, saved bindings, offline edits and
  pending/unknown/identity-only/legacy session changes. Native tests now exercise the runtime's live
  display/ordinary-flash admission and reaction refusal across identity resolution/reconnect.
  Catalog generation and reversed image-set fixtures compile the generated sources outside shipping
  roots. The [software acceptance evidence](./validation.md#m28-product-acceptance-evidence) maps
  each criterion to its checks; real hardware rows remain open.
- [x] **Product identifier guard:** scoped to shared app/tooling, catalog-derived identifiers,
  exact line/count exceptions and CLI positive controls on Ubuntu/Windows. Includes untracked files,
  filenames, camel/separator variants and production code after Rust test modules; real-repository
  coverage is asserted. Offline dependency checks visit every catalog firmware workspace.
  Supplement behavioral tests rather than replacing them; hardware-specific dev matrices remain
  explicit reviewed exceptions.
- [x] **Firmware release guard:** reject changed firmware inputs under an unchanged released version
  before writing versions or pushing. Compare resolved versions, including exact overrides; define
  first-release behavior, reachable product-specific baselines and catalog/build/assets/toolchain/
  lockfile inputs. Recognize harmless version bookkeeping by content, never commit title or author.
  List offending commits; test rejection, valid bumps, exact overrides, no prior tag, divergent
  history and bookkeeping mixed with real changes.
- [x] **Firmware bundles and release metadata:** replace the single-image bundle with validated
  catalog-driven product/board images; select matching firmware for updates, flash and recovery.
  Build/package the whole supported set and list every product's independent version in release
  titles/notes. Test two product fixtures, image identity/version mismatch and wrong-product refusal.
  No-handshake recovery requires explicit choice for ambiguous USB/board hints and refuses no match;
  never choose an implicit default. Release validation excludes all test products/images.
  Keep one app release, permanent firmware tags, draft publication and existing flash safety rules.
  Pair choices now include board ids/names and require a new visible BOOT confirmation. Native
  re-enumeration, live identity checks and exact image selection precede any port reservation.

Written policy: [release promises and M2.8 guard requirements](./release.md#release-policy), and
[protocol change discipline](./adr/0002-one-wire-protocol-crate.md). These policies are recorded;
the checked boxes record verified, merged software implementation. Hardware acceptance and ADR
approval remain separate; no new hardware evidence is claimed.

Out of scope: multi-device UI, a device-gallery Home, visual redesign of customer pages, new
firmware instrumentation, runtime plugins, cloud sync, downgrade guarantees, update channels,
independent per-product profile stores and platform work already scheduled for M4/Later.
M2.8 does not relax any paid-beta gate.

## M3: Paid beta

**Outcome:** an external user receives Kivori, installs it, configures it, recovers from common
problems and uses it without developer help. Start charging for beta units here.

- [ ] Enclosure (keeps the USB port and the ESP32-C3 BOOT path reachable for recovery; the three
  buttons sit in a row directly under the screen, centres one third of the panel's active width
  apart, each under its 80 px label column (#14))
- [ ] BOM and real unit cost (draft parts list in [bom.md](./bom.md); prices, suppliers and the real unit cost still need the owner)
- [ ] Repeatable assembly process (guide written: [assembly.md](./assembly.md); stays open until a person has built a unit with it)
- [ ] Device flashing and provisioning process (tool exists: `just provision`, see [provisioning.md](./provisioning.md); stays open until a person has run it on real units)
- [ ] Hardware QA checklist per unit (built from the remaining Phase 1–3 rows in [validation.md](./validation.md); the automated part is `just provision`, validation Phase 12, which still needs a human run)
- [ ] Signed Windows installer, bundling the known-compatible firmware (this is the beta's update authentication, gate 10). The unsigned pipeline exists (M3 S5: `release.yml` builds, install-tests and drafts the NSIS installer with the pinned `espflash` sidecar); this stays open until the installer is signed, which waits on owner certificates (#36). See [release.md](./release.md)
- [ ] macOS build (if shipped; `scripts/bundle-macos.sh` bundles the adapter, entitlements and Info.plist and signs only when `APPLE_SIGNING_IDENTITY` is set; software done in M3 S5, signing and the hardware rows are open): bundle `target/<profile>/mediaremote-adapter/` as a resource, Developer ID-sign its two Mach-O helpers with hardened runtime, add `NSAppleEventsUsageDescription` and the `com.apple.security.automation.apple-events` entitlement, list the adapter (BSD-3) in third-party notices (ADR-0009)
- [ ] Launch at login and tray presence; the window stays optional. Software done in M3 S4 (autostart plugin, hidden start, tray status and switch, Device page switch); stays open until the S4 hardware rows in [validation.md](./validation.md) pass
- [ ] Survive sleep/wake and lock/unlock with intentional screens, never a frozen frame (gate 9). Software done in M3 S1+S2 (host takeovers: Sleeping, Updating and a 4 s host-silence Offline; sleep, lock and console-switch handling in Desktop); stays open until the S2 hardware rows (Phase 7) in [validation.md](./validation.md) pass
- [ ] First-run onboarding. Software done in M3 S6 (stepper for plug in, macOS Accessibility, a capability-filtered non-verifying guide (replacing the earlier live-tour heuristic), default view, buddy intensity and launch at login, with "Run setup again" on the Device page); stays open until the S6 hardware rows in [validation.md](./validation.md) pass
- [ ] User-facing firmware update and recovery flow (flash the bundled build; a failed flash says so). Software done in M3 S3 (flash, classified failures, BOOT-button restore, incompatible recovery); the pinned `espflash` 4.6.0 sidecar now ships in the installer (S5); stays open until the S3 hardware rows (Phase 8) in [validation.md](./validation.md) pass
- [x] Device and software version shown to the user (Device page and Home show the device firmware, app version and the firmware bundled in the app, with an "update available" hint; the bundled version is read from the firmware image, so it appears whenever a build embeds one)
- [ ] Regulatory check before selling (FCC Part 15 / CE for the finished unit, not just the module)
- [ ] Basic packaging and a short user guide (guide written: [user-guide.md](./user-guide.md); stays open until the physical packaging exists)
- [ ] Build and ship the first 5–20 beta units
- [ ] All 10 gates for what ships

## M4: v1

Use paid-beta evidence to pick what deserves deeper investment. None of these is assumed required
before demand is proven.

- Better macros; more monitoring (GPU temperature, mic where available, custom sources)
- More physical inputs
- macOS support (CoreAudio backend, Accessibility permission flow, menu-bar presence)
- Integrations (Discord, Teams, OBS) as indicators
- Production PCB with an app-independent ROM recovery path, on the v1 controller chosen in ADR-0010
  (proposed: native USB so basic controls work with no app installed)
- `Update All` in one step; A/B firmware with rollback
- Secure boot on production units with native USB (ADR-0016)
- Update channels (beta / stable) and, if firmware cadence demands it, firmware fetched apart from
  the app through the same catalog format (ADR-0014)
- macOS auto-update, with Mac signing
- Long-term reliability: 8 h idle soak, display idle and burn-in protection, 10 workdays of real use

## Later

Unless it blocks the beta:

- Windows/macOS feature parity
- Exhaustive sleep/lock edge cases
- Multi-user session switching; a second device stays Passive
- Factory reset, separate from recovery
- Composite inputs (`Hold + Rotate`) and split-direction bindings
- Buzzer or haptics
- Linux support
- Monitor Mode for a second device
- Focus timer on the device (truthful, since the device owns it, but not core)
- External status sources (CLI / local HTTP / WebSocket). A local listener is a new attack surface
  and breaks the no-network rule, so it needs its own ADR first
