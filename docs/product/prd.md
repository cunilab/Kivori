# Kivori Product Requirements Document

**Status:** Product baseline, revised with release phasing  
**Date:** 2026-09-16 (revised 2026-09-25)  
**Behavioral contract:** [`user-story-contract.md`](./user-story-contract.md)  
**Build order:** [`../roadmap.md`](../roadmap.md)

## 1. Product Summary

Kivori is a physical desktop companion that lets people control their computer through tactile inputs while visually representing important desktop state.

The product is built around a two-way relationship:

- **User -> Kivori -> Desktop**: the user performs a physical action and Kivori requests a desktop action.
- **Desktop -> Kivori -> User**: the desktop reports observable state and Kivori represents it through the buddy, overlays, indicators, and feedback.

Kivori is not a notification dashboard or a generic macro pad. It has a mascot, the **buddy**, and the buddy is central to the product: it is what makes Kivori an object people want on their desk rather than another peripheral. The buddy has personality, but its job is to make desktop state understandable, glanceable, and personal. Kivori is not a virtual pet with its own goals or progression. The buddy's expression always serves the desktop (see §9.5).

> **Product thesis:** Control the desktop physically. Understand the desktop visually.

## 2. Product Goal

Kivori should make frequent desktop interactions faster and more tangible without sacrificing trust or predictability.

The product should:

1. provide useful physical controls without requiring the user to leave their current task;
2. represent important desktop state without requiring another window to be opened;
3. acknowledge physical interaction immediately;
4. distinguish requested state from confirmed desktop state;
5. adapt controls to context without surprising the user;
6. remain understandable during disconnects, permissions problems, sleep, updates, and other edge conditions;
7. make waiting, processing, recovery, restriction, and failure states visibly intentional rather than appearing frozen or unresponsive.

The buddy represents the desktop. It should not claim a state that Kivori cannot observe or verify.

### 2.1 Success measures

Each release has one outcome that can be tested:

- **v1 (daily use):** the maintainer uses Kivori every workday for 10 consecutive workdays, on both a Windows and a macOS machine. In that time there is no restart, no reflash, and no opening of the configuration window except to change a setting on purpose.
- **v2 (beta):** a user who is not the maintainer installs, configures, updates, and recovers Kivori using only the shipped documentation.

Useful signals to watch: how often each control is used per day, input → feedback latency, the number of Unverified or Error outcomes, and the number of manual reconnects (zero is the target).

## 3. User Problem

Desktop users repeatedly perform small actions such as changing volume, muting a microphone, controlling media, launching commands, or triggering application actions. Existing options create tradeoffs:

- keyboard shortcuts are fast but provide little persistent physical feedback;
- software controls require visual attention and window switching;
- macro pads can trigger commands but often do not represent the resulting state;
- status information is fragmented across applications, tray icons, menus, and overlays.

Kivori addresses this by combining **physical control** and **persistent visual feedback** in the same desk object.

## 4. Core Experience

The canonical interaction loop is:

1. the desktop has an observable state;
2. Kivori represents that state;
3. the user interacts physically;
4. Kivori acknowledges the input immediately;
5. the desktop executes the action when permitted;
6. Kivori confirms only what can actually be verified;
7. Kivori reconciles the display with the real desktop state.

A responsive preview may appear before confirmation, but confirmed desktop state remains authoritative.

## 5. Product Principles

### 5.1 Desktop truth wins

Kivori represents observable desktop truth, regardless of whether a change originated from Kivori, keyboard, mouse, OS controls, application UI, or supported hardware.

### 5.2 Acknowledgement is not confirmation

Kivori may react immediately to input, but must not present an action as successful until the appropriate level of success can be observed.

### 5.3 Explicit intent beats inferred intent

Application context follows stable OS focus, not mouse hover or momentary focus theft. A physical gesture belongs to the context in which it started.

### 5.4 Predictability beats maximum flexibility

Kivori should avoid silent overrides, hidden fallbacks, replayed stale actions, implicit multi-device mirroring, or ambiguous context changes.

### 5.5 Uncertainty stays local

If one state cannot be verified, only that state should become unknown when possible. One uncertain capability should not make the entire device appear broken.

### 5.6 Recovery must remain available

Recovery must not depend on one healthy software layer.

Normal product recovery should include:

