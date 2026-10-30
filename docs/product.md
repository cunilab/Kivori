# Kivori Product

What Kivori is, what the beta must do, and the rules every build keeps. See also [roadmap.md](roadmap.md), [validation.md](validation.md).

## What Kivori is

Kivori is a desk buddy you control your computer with. Two pillars carry equal weight: the
buddy is why people want one, the controller is why they keep using it. You control your computer
with a physical knob and button; the buddy is always on screen, shows important desktop state and
says what the controls do; simple system signals support it and never take its place. Hardware: ESP32-C3, ST7789
240x240 display, HW-040 rotary encoder with push button, three contextual buttons; no buzzer,
haptics or light sensor. A Tauri
app, Kivori Desktop, drives it and is the configuration center. The buddy has personality but only
reports the desktop; it is not a virtual pet with its own goals.

The first product is **Kivori Pip** (short name Pip, model KV-P1, machine id `pip` in
`products/`). A product's machine id is its short name in lowercase (Pip's was the codename `pala`
until 2026-10-30); the customer name, short name and model code live in its catalog entry.

**Beta target:** a non-maintainer on Windows receives a unit, installs, configures, updates and
recovers Kivori using only the shipped docs, and pays for it. **v1 target:** decided from beta
evidence (see [roadmap.md](roadmap.md#m4-v1)).

## Scope

### In the paid beta

- One Kivori on one active desktop session, on Windows or macOS. Initial Mac builds target Apple Silicon.
  Mac app updates are installed manually; per-app volume and mute are unavailable on macOS.
  Public distribution and physical acceptance are tracked separately in [macOS support](./macos.md).
- Rotary + button input (press, hold). Actions: system volume, media, master mute, mic mute,
  keyboard shortcut, app launch, simple ordered macros.
- Monitoring: CPU, RAM, volume / mute, mic mute and mic in use, current media, clock, connection and
  device health.
- Immediate acknowledgement, honest confirmation, live state sync (including changes
  made outside Kivori).
- General profile; config UI; config local per OS user and machine; reset to defaults.
- App-aware profiles on Windows: built-in and user-created, match by executable, General as
  fallback, Protected classified first. A profile can be pinned from the device (this is what "Focus / Media / Meeting" modes
  are: pinned profiles, not a second concept).
- Presentation model (buddy, personality, takeover, health, indicators) with intentional
  visuals for every waiting, failure and recovery state the beta can reach. A buddy that stays
  alive on its own when idle; a brightness setting.
- Recovery across sleep, wake, lock and reconnect.
- The ~10 s MCU reboot gesture; launch at login; tray presence.
- Visible desktop and firmware versions; user-initiated flashing of the bundled firmware,
  shipped inside a signed installer.
- One app for every Kivori product: each device reports its product, board and hardware revision,
  and the app only flashes firmware made for that product and board.
- App updates found through GitHub Releases and offered in the app; a firmware update offered when
  a device with older firmware connects.

### After the beta (decided from beta evidence)

- Wider macOS hardware/version coverage and automatic app updates.
- Call indicators; GPU temperature and custom sources.
- App-free basic controls (volume, media) on the v1 controller, when no Kivori Desktop session
  exists ([ADR-0010](./adr/0010-v1-controller-with-native-usb.md), proposed).
- Display idle and burn-in protection beyond the basics.
- `Update All` in one step; update channels; rollback-safe (A/B) firmware install; macOS
  auto-update.
- Production hardware with an app-independent recovery path, detected by Desktop; secure boot on
  production units that can act as a USB keyboard (ADR-0016).
- OS user switching; Passive handling of extra devices; factory reset.

### Not planned

- Linux (until demand shows); cloud sync; news/weather dashboards; pet progression; workflow
  orchestration.
- Implicit chords, implicit key-repeat, split rotary directions, guessing sub-apps.
- Replaying input or transients after reconnect; auto-restarting uncertain host jobs.
- More than one Active Kivori; Passive as Monitor Mode; waking the host.
- Process injection, memory inspection or anti-cheat hooks to detect overlays.
- Silent flashing, unauthenticated or known-incompatible installs, fake progress.
- Firmware downloaded apart from the app (it ships inside the app; ADR-0014 keeps the format open).
- Locking firmware against copying onto clone boards.

The rules below are written for the whole product. A rule for a feature that is not shipped yet
(profiles, macOS, multi-user) applies when that feature ships.

## Core behavior

Timings are initial targets. They may be tuned; the rule behind them may not change.

### Input and gestures (US1, US3)

- One gesture owns all input at a time. Simultaneous inputs never become chords; future
  combos must be explicit input types.
- Only validated logical detents count. Firmware may debounce and decode quadrature;
  noise never counts as a reversal. Missed detents are not reconstructed and are not an
  error ("missing input is not failed input").
- **Gesture boundary:** a rotary gesture ends after
  **250 ms** with no new detent. A reversal does not split it. This is separate from
  acceleration reset, even though both default to 250 ms.
- Rotary actions may set range, step, sensitivity and acceleration. Acceleration builds
  only across same-direction detents, resets to 1x after 250 ms idle or on any valid
  reversal, and never exceeds 5x the base step. Actions may lower or disable it.
- Bounded values clamp at once, with one subtle reaction at the limit and no spam. The
  first reverse detent applies immediately at 1x.
- A `Rotate` binding owns both directions. A profile may replace the whole binding,
  never one direction.
- Short press fires only if released within **500 ms**. Discrete mappings fire once per
  press; no implicit key-repeat.
- Contextual buttons give Press and Hold with the same timings, never recovery: held past ~2 s they
  fire nothing. Their labels on screen always match what they run.
- Recovery button: release between 500 ms and ~2 s may run a mapped Hold. At ~2 s
  recovery owns the gesture and the Hold is cancelled; at ~10 s from key-down the MCU
  reboots. Rotation during the hold changes nothing. Not configurable in v1.
- **Input freshness:** a discrete action (switch or contextual-button Press, Hold, Double
  press) older than **750 ms** when Desktop is about to run it is dropped and logged, never
  run late, whether it waited on the link, the device task or the action queue. Knob detents
  and gesture start/end are never dropped for age: a late turn is still a turn, and dropping a
  gesture end would leave the gesture open. Age needs the device clock offset from the
  heartbeat; until the first reply there is no age and nothing is dropped.
- Rapid rotation uses a local preview, no round trip per detent. During a gesture,
  external updates to the same value do not fight the preview; the confirmed value
  wins when the gesture ends.

### Context and profiles (US5)

- The profile follows stable OS keyboard/window focus, never hover. New focus commits
  after **300-500 ms** of stability.
- Deliberate input commits a valid pending foreground app at once (after the Protected
  check). General is never a stand-in during a race.
- A gesture stays bound to the context where it began. If that app disappears, the
  gesture is not retargeted: it ends as Error / Context Lost (shown once) and its
  remaining detents are ignored.
- Workspace-switch UI and confidently recognized transient overlays do not own profiles.
  Ambiguous overlays follow normal focus rules.
- Generic hosts (`chrome.exe`, `python.exe`) use their host profile unless the user
  targets more precisely. Background apps never own physical mappings.
- Built-in profiles: General (fallback), Browser, Code, Media, Zoom, Teams. Users can override
  any binding per profile in the config UI, and reset a profile or everything to these defaults.
- Custom profiles: "Add the app in front of me" (or a running app) creates a profile matched by that
  executable (bundle id on macOS). It starts as a copy of General's bindings, made once at creation;
  later changes to General do not flow into it. Users can add apps to any profile, built-in ones
  included. One app belongs to exactly one profile: adding it elsewhere moves it, and says so.
  At most 16 custom profiles; a name longer than the panel's profile slot is shortened there,
  never replaced. Deleting a profile sends its apps back to General.
- Holding the middle contextual button pins the next profile (Auto, then General and the built-in
  profiles, then custom profiles marked "In pin cycle", then back to Auto); a pin ignores focus but
  never Protected.
- In a protected context, shortcuts and launches are suspended: their labels disappear and a
  press says it can't run. System volume, media keys and mute keep working.
- A knob bound to shortcuts sends one per detent with no badge per detent; the label is the
  feedback, and a failure shows once per gesture.
- Actions have explicit scope: `System Volume` and `Discord Volume` differ. Global audio
  follows the OS default endpoint; app audio is configured explicitly.

### Actions and confirmation (US2)

- **State Confirmed:** the resulting state is observed (mic reported muted).
- **Execution Confirmed:** the operation is known to have started or finished, but no
  lasting state exists (app launch started).
- **Unverified (Triggered):** dispatched, effect unknown (a shortcut). Never shown as success.
- Acknowledge < 50 ms; Processing at ~500 ms; timeout at ~1,500 ms. Known failure ->
  Error; unknown outcome -> Unverified. Timeout alone is never Error.
  A confirmed cancellation before input starts is a known non-execution and may be Error.
- Macros report their least-confirmed required step; a known step failure makes the
  macro Error / Partial Failure. No automatic rollback of earlier steps.
  Native input whose completion is still unknown stops further steps as Unverified, so an
  unfinished chord cannot overlap a later launch. Completed writes with unknown read-back may continue.
- **Mic mute** mutes every active capture device at the OS level, below any app, and reads each one
  back. It is State Confirmed only when every device reads muted; a device that stays live makes it
  Error and names that device. A device with no mute control (a phone's Continuity microphone)
  counts as muted only while no app is recording from it; if one starts, the state turns live
  within about a second, and a device whose capture cannot be observed counts as live. A device that cannot be read back makes it Unverified with the
  amber badge, never Confirmed. Unmute restores each device to unmuted (not to a remembered
  level). The scope is the OS microphone, not an app's own mute button, and the UI says so.
- "Mic in use" (an app is capturing) is shown only where the OS reports it. Unknown shows no cue,
  never "not live".
- Long-running host jobs have a Running state. A link problem does not fail a job the
  host still observes; reconnect reconciles the job and never reissues it.
- **Quiet Unverified.** A trigger-only outcome (media key, shortcut, a macro whose weakest step is
  a trigger) is acknowledged quietly on the device: the keycap press and a short label pulse, no
  badge. It never borrows a success look and never celebrates. The amber "?" badge is kept for a
  state action that could not read its result back (mute, mic mute, app volume), where the user
  relies on the answer. Desktop's Activity log and Test always name the outcome in words.
- Transients: Success ~800 ms, quiet acknowledgement ~400 ms, Unverified badge ~1.2 s, Error ~2 s. The newest deliberate input
  replaces older feedback (no queue); background transients wait; urgent states may
  preempt. A finished transient returns to the real state (for example Running), not Idle.

### Display and buddy (US7, US8)

- Layers, highest first: takeover state, system health, primary buddy state, secondary
  indicators. A personality layer animates beneath them.
- The buddy is the home screen. Clock, indicators and control labels sit around it; other views
  are details you open on purpose, and transients return to the buddy, never replace it.
- Home layout, around a full-size buddy: profile name top left (a dot when pinned), clock top
  centre, indicators top right, the knob's label under the clock, the three button labels along
  the bottom over ticks pointing at their buttons. While the knob switch is held the bottom row
  shows what releasing does (Press / Hold).
- The home screen says what every control does right now, in the active profile's words. The
  labels come from the bindings Desktop actually runs; with no label the control shows nothing,
  never a guessed action. A context switch changes labels and bindings together.
- Takeovers: Sleeping / Locked, Switching User, Protected, Permission Required, Host
  Starting / Resuming, Firmware Updating, Firmware Recovery, Reconnecting, Disconnected (shown
  on the panel as Offline), Waiting, Passive. A takeover cancels an active gesture and shows at once. Sleeping stays
  on the panel when the host says it is going to sleep, even after the link drops. Firmware
  Updating ("Updating", keep plugged in) shows while the host flashes, for at most 2 minutes.
  A host that goes silent for 4 seconds (crashed or killed Desktop, no goodbye) shows Offline,
  never a frozen "connected" frame.
- Healthy needs no space. Degraded gets its own cue, not an indicator slot.
- Primary states (Idle, Active, Busy, Success, Error, Unknown) each have a distinct face.
- The resting buddy reads observed truth (#13), first match wins: an Error result shows the
  error face; a confirmed result (State or Execution Confirmed, never Unverified) celebrates
  with a deep press; then mic muted (a covered mouth, distinct from master mute), sustained high
  load (strained), master mute (muted), media playing (listening), a meeting profile (attentive). Booting, Offline and Sleeping are never
  overridden, and Offline never looks like Sleeping.
  One unknown indicator never makes the whole buddy Unknown.
- Indicator priority: mic, call, master mute, media, custom app. Custom indicators never
  displace the first four. Over budget, hide the lowest. Higher priority appears at once;
  a lower one promotes after ~250 ms stable.
- Personality may animate freely but must never look like a desktop state. It yields to
  every layer above it, and its settings change intensity only.
- Idle life runs on the device itself (blinks, glances, a look towards a control as it is touched,
  slower blinks late in the evening), so a busy or stalled host never leaves a frozen buddy. Outside
  Sleeping the eyes never fully close, so idle life never looks like Sleeping. The buddy is still
  not a pet: no needs, no progression, nothing that punishes neglect.
- No ambiguous silence: when the device can render, every waiting, processing, restricted,
  transitional, failure or recovery state has an intentional look. Unknown progress is
  indeterminate activity, never a made-up percentage.

### Connection, sleep and recovery (US9, US10)

- States: Connected; Waiting (no Kivori Desktop session); Passive (session exists, unit
  not the controller); Host Starting (15-30 s grace); Switching User; Reconnecting
  (intentional restart); Degraded (health cue); Disconnected (healthy link lost); Sleeping
  / Locked. Known sleep, lock, switch or update state outranks link-loss symptoms.
- Sleep and lock (Desktop): when the computer is about to sleep, Desktop tells the unit
  first, so the buddy shows Sleeping rather than Offline, then lets go of the port. On
  wake it looks for the unit again at once. Nothing turned or pressed before the sleep runs
  afterwards. Locking the screen puts the buddy to sleep and unlocking restores what you
  had chosen; the knob keeps working under the lock screen. When another user takes over
  the console, Desktop releases the unit so that user's Kivori can use it.
- Nothing is buffered during Reconnecting or Disconnected. Reconnect restores current
  truth and never replays old input or old transients.
- Brightness is a setting. Its lowest step stays clearly on, so it never looks like Display Sleep.
  Kivori does not dim on its own by time of day.
- Display idle: Normal -> Dim -> Low Motion -> Display Sleep, staying Connected. Each step
  is configurable within hardware-safe bounds. Display Sleep is the one intended blank.
- A gesture that starts in Display Sleep is wake-only for its whole length. In Dim or Low
  Motion, input runs normally. Passive vibration or USB activity does not wake the
  display; a narrow set of urgent states (incoming call, mic going live) may.
- Desktop `Test Action` / `Preview Buddy State` may wake Display Sleep, keep the idle age,
  and sleep again ~1 s after feedback. They count as deliberate input but never bypass
  Protected, permission, assignment or takeover rules.
- The recovery hold works without Desktop, in any link state, in Passive, in Protected and
  from Display Sleep. It shows truthful progress and never erases config. Factory reset
  is a separate, harder procedure.
- One Active Kivori per session. Others are Passive: no mappings, no mirroring, recovery works.

### Permissions and protected contexts (US6)

- Protected contexts (UAC, secure screens, lock screen, admin surfaces) are classified
  before any profile fallback. Custom actions and macros are suspended there.
- A missing permission is not a disconnect. Unaffected capabilities keep working, blocked
  actions are not retried in a loop, and a broad loss may escalate to Permission Required.
- macOS needs Accessibility for shortcuts and focused-app detection. Missing it is a normal
  Permission Required case, not an error.

### Desktop app

- Developer mode (a per-computer setting, off by default) reveals Activity, technical device
  details and device internals: flash, RAM, CPU, frame timing, chip and link counters. Internals are
  diagnostics only: they are fetched only while shown, carry no ids or payloads (ADR-0005), and a
  value the device cannot measure shows as unavailable, never as zero.
- A splash screen covers a real startup only: buddy, version and one honest status line, with
  indeterminate motion. It is skipped on a fast start and on a hidden start at login, and it never
  delays a ready app.

### Configuration and users (US11)

- Desktop configures profiles, actions, rotary tuning, global bindings, feedback, display
  idle, app targeting and device assignment, and shows versions and update status. The
  window need not stay open. Core features work offline.
- Config belongs to the active OS user and machine. On a user change, the old user's
  mappings stop and their app names, icons and feedback become non-renderable first.
- Background feedback respects OS Do Not Disturb. No buzzer settings until hardware has one.

### Updates (US10, US11)

- Versions: the app has one version (the release tag). Each product's firmware has its own
  version. Compatibility is decided only by the protocol version and capabilities, never by
  pairing app and firmware version numbers. The app supports the current and the previous protocol
  major, and carries the firmware for every product it supports, so it can always bring a device up
  to date.
- Identity: a device is a product, a board and a hardware revision. Machine ids and wire numbers are
  fixed in the product registry and never reused or renamed; display names may change. Kivori never
  flashes firmware made for another product or board, and firmware that finds itself on the wrong
  board says so on screen instead of running.
- App updates (beta, Windows): the app checks the latest GitHub Release at start and once a day, on
  by default, with a setting to turn it off. The check sends no ids. An update is installed only
  after its signature checks out against the key built into the app, and only when the user chooses
  "Install & restart"; "Later" asks again after 3 days and "Skip this version" skips that version.
  The offer is a tray badge, then a dialog when the window opens; it never appears during a flash
  or a gesture. No network, or a failed check, changes nothing and says nothing.
- Firmware updates (beta): flashing is manual and user-initiated with the bundled,
  known-compatible build. When a device whose firmware is older than the bundled one connects and is
  idle for a few seconds, Desktop offers the update once per device and firmware version ("Update
  now" or "Later"; "Later" asks again on the next connect). When an app update is also due, the app
  update is offered first. A device that cannot work with this app gets the required update; a
  device newer than the app is told to update the app. A failed flash is reported as failed.
- Later: Desktop reads an authenticated manifest and checks compatibility and order before
  touching either side. `Update All` updates Desktop first when firmware needs it and
  stops if that fails.
- An available update is not consent. Firmware update is a takeover with truthful phases;
  mappings are suspended. Success means post-update validation passed, not a reboot. A
  rollback is reported as "Update Failed - Previous Firmware Restored".
- Production hardware keeps a ROM bootloader path below the app image; Desktop offers a
  restore flow when it detects it.

## Invariants

Numbers are fixed from the original user-story contract. Gaps are intentional; never renumber.

1. **Desktop truth wins.** Confirmed observable state overrides local prediction or requested value.
2. **Observable truth only.** Kivori never invents state it cannot reliably observe.
3. **Acknowledgement is not confirmation.** Input feedback is never shown as proof of success.
4. **Confirm only what is knowable.** Known success, known failure and unknown outcome stay distinct.
5. **No stale replay.** Expired input never runs after a connection or service recovers. Within one session a discrete action older than 750 ms is dropped too; detents and gesture boundaries are exempt (see Input and gestures).
6. **Explicit action scope.** An action's meaning never changes because another profile is active.
8. **Interaction commits intent.** Deliberate input commits a valid pending foreground app first.
9. **Security context beats profile fallback.** Protected is classified first; never General.
12. **Gesture ownership.** A gesture stays bound to the context in which it began.
14. **Wake gestures are atomic.** A gesture that begins in Display Sleep is wake-only in full.
17. **User isolation.** One OS user's mappings and presentation never show or act for another.
19. **No hidden fallback.** If an action cannot run, Kivori says so instead of switching mechanism.
21. **Known host/session state wins.** Sleep, lock, user switch or update outranks link symptoms.
24. **Hardware recovery is out-of-band.** The recovery hold works regardless of Desktop, assignment, sleep or restrictions.
30. **Continuous gestures reconcile at gesture boundaries.** External updates do not fight an active rotary gesture; confirmed state reconciles when it ends.
31. **Execution health is separate from transport health.** A link drop does not fail a confirmed host job.
33. **Recovery hold has absolute input arbitration.** Detents during a recovery hold never cancel it or run actions.
36. **Takeover states preempt active gestures.** A takeover cancels the gesture without waiting for its end.
38. **No ambiguous silence.** When it can render, every user-relevant condition has an intentional look.
40. **Short press is release-qualified.** A short press is ineligible once 500 ms passes.
42. **Only observed hardware input is actionable.** Kivori never reconstructs detents the device did not observe.
45. **Composite confirmation is capped.** A macro is never more confirmed than its least-confirmed required step.
46. **Input conditioning precedes gesture semantics.** Raw encoder edges may be filtered; only validated logical detents drive reversal and acceleration.
50. **Transient presentation restores current underlying truth.** A finished transient shows the real state (including Busy), not Idle.
54. **A normal rotary binding owns both directions.** `Rotate` reserves both directions; unrelated scopes never split them.
56. **Update artifacts are authenticated.** Nothing is installed without passing integrity checks.
58. **Update availability is not consent.** A newer version never silently triggers a disruptive flash.

## Product acceptance gates

An increment is aligned only if it keeps all ten. The roadmap cites them by number.

1. **No false confirmation:** an unverified outcome is never shown as confirmed success.
2. **No stale replay:** intent that expires during an interruption is never replayed later.
3. **No silent context mutation:** profile changes come from stable, explicit context.
4. **No cross-user leakage:** one OS user's config never stays active for another user.
5. **No hidden execution fallback:** unavailable actions never switch mechanism silently.
6. **No state invention:** Kivori shows observable state only.
7. **No passive-device ambiguity:** Passive devices never act like Active or Monitor devices.
8. **Recoverability:** recovery works without healthy desktop software; production hardware has a path that does not need valid app firmware.
9. **No ambiguous silence:** when Kivori can render, every user-relevant state has intentional feedback, never a frozen or accidental blank look.
10. **Compatibility-safe updates:** updates are authenticated, compatibility-checked, ordered safely, and leave a recovery path if they fail.