- a user-accessible MCU reboot path that does not depend on healthy desktop software;
- rollback/recovery behavior for interrupted or invalid firmware updates where the platform supports it;
- an application-independent hardware/ROM bootloader recovery path on production hardware so a corrupt application image does not permanently brick the device.

Factory reset remains separate from ordinary recovery.

### 5.7 Offline-first core

Core control, state representation, configuration, and device operation should not require cloud connectivity. Cloud profile synchronization is not required for v1 or v2.

Update discovery may use network connectivity, but already-installed core functionality should continue to operate without cloud access.

### 5.8 No ambiguous silence

Whenever Kivori is powered and capable of rendering, a user-relevant waiting, processing, restricted, transitional, failure, or recovery condition should have an intentional visual representation.

A blank, static, or unchanged presentation must not accidentally look like a frozen device. Motion may communicate that Kivori is alive and working, while the buddy, iconography, text, or state treatment communicates what the device is doing.

If progress cannot be measured truthfully, Kivori should show indeterminate activity rather than inventing a percentage or completion estimate.

Deliberate Display Sleep is the primary normal exception: the panel may be blank because blanking is itself the intended presentation. Low-level states in which the renderer is technically unavailable may also temporarily lack the normal visual surface, but Kivori should use any reliable hardware indication available and Kivori Desktop should communicate the state where possible.

### 5.9 Updates preserve compatibility and recoverability

Kivori Desktop and device firmware are one product system even when they version independently.

Update behavior should therefore:

- make desktop/firmware compatibility explicit before installation;
- authenticate update artifacts before installation;
- resolve required installation order before changing either side;
- avoid disruptive firmware flashing without clear user intent except where a future documented safety/security policy explicitly requires otherwise;
- preserve a recovery path if installation fails;
- communicate current phase, success, failure, rollback, and recovery intentionally.

A new version being available does not by itself justify immediately interrupting the user's work.

## 6. Primary Product Capabilities

### 6.1 Physical desktop control

Kivori supports explicit actions such as:

- system volume;
- media play/pause;
- microphone mute;
- keyboard shortcuts;
- application launch;
- scripts and macros;
- application-specific actions.

Actions have explicit scope. For example, `System Volume` and `Discord Volume` are different actions.

### 6.2 Application-aware profiles

The active application profile follows stable OS keyboard/window focus. The General profile applies when no application-specific profile is active.

Profiles can define different actions for the same physical input, but they do not silently change the meaning of an action.

### 6.3 Desktop state representation

The buddy and indicators may represent states such as:

- idle or active desktop;
- busy, success, error, or unknown primary state;
- microphone mute;
- master audio mute;
- media or call activity;
- connection/system health;
- restricted or unavailable capabilities.

### 6.3.1 Supported platforms

v1 supports **Windows and macOS** equally. A desktop capability is complete only when it works on both, or the PRD explicitly documents it as unavailable on one platform. When that happens, the unavailable platform shows the capability as unavailable rather than failing silently (§11).

Known differences between the platforms, which the product must handle:

- macOS needs **Accessibility** permission for keyboard shortcuts and focused-app detection. Before that permission is granted, it is a normal, expected Permission Required case (§11), not an error.
- The system volume, output device, and microphone mute APIs differ per platform; each platform needs its own backend behind the same action contract.
- Launch at login and tray / menu-bar presence use each platform's own mechanism.

Linux is outside v1.

### 6.4 Responsive physical feedback

Physical input should receive immediate local acknowledgement. The display may show local previews for continuous controls, then reconcile them against confirmed state.

### 6.5 Desktop configuration and update coordination

Kivori Desktop is the configuration center for:

- General controls;
- application profiles;
- explicit actions;
- rotary sensitivity and acceleration;
- global bindings;
- visual feedback, and buzzer/haptic feedback on hardware that has it (§19);
- ambient/display behavior;
- application targeting;
- device assignment;
- desktop application updates;
- firmware updates;
- installed desktop, firmware, and hardware-revision visibility;
- release/update status and compatibility guidance.

The configuration window does not need to remain visible during normal use.

Kivori Desktop should act as the update coordinator: it discovers applicable releases, evaluates desktop/firmware/hardware compatibility, presents release information, and performs update operations in a safe dependency order.

## 7. Interaction Model

### 7.1 Input model

v1 physical input is based on a rotary encoder with press behavior.

v1 uses one active gesture at a time. Simultaneous controls are not interpreted as implicit chords. Future combinations such as `Hold + Rotate` must be introduced as explicit input types.

Normal discrete button mappings are release-qualified and do not implicitly inherit OS key-repeat behavior. A user who maps a discrete action such as `Next Track` receives one action per valid discrete press/release.

Continuous or repeating button behavior must be an explicit action/input type rather than a hidden consequence of holding a discrete mapping.

### 7.2 Continuous controls and rotary scope

Rotary-capable actions may define:

- range;
- step size;
- sensitivity;
- optional acceleration;
- acceleration reset interval.

The default acceleration reset target is **250 ms** without a new detent.

Bounded values clamp immediately. Excess movement at a boundary may receive one subtle reaction, and reversing direction takes effect immediately.

A normal `Rotate` binding represents one bidirectional logical control. A Global `Rotate -> Master Volume` binding therefore owns both clockwise and counter-clockwise detents for that rotary gesture.

v1 must not silently compose unrelated mappings such as Global clockwise volume with Application counter-clockwise custom behavior. If future directional split binding is supported, it must be an explicit configuration mode with visible conflict/precedence rules.

### 7.3 Confirmation model

Kivori distinguishes:

- **State Confirmed**: resulting state is directly observable;
- **Execution Confirmed**: operation is known to have started or completed, but no persistent state exists;
- **Triggered / Unverified**: action was dispatched but final effect cannot be verified.

For ordinary interactive actions, initial product targets are:

- local acknowledgement: **< 50 ms**;
- delayed/processing indication: around **500 ms**;
- unresolved timeout: around **1,500 ms**.

Known failure becomes Error. Unknown outcome becomes Unverified.

## 8. Context Model

### 8.1 Active application

Application context is based on stable OS keyboard/window focus.

Mouse hover does not switch profiles. Momentary focus changes should not immediately switch profiles. Initial focus stabilization target: **300-500 ms**.

### 8.2 Gesture ownership

A gesture belongs to the context in which it began. If focus changes mid-rotation, the entire current gesture remains bound to its starting profile. The new profile applies to the next interaction.

### 8.3 Binding precedence

Global bindings are explicit reservations of their configured logical input scope.

For v1, a global bidirectional `Rotate` binding reserves the complete rotary gesture. An application profile may replace that whole logical rotary binding only through an explicit higher-precedence profile override; it does not partially steal one direction from the global binding.

### 8.4 Active OS user

Configuration belongs to the active OS user. One user's mappings must never carry into another user's desktop session.

Kivori Desktop is expected to operate in the user's normal session for v1 and v2 rather than requiring the entire product to run as a permanently privileged machine-wide service.

### 8.5 Machine scope

v1 profiles are local per machine and per OS user. Machine-local paths, scripts, device identities, and application installations are not cloud-synchronized.

## 9. Visual State Model

Kivori uses four visual layers (§9.1–§9.4), in priority order, above a personality layer (§9.5) that animates the buddy between and beneath them.

Every user-relevant state should look intentional. When Kivori can render, transitions such as Waiting, Reconnecting, Processing, Protected, Permission Required, Firmware Updating, recovery, and known failure should not be represented by unexplained visual silence or a frozen-looking frame.

When exact progress is unavailable, Kivori should prefer truthful indeterminate activity over invented progress percentages.

### 9.1 Takeover state

Takeover states replace normal desktop representation when normal interaction is unavailable or intentionally suspended.

Examples:

- Sleeping / Locked;
- Switching User;
- Protected;
- Permission Required;
- Host Starting / Resuming;
- Firmware Updating;
- Reconnecting;
- Disconnected;
- Waiting;
- Passive / Unassigned.

### 9.2 System health

System health indicates whether current desktop information can be trusted.

- Healthy does not require persistent screen space.
- Degraded receives dedicated visual priority and does not consume an ordinary secondary-indicator slot.

### 9.3 Primary buddy state

The buddy represents the main current desktop condition, such as:

- Idle;
- Active;
- Busy;
- Success;
- Error;
- Unknown.

Each primary state has a distinct buddy expression that stays recognizable at a glance, even when a secondary indicator or an overlay is visible at the same time.

### 9.4 Secondary indicators

Secondary indicators represent simultaneous supporting state, such as microphone mute, master audio mute, media activity, call activity, or application-specific state.

The UI should prioritize a small number of meaningful indicators instead of shrinking indefinitely to display every possible status.

### 9.5 Buddy personality layer

Beneath the state layers, the buddy has **personality**: idle motion, blinking, small reactions to input, and expressive transitions between states. Personality makes Kivori feel alive, and it is how Kivori avoids the "frozen device" look that §5.8 forbids.

Personality follows one rule: **it may animate freely, but it must never look like a desktop state.**

- A reaction to a knob turn, an idle fidget, or a playful self-play animation is allowed at any time.
- A personality animation must not borrow the look of a meaningful state. For example, a celebratory wiggle must not resemble Success, and a sleepy loop must not resemble Sleeping / Locked while the host is awake.
- Personality always gives way to the higher layers. A takeover state, a Degraded health signal, a primary-state change, or an input overlay takes priority immediately.
- Personality settings (such as a calmer or livelier buddy) change only how strongly the buddy moves, never which states it reports.
- In the Low Motion and Display Sleep idle stages (§12), personality motion is reduced or stopped first.

## 10. Connection and Session State Model

### Connected

A usable desktop session exists and this Kivori is assigned for normal use.

### Waiting

Kivori has power but no usable Kivori Desktop session is available. Examples include wall power, a secondary PC without Kivori Desktop, or an intentionally closed desktop app.

### Passive / Unassigned

A desktop session exists, but this Kivori is not the active assigned controller. Passive is not Waiting and is not a visual Monitor Mode.

### Host Starting / Resuming

The host is starting or waking and Kivori is allowing the user-session software time to become available. Initial grace target: **15-30 seconds**.

### Switching User

The OS is transitioning between user sessions. Kivori should preserve the transition state until the next user session is confirmed or determined unavailable.

### Reconnecting

The desktop service is intentionally restarting, such as during an update. Inputs are not buffered for later execution.

### Degraded

Communication is unhealthy but partially available.

### Disconnected

A previously healthy connection disappears unexpectedly.

### Sleeping / Locked

The host is known to be sleeping, hibernating, or locked. Known host/session state outranks generic communication-loss symptoms.

## 11. Restricted and Permission-Limited Operation

Kivori must distinguish connection failure from capability restriction.

When Kivori Desktop remains connected but an OS permission is missing or revoked:

- unaffected capabilities continue working;
- affected capabilities become unavailable;
- blocked actions are not repeatedly retried;
- a broad restriction may escalate to a Permission Required takeover state.

Normal custom actions are suspended in protected or locked contexts for v1 and v2.

## 12. Display Idle and Burn-In Protection

If the host remains awake while its monitor blanks, Kivori remains Connected and changes presentation rather than connection state.

Presentation may progress through:

**Normal -> Dim -> Low Motion -> Display Sleep**

Protection may include lower brightness, reduced animation, subtle pixel repositioning, and eventual full display blanking.

When fully in Display Sleep, only deliberate configured controls wake the display by default. The first deliberate interaction wakes the display only and does not execute the assigned desktop action. In Dim or Low Motion states, interaction executes normally while restoring full presentation.

Display Sleep is intentionally blank; it must not be confused with an unexplained blank caused by a normal waiting, processing, or error condition.

## 13. Feedback and Do Not Disturb

Kivori distinguishes user-initiated feedback from unsolicited background feedback.

- user-initiated acknowledgement follows Kivori's own feedback settings;
- background buzzer feedback respects OS Do Not Disturb / Focus Mode by default.

The current hardware has no buzzer (§19). Until a hardware revision adds one, feedback is visual only, and buzzer settings are not shown.

## 14. Device Recovery and Update System

### 14.1 Normal MCU recovery

v1 should provide a software-independent-from-desktop recovery gesture:

**Hold the primary hardware button for approximately 10 seconds -> force MCU reboot.**

Recovery must not erase configuration. Factory reset must use a separate, harder-to-trigger procedure.

Recovery should visibly acknowledge the hold and, where practical, communicate that the device is progressing toward reboot rather than appearing stuck.

### 14.2 Application-independent hardware recovery

Production Kivori hardware must provide a recovery path that does not depend on the installed Kivori application firmware being valid.

For the current ESP32-C3 class of hardware, this means exposing a documented mechanism to enter the MCU's ROM/download bootloader and reset the device even when the normal application image cannot boot. The production PCB should preserve an accessible service/user recovery mechanism and test access appropriate to the final enclosure and support model.

Kivori Desktop should be able to detect/support a documented recovery workflow when the device is in this low-level recovery mode.

### 14.3 Recoverable firmware installation

Normal firmware update should use a rollback-safe strategy where supported, such as inactive-slot/A-B installation with post-boot validation.

A failed or invalid new image should recover to the previous known-good firmware automatically when the platform can do so. The ROM/bootloader recovery path remains the last-resort recovery when application-level rollback cannot run.

### 14.4 Update discovery and compatibility

Kivori Desktop is the update coordinator for both desktop and firmware releases.

A release source/manifest should describe enough information to make safe decisions, including:

- desktop version and platform artifacts;
- firmware version;
- supported hardware revision(s);
- minimum/compatible desktop version for firmware;
- minimum/compatible firmware version for desktop behavior where required;
- release notes;
- update importance/severity;
- artifact integrity/authentication metadata.

Desktop and firmware may version independently, but Kivori must evaluate compatibility before installation.

If both desktop and firmware updates are required, `Update All` must determine and communicate the required dependency order before installing either component.

### 14.5 Desktop application updates

When a desktop update is available, Kivori Desktop should clearly communicate:

- installed version;
- available version;
- release notes/important changes;
- whether the update is optional, recommended, critical, or compatibility-required;
- whether restart is required.

Desktop update packages must be authenticated before installation.

During an intentional desktop restart/update, the physical Kivori should use the existing intentional Reconnecting/Host Updating semantics rather than presenting the event as an unexplained crash.

### 14.6 Firmware update UX

Firmware update is an intentional takeover state and must not look like an unexplained connection failure.

Before flashing, Kivori Desktop should validate at least:

- target device identity/hardware revision;
- firmware compatibility with the installed/target desktop version;
- update artifact authenticity/integrity;
- usable connection/recovery prerequisites.

Update availability alone must not silently trigger a disruptive firmware flash during normal user activity. Normal/recommended updates should require clear user intent. Future critical/security policy may impose stronger requirements only if explicitly documented.

While the normal renderer is available, the device should show an update/activity presentation. Trustworthy phase/progress may be shown; when exact progress is not trustworthy, Kivori should show indeterminate activity rather than fake percentage progress.

Useful phases include Preparing, Downloading, Firmware Updating, Verifying, Restarting, Restored Previous Firmware, and Recovery Mode where applicable.

If the renderer becomes unavailable during a low-level update, Kivori should use the simplest reliable hardware indication available, while Kivori Desktop shows update state/progress where possible.

### 14.7 Update failure behavior

Update failure must resolve into an understandable path rather than ambiguous silence.

Examples:

- if the previous firmware remains valid, report that the update failed and the previous firmware was restored;
- if the device enters low-level recovery/ROM bootloader mode, Kivori Desktop should identify recovery mode and offer a restore flow;
- if the device cannot be detected automatically, Desktop should provide the documented physical bootloader/recovery procedure;
- failed update state must never be presented as successful solely because the device restarted.

## 15. Multi-Device Behavior

v1 supports **one Active Kivori per desktop session**.

Additional connected units remain **Passive / Unassigned**. They must not silently mirror desktop state, execute mappings, or become visual-only monitors.

Future multi-device support may introduce explicit roles, including a separate Monitor Mode, but this is outside v1 and v2.

## 16. Release Scope

The capabilities in §6–§15 describe the complete product. They ship in two releases so that the core loop is proven in daily use before any investment in distribution. The build order within each release is in the [roadmap](../roadmap.md).

### 16.1 v1: Daily use (maintainer, Windows + macOS)

v1 proves the product loop on real desks. It includes:

- one Kivori connected to one active desktop session, on Windows and macOS;
- rotary + button physical input;
- system volume, media, microphone mute, master mute, keyboard shortcut, and app launch actions;
- immediate input acknowledgement and honest confirmation semantics (§7.3);
- real desktop state synchronization where observable, including changes made outside Kivori;
- General profile and application-aware profiles with stable focus and gesture ownership;
- desktop configuration UI with local per-user, per-machine configuration;
- buddy + personality + takeover + system-health + secondary-indicator presentation model;
- intentional visual feedback for waiting, processing, restriction, transition, failure, and recovery states;
- connection/session recovery across sleep, wake, lock, and reconnect;
- Permission Required handling, especially the macOS Accessibility flow;
- safe display-idle and burn-in behavior;
- the normal MCU reboot gesture;
- launch at login and tray / menu-bar presence;
- visible desktop and firmware versions, plus user-initiated firmware flashing of the bundled build.

### 16.2 v2: Beta (other users)

v2 makes Kivori safe to hand to someone else. It adds:

- an authenticated release manifest, compatibility evaluation, and `Update All` ordering (§14.4);
- desktop application installers and authenticated updates (§14.5);
- rollback-safe (A/B) firmware installation with post-boot validation (§14.3);
- production hardware with an application-independent recovery path, and Desktop detection of recovery mode (§14.2);
- hardware revision identity reported to Desktop;
- OS user switching and Passive handling of additional devices (§8.4, §15);
- a factory reset procedure separate from recovery.

Until v2, firmware updates are manual and user-initiated. The v1 flow must still never present a failed flash as success (gate 10 applies in its v1 form: flashing is explicit, the bundled build is known to be compatible, and failure is reported).

## 17. Non-Goals

The following are intentionally outside the product contract for v1 and v2:

- cloud profile synchronization;
- independent virtual-pet progression (needs, levels, or goals that don't come from the desktop);
- generic news/weather/crypto dashboard features;
- workflow orchestration as a flagship concept;
- automatic inference of arbitrary web apps or scripts from window titles;
- implicit physical-input chords;
- implicit OS-style key-repeat for ordinary discrete button mappings;
- silently mixing opposite rotary directions between unrelated Global and application-profile actions;
- replaying actions after a connection recovers;
- multiple simultaneously active Kivori controllers;
- Passive devices acting as Monitor Mode;
- driverless macro/media fallback when Kivori Desktop is unavailable;
- waking the host PC through Kivori;
- guaranteeing visibility into hardware states the OS/device does not expose;
- silently flashing normal/recommended firmware updates merely because a newer version exists;
- installing unauthenticated desktop/firmware artifacts;
- installing a desktop/firmware pair without validating known compatibility constraints.

## 18. Product Acceptance Gates

A product increment is aligned with this PRD only if it preserves these invariants:

1. **No false confirmation:** Kivori never presents unverified action outcome as confirmed success.
2. **No stale replay:** physical intent that expires during interruption is not replayed later.
3. **No silent context mutation:** app/profile changes are based on stable, explicit context.
4. **No cross-user leakage:** one OS user's configuration never remains active for another user.
5. **No hidden execution fallback:** unavailable actions do not silently switch to another mechanism.
6. **No state invention:** Kivori represents observable state only.
7. **No passive-device ambiguity:** Passive devices do not behave like Active or Monitor devices.
8. **Recoverability:** recovery remains possible without healthy desktop software, and production hardware preserves a path that does not depend on valid application firmware.
9. **No ambiguous silence:** when Kivori can render, user-relevant waiting, processing, restricted, transitional, failure, and recovery states have intentional visual feedback rather than appearing frozen or accidentally blank.
10. **Compatibility-safe updates:** desktop and firmware updates are authenticated, compatibility-checked, ordered safely, and leave a defined recovery path if installation fails.

Detailed normative behavior and acceptance criteria live in [`user-story-contract.md`](./user-story-contract.md).

## 19. Hardware

### 19.1 Current hardware (v1)

- ESP32-C3 with native USB (power, flashing, and data link over one cable);
- ST7789 240×240 display;
- HW-040 rotary encoder with a push button, which is the only physical input surface;
- no buzzer, no haptics, no ambient light sensor.

One knob and one button is a narrow input surface. v1 compensates with explicit profiles (§6.2) rather than hidden gestures or chords (§7.1).

### 19.2 Hardware direction

Future hardware revisions are expected to add, driven by what v1 daily use shows is missing:

- an accessible recovery / boot control on the production PCB (required for v2, §14.2);
- buzzer or haptic feedback (§13);
- possibly additional inputs, introduced as explicit input types rather than implicit chords;
- an enclosure suited to long-term desk use.

Every hardware revision reports its identity to Desktop, so that compatibility (§14.4) and available capabilities (such as whether a buzzer exists) are known and never assumed.

