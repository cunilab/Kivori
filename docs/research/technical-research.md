# Research: Kivori Product Technical Implementation

**Date**: 2026-09-16  
**Status**: Technical research and suggested implementation guidance  
**Product requirements**: [`../PRD.md`](../PRD.md)  
**Behavior contract**: [`./user-story-contract.md`](./user-story-contract.md)  
**Current foundation**: [`architecture.md`](../features/001-device-connection-foundation/architecture.md), [`research.md`](../features/001-device-connection-foundation/research.md)

**Purpose**: Research technical approaches that can implement the Kivori PRD and User Story Contract on the current repository foundation, with explicit consideration for Windows, macOS, Linux/X11, Linux/Wayland, ESP32-C3 firmware, release/update delivery, hardware recovery, permissions, multi-user ownership, and validation constraints. Each research item is stated as **Decision · Rationale · Alternatives · Caveats**, following the style of the Feature 001 research document.

This document is **non-normative technical guidance**. The PRD and User Story Contract define what users must be able to rely on. A technical recommendation here is a researched starting point, not an immutable implementation mandate. It may be challenged or replaced during implementation when platform documentation, prototype results, hardware measurements, security findings, maintainability evidence, dependency constraints, or automated/manual testing demonstrate a better approach, provided the relevant PRD/User Story behavior remains satisfied. Material architectural deviations should be recorded in an ADR or by updating this research.

Recommendation labels used below:

- **CURRENT** — already present in the repository and supported by current code/evidence.
- **SUGGESTED** — preferred starting implementation based on current evidence.
- **CANDIDATE** — plausible alternative that remains worth evaluating.
- **SPIKE REQUIRED** — evidence is insufficient to freeze a technical choice.
- **PLATFORM-LIMITED** — the operating system/runtime does not expose equivalent capability everywhere.
- **REJECT FOR NOW** — conflicts with current product constraints or adds unjustified complexity today.

---

## R-1. Preserve the Feature 001 foundation; evolve around it

**Decision**: **SUGGESTED** — preserve the current split workspaces, shared `no_std` crates, deterministic renderer, USB protocol, device session code, least-privilege Tauri boundary, and testable firmware ports. Add the product-level architecture around these pieces rather than replacing them.

**Rationale**: The repository already has useful hard boundaries: firmware is isolated from the host Cargo workspace; `kivori-model`, `kivori-protocol`, `kivori-framebuffer`, `kivori-renderer`, and `kivori-assets` are shared by host and device; the firmware core is written against `Clock`, `Transport`, and `DisplaySink`; and the desktop native core owns serial communication instead of the React webview.

**Alternatives**: Rewrite around a new async firmware framework, replace Tauri, or collapse host/device models into one application-specific crate. Rejected for now because none resolves a demonstrated blocker and each would discard tested foundations.

**Caveats**: Feature 001 solves device connection and canonical rendering, not the whole product contract. Existing state types and the current `Orchestrator` must not become catch-all product state merely because they already exist.

## R-2. Keep Tauri as the per-user product host, but do not assume it can always own USB directly

**Decision**: **SUGGESTED / CONDITIONAL** — continue using the Tauri v2 native Rust process as the per-user Kivori application host. Direct serial ownership by that process is acceptable on ordinary single-user machines, but a machine-level hardware broker becomes necessary if the product promises deterministic Fast User Switching takeover despite arbitrary suspension of an old user process.

**Rationale**: Tauri already provides the desired system/webview separation and the native process survives window hide/reload. But a suspended process can retain an exclusive serial handle, and another ordinary process cannot force that handle closed.

**Alternatives**: Introduce a machine-wide broker immediately. This is a first-class **CANDIDATE**, not mandatory if the product only promises fail-closed privacy plus best-effort reassignment.

**Caveats**: A future broker should remain minimal: USB ownership, active-session arbitration, and update ownership. AppCore, profiles, bindings, and user configuration should remain per-user unless evidence requires otherwise.

## R-3. Build a modular AppCore rather than growing `device_task` into a god loop

**Decision**: **SUGGESTED** — introduce product-level modules inside `apps/desktop/src-tauri` while interfaces stabilize: `ContextEngine`, `BindingResolver`, `ActionEngine`, `ExecutionTracker`, `PresentationResolver`, `DeviceRegistry`, `SessionOwnershipGate`, `ConfigStore`, and `UpdateCoordinator`.

**Rationale**: The current `device_task` correctly owns one serial session and the current `Orchestrator` correctly owns one desired companion state. The User Story Contract now spans context, permissions, profiles, macros, jobs, multi-device assignment, display policy, and updates. Putting all of these into the connection actor would couple unrelated lifecycles.

**Alternatives**: Immediately split each concept into its own Cargo crate. Rejected for now; module boundaries should prove themselves first.

**Caveats**: “Modular AppCore” does not require an actor framework. Direct typed calls, bounded channels, or task-owned state are all acceptable if invariants remain testable.

## R-4. Model product truth as orthogonal state axes

**Decision**: **SUGGESTED** — keep independent state domains rather than creating one giant `KivoriState` enum. At minimum separate transport, host session, assignment, restriction/permission, execution, display power, update, persistent indicators, and transient feedback.

**Rationale**: Multiple facts can be true simultaneously: transport may be Degraded while a host build remains Running, microphone is muted, display is Dim, and device remains Assigned.

**Alternatives**: Expand `ConnectionState` with product states such as `Protected`, `Passive`, `FirmwareUpdating`, and `Running`. Rejected because these are different axes.

**Caveats**: The exact Rust representation is challengeable. Independent lifecycles and truthful reconciliation are the architectural requirement.

## R-5. Use a pure PresentationResolver for visual priority and “no ambiguous silence”

**Decision**: **SUGGESTED** — implement a deterministic function conceptually equivalent to `PresentationResolver(ProductSnapshot) -> PresentationSnapshot`, with the four-layer User Story hierarchy encoded as explicit rules and tests.

**Rationale**: Kivori's visual behavior is a reconciliation problem. A pure resolver can prove that disconnect preempts a gesture, a Volume transient expires back into a still-running job, urgent indicators preempt ordinary ones, and Display Sleep is intentionally blank while Waiting is not.

**Alternatives**: Let each subsystem directly command buddy state. Rejected because independent subsystems would race and stale transients could overwrite current truth.

**Caveats**: Firmware owns local states below Desktop such as Booting, recovery hold, low-level update phases, Display Sleep, and host-link absence. Final presentation may merge Desktop semantics with firmware-owned overrides.

## R-6. Report platform capability explicitly instead of pretending OS feature parity

**Decision**: **SUGGESTED** — define runtime capability states such as `Supported`, `SupportedWithPermission`, `SupportedWhenBackendAvailable`, `TriggeredButUnverified`, and `Unsupported`.

**Rationale**: Windows, macOS, X11, and Wayland expose materially different capabilities. Kivori's observable-truth rule requires these differences to be represented instead of hidden behind a boolean platform adapter.

**Alternatives**: A single `PlatformAdapter` returning success/failure for every abstract action. Rejected because it erases permission and observability differences.

**Caveats**: Capability state can change while Kivori runs. Backends must refresh after permission, audio-device, compositor, session, or portal changes.

## R-7. SessionOwnershipGate is normal-path arbitration, not a hard guarantee against frozen handle owners

**Decision**: **SUGGESTED with explicit limitation** — only the active interactive user's Kivori process should be eligible to acquire serial devices. The outgoing process should release hardware and invalidate session-private presentation when session-resign notifications arrive. This is best-effort ownership when the handle lives in a suspendable per-user process.

**Rationale**: Windows Fast User Switching guidance explicitly mentions shared resources such as serial ports. macOS switched-out applications continue running. Linux logind exposes active sessions. But an already-open exclusive handle remains owned until it is closed; if the process is frozen before it runs cleanup, an incoming user cannot force ordinary takeover.

**Alternatives**: A machine-level `HardwareBroker` that owns Kivori independently of GUI-session suspension. This becomes required if the product promises deterministic availability after arbitrary old-session suspension.

**Caveats**: Privacy and availability are separate guarantees. R-79 defines a firmware host lease that can fail closed for privacy even when new-user acquisition remains blocked.

## R-8. Replace `first_candidate()` with DeviceRegistry + one DeviceActor per physical device

**Decision**: **SUGGESTED** — evolve discovery into a `DeviceRegistry` that normalizes physical devices, owns one actor/session per Kivori, and applies product assignment separately from transport health.

**Rationale**: The current runtime opens only the first allowlisted candidate, while the product contract permits several connected Kivori units with only one Active controller.

**Alternatives**: Keep one open serial device and ignore extras. Rejected because ignored devices cannot show Passive/Unassigned truth or be assigned reliably.

**Caveats**: Port names are not identity. Normal application mode should ultimately trust verified handshake identity.

## R-9. Give every physical Kivori a product DeviceId and an immutable HardwareId

**Decision**: **SUGGESTED** — replace the current hard-coded `DeviceId`. Use a Kivori product `DeviceId` for assignment/friendly identity and also expose an immutable hardware identifier derived from factory MCU identity, such as the ESP32-C3 factory MAC/eFuse value.

**Rationale**: `DeviceId` is useful for Kivori product lifecycle; `HardwareId` is useful for recovery/update correlation because the ROM bootloader can expose chip identity even when Kivori application firmware is unavailable.

**Alternatives**: Use port path, VID/PID, or a short UI hash as persistent identity. Rejected: these do not uniquely and stably identify a product unit.

**Caveats**: Neither identifier is automatically an authentication credential. If either becomes security-sensitive, provision cryptographic identity separately.

## R-10. Preserve the current protocol and extend it through version/capability negotiation

**Decision**: **SUGGESTED** — evolve `kivori-protocol` rather than replacing it. Add append-only message variants gated by negotiated capabilities and reserve major-version changes for genuinely incompatible wire semantics.

**Rationale**: Existing framing already provides COBS resynchronization, CRC, protocol versioning, sequence classification, and a capability bitset.

**Alternatives**: Switch to JSON, protobuf, HID reports, or a new RPC framework. None currently resolves a demonstrated deficiency large enough to replace tested framing.

**Caveats**: Current capability bits are empty. New bits need central allocation and compatibility tests.

## R-11. Separate ephemeral/latest-wins traffic from reliable update transactions

**Decision**: **SUGGESTED** — ordinary input/presentation remains current-truth oriented; application-level firmware transfer gets its own explicit reliable transaction with image ID, offsets, acknowledgements, and integrity checks.

**Rationale**: A detent lost during disconnect must not execute later, while firmware bytes cannot simply be discarded after a gap.

**Alternatives**: Retransmit every Kivori frame automatically. Rejected because generic reliability creates stale-input replay.

**Caveats**: Presentation should carry a revision/current-state identity. Firmware resumption must be scoped to the same update transaction.

## R-12. Add explicit physical input and gesture messages

**Decision**: **SUGGESTED** — device-originated input should carry enough identity/timing for atomic gestures and no-stale-replay semantics, conceptually including `event_id`, `gesture_id`, `control_id`, kind/delta, and a device monotonic timestamp.

**Rationale**: Context commitment, target-loss cancellation, wake-only gestures, recovery arbitration, acceleration reset, and single-gesture ownership depend on knowing which events belong to one interaction.

**Alternatives**: Send only raw rotate/button events with no gesture identity. Rejected because Desktop would have to reconstruct gesture ownership from transport timing.

**Caveats**: Rapid rotary input may be batched, but ordered direction segments and enough timing must be retained.

## R-13. Keep semantic presentation on the wire; do not stream pixels in normal operation

**Decision**: **SUGGESTED** — extend the protocol with a semantic `PresentationSnapshot` instead of streaming 240×240 frame buffers for normal operation.

**Rationale**: Semantic traffic is compact and lets firmware retain local boot/recovery/update truth even if Desktop disappears. The shared deterministic renderer remains a strong foundation.

**Alternatives**: Desktop renders every physical frame. Rejected for bandwidth, host dependency, and recovery fragility.

**Caveats**: The semantic schema must express takeover, health, primary buddy, indicators, transients, truthful activity, and real progress when known.

## R-14. Firmware owns raw electrical truth and recovery; Desktop owns action meaning

**Decision**: **SUGGESTED** — firmware owns input conditioning, logical detent formation, button timing, wake ownership, gesture arbitration, and MCU recovery. Desktop owns profile/context selection, action identity, sensitivity, acceleration policy, ranges, and OS execution.

**Rationale**: Recovery/input validity must work when Desktop is absent, while sensitivity and acceleration are action-specific.

**Alternatives**: Put all timing on Desktop or all action semantics in firmware. Both are rejected for the baseline architecture.

**Caveats**: The boundary is stable; PCNT, GPIO interrupts, sampling, and analog/digital conditioning remain challengeable implementation choices.

## R-15. HW-040 decoding: PCNT alone is not a sufficient debounce plan for millisecond mechanical bounce

**Decision**: **SPIKE REQUIRED** — compare at least GPIO edge/sampling plus quadrature state machine, PCNT after external conditioning, and hybrid qualification. Do not assume PCNT's internal filter solves HW-040 bounce.

**Rationale**: PCNT's filter threshold is bounded to 1023 APB cycles; at 80 MHz that is about 12.8 µs, far below millisecond mechanical bounce.

**Alternatives**: Poll in the main render loop. Rejected. PCNT-only remains acceptable only if measurements prove the board signal is already sufficiently conditioned.

**Caveats**: The decoder should expose invalid-transition/quality counters instead of inventing motion.

## R-16. Implement button/recovery behavior as a firmware gesture state machine

**Decision**: **SUGGESTED** — model the primary button explicitly, for example `ShortCandidate -> HoldArmed -> RecoveryOwned -> Reboot`, with monotonic deadlines and release-qualified action emission.

**Rationale**: This directly supports short press, Hold, recovery ownership, 10-second reboot, wake-only behavior, and suppression of auxiliary actions during recovery.

**Alternatives**: Let Desktop decide the 10-second recovery action. Rejected because recovery must exist below host software.

**Caveats**: Intermediate timing may change after usability testing; rendering should consume semantic recovery progress rather than duplicate timer logic.

## R-17. Compute rotary acceleration only from validated logical detents

**Decision**: **SUGGESTED** — firmware reports completed logical detents and timing; Desktop computes action-specific sensitivity/acceleration. A legal electrical Gray transition is not itself a detent.

**Rationale**: Bounce may move between adjacent Gray states without completing a mechanically meaningful detent. Acceleration should only see `ValidatedDetent(CW/CCW)` produced after complete phase/detent qualification.

**Alternatives**: Treat every valid electrical transition as a logical tick. Rejected because partial bounce sequences would repeatedly disturb acceleration.

**Caveats**: If a degraded encoder produces an entire false-but-valid reverse detent sequence, Kivori cannot distinguish that from real reversal. Hardware/decoder quality must make such completed false detents sufficiently rare; R-82 tightens this model.

## R-18. Use typed action attempts/outcomes, not `execute() -> bool`

**Decision**: **SUGGESTED** — represent execution with an ID, committed context, confirmation strategy, lifecycle, and outcome such as `Running`, `StateConfirmed`, `ExecutionConfirmed`, `TriggeredUnverified`, and `Failed`.

**Rationale**: Confirmation quality depends on action/platform. Master volume can be read back, synthetic shortcuts often cannot, processes can be spawned while a later postcondition remains pending.

**Alternatives**: Treat no API error as success. Rejected because many OS APIs do not prove downstream effect.

**Caveats**: Confirmation strategy is resolved at execution time, not solely from abstract action type.

## R-19. Give macros their own runner and composite outcome model

**Decision**: **SUGGESTED** — implement macros as sequential ordinary action attempts with required/optional semantics, waits, cancellation, and derived overall status. Do not add hidden transactional rollback.

**Rationale**: Required known failure produces Partial Failure; required Unverified caps the composite at Unverified; earlier side effects remain desktop truth unless compensation is explicit.

**Alternatives**: Compile macros into opaque scripts. Valid as a separate script action, not as the only first-class macro representation.

**Caveats**: Future explicit compensation is different from automatic rollback.

## R-20. Separate ExecutionTracker from device transport and seat ownership

**Decision**: **SUGGESTED** — host-side jobs should live in an `ExecutionTracker` whose lifetime is not owned by USB `Session` or active Kivori device assignment.

**Rationale**: A confirmed build/script/process can continue while the physical device is released because the user switched seats or transport is unavailable. Device ownership and job observation are separate lifecycles.

**Alternatives**: Store execution only inside the DeviceActor. Rejected because releasing USB would destroy host truth.

**Caveats**: If the OS suspends the Kivori Desktop process itself, live observation is suspended too. On resume, reconcile through persistent observable job/process identity; otherwise classify as Unverified rather than invent continuity. R-81 expands this.

## R-21. Windows bindings: isolate Win32/COM/WinRT behind a dedicated backend

**Decision**: **SUGGESTED** — use Microsoft's `windows` Rust crate as the primary typed binding layer and isolate unsafe/COM/thread-affine code under `platform/windows` or equivalent.

**Rationale**: Windows integration spans Win32, Core Audio COM, and WinRT. Keeping these types out of AppCore preserves portability.

**Alternatives**: Hand-written bindings or many small FFI crates. Possible for narrow APIs, but less consistent.

**Caveats**: Feature-gate narrowly and own COM apartment/thread rules inside the backend.

## R-22. Windows foreground observation: event hook, not high-frequency polling

**Decision**: **SUGGESTED** — use `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, ..., WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS)` on a message-loop thread and publish candidates into `ContextEngine`.

**Rationale**: Out-of-context hooks avoid target-process injection and fit the non-invasive profile requirement.

**Alternatives**: High-frequency `GetForegroundWindow()` polling. Rejected as the primary path; readback remains useful for reconciliation.

**Caveats**: Foreground HWND alone does not prove secure/protected context or overlay ownership.

## R-23. Windows workspace/session observation: use public APIs only

**Decision**: **SUGGESTED** — use WTS session notifications, public virtual-desktop membership APIs, power-setting notifications, and input-desktop checks where needed.

**Rationale**: These signals are sufficient for stable app profiles and protected/session transitions without relying on private Shell APIs.

**Alternatives**: Undocumented virtual-desktop COM APIs. Rejected for baseline behavior.

**Caveats**: No single Windows API says “injection is safe.” Security classification remains an aggregate decision.

## R-24. Windows master volume/mute: Core Audio endpoint state with callbacks

**Decision**: **SUGGESTED** — use `IAudioEndpointVolume` plus callback/default-device observation for master render/capture endpoint controls.

**Rationale**: Mutation plus readback/callback supports `StateConfirmed` rather than assuming dispatch success.

**Alternatives**: Multimedia-key injection. Useful only as a weaker fallback.

**Caveats**: Default endpoints change. Rebind on system notification rather than silently pinning old hardware.

## R-25. Windows app audio: Core Audio sessions, with explicit ambiguity handling

**Decision**: **SUGGESTED** — investigate `IAudioSessionManager2`, `IAudioSessionControl2`, and `ISimpleAudioVolume` for app-specific audio.

**Rationale**: Windows exposes first-class per-session controls and process metadata.

**Alternatives**: UI automation or media keys. Rejected as primary app-volume implementation.

**Caveats**: Sessions may span processes and one app may own multiple sessions; matching needs an aggregation policy.

## R-26. Windows microphone activity/state: treat capture-session mapping as a spike

**Decision**: **SPIKE REQUIRED** — evaluate capture endpoints/sessions for microphone activity and process association, separating endpoint mute from active capture.

**Rationale**: Kivori's privacy indicators must be based on observable state, not app-name guesses.

**Alternatives**: Infer activity from focused conferencing apps. Rejected.

**Caveats**: Windows' own privacy UI may have information unavailable to ordinary apps; incomplete coverage must be represented honestly.

## R-27. Windows shortcuts: `SendInput` as Triggered/Unverified fallback

**Decision**: **SUGGESTED** — use `SendInput` for eligible generic shortcuts but classify as `Triggered/Unverified` unless an independent postcondition confirms the effect.

**Rationale**: UIPI may block higher-integrity targets without a reliable explicit reason.

**Alternatives**: Run Kivori elevated merely to inject shortcuts. Rejected for MVP.

**Caveats**: Protected/secure context must be classified before dispatch.

## R-28. Windows global media: GSMTC is promising but packaging must be proven

**Decision**: **SPIKE REQUIRED** — test `GlobalSystemMediaTransportControlsSessionManager` in the actual shipped Tauri packaging model.

**Rationale**: GSMTC can expose observable media sessions and stronger confirmation than key simulation.

**Alternatives**: Synthetic media keys remain a Triggered/Unverified fallback.

**Caveats**: The `globalMediaControl` capability/distribution behavior must be proven in Kivori's packaging model.

## R-29. Windows DND: model interruption suitability, not an invented exact DND toggle

**Decision**: **SUGGESTED** — use documented notification-suitability signals such as `SHQueryUserNotificationState` as one input to feedback policy.

**Rationale**: The API exposes useful interruption states but not a guaranteed mirror of every modern Windows DND/Focus setting.

**Alternatives**: Scrape undocumented registry/shell state. Rejected.

**Caveats**: User-facing copy should state exactly what Kivori respects.

## R-30. Windows overlay detection: conservative classification from OS-visible facts

**Decision**: **SUGGESTED** — classify overlays from multiple signals such as foreground ownership, process identity, relevant styles, DWM visibility/cloaking, short transition history, known rules, and explicit user overrides.

**Rationale**: No single style or flag proves semantic overlay ownership.

**Alternatives**: Process injection, DLL hooks, memory inspection, or anti-cheat integration. Rejected.

**Caveats**: Unknown overlays fall back to normal stabilization; diagnostics should avoid logging unnecessary sensitive content.

## R-31. macOS app-level foreground profiles: NSWorkspace first

**Decision**: **SUGGESTED** — use `NSWorkspace.didActivateApplicationNotification` / `NSRunningApplication` for app-level focus without requiring Accessibility permission.

**Rationale**: App-level profile selection is available through public AppKit signals.

**Alternatives**: Poll frontmost application or require Accessibility for all profiles. Rejected as defaults.

**Caveats**: Window-level matching is a separate permission-gated capability.

## R-32. macOS window-level context: Accessibility is permission-gated

**Decision**: **SUGGESTED** — treat Accessibility-dependent window inspection/input as `SupportedWithPermission`, using supported trust checks and requesting permission only when needed.

**Rationale**: This preserves least privilege while allowing app-level profiles without broad UI access.

**Alternatives**: Make Accessibility mandatory during onboarding. Rejected unless later scope proves necessary.

**Caveats**: Permission can be revoked while running; capability state must refresh independently of transport.

## R-33. macOS Spaces/session transitions: notifications are hints that must reconcile to current truth

**Decision**: **SUGGESTED** — observe Space/session/wake notifications, but treat them as event ingress only. Minimal callback data should be moved into a bounded channel while `ContextEngine` and `SessionOwnershipGate` perform stabilization and reconciliation outside the callback.

**Rationale**: This preserves AppKit/Tauri main-run-loop responsiveness and avoids committing stale context merely because a notification arrived late.

**Alternatives**: Resolve profiles/device I/O directly inside NSWorkspace callbacks. Rejected.

**Caveats**: Every passive commit should confirm the latest observation revision/current foreground before applying; R-83 defines revision semantics for rapid switching and physical interaction.

## R-34. macOS system audio: Core Audio, but validate controllable-volume semantics

**Decision**: **SUGGESTED / SPIKE REQUIRED** — use supported Core Audio hardware APIs for default device observation and available volume/mute controls.

**Rationale**: Core Audio is the supported system-level stack and can provide readback where the device exposes writable controls.

**Alternatives**: AppleScript/UI automation as the primary system path. Rejected.

**Caveats**: Some output devices do not expose software master-volume controls; unsupported must be reported honestly.

## R-35. macOS microphone/process audio observability: modern Core Audio process APIs are promising

**Decision**: **SPIKE REQUIRED** — evaluate process audio activity metadata against desired microphone/call indicators and minimum supported macOS version.

**Rationale**: Modern APIs may provide process-level input/output activity without guessing from focus.

**Alternatives**: Infer microphone activity from known app names. Rejected.

**Caveats**: API availability can raise the deployment target and must be considered with release policy.

## R-36. macOS arbitrary per-app volume: do not promise it yet

**Decision**: **PLATFORM-LIMITED / SPIKE REQUIRED** — do not claim generic Windows-style per-application volume control on macOS until a supported implementation is demonstrated.

**Rationale**: Explicit action scope is more important than fake parity.

**Alternatives**: Install a virtual audio driver. Technically possible but too large a product/security expansion to assume for MVP.

**Caveats**: Capability should remain data-driven in case future APIs improve support.

## R-37. macOS global media control: generic public control of other apps remains unresolved

**Decision**: **SPIKE REQUIRED** — do not rely on private MediaRemote frameworks; research a supported mechanism or fall back to synthetic media behavior as Unverified.

**Rationale**: Public MediaPlayer APIs are primarily documented for an application's own media session.

**Alternatives**: Private MediaRemote. Rejected for baseline product support.

**Caveats**: App-specific integrations may still be explicit actions; they are not a universal backend.

## R-38. macOS Focus status: permission-aware optional capability

**Decision**: **SUGGESTED** — integrate supported Focus-status APIs only after authorization and expose them as optional feedback-policy capability.

**Rationale**: Normal Kivori control should not depend on Focus authorization.

**Alternatives**: Scrape Control Center state. Rejected.

**Caveats**: Lack of permission is Unknown/Permission Required, not “Focus off.”

## R-39. Treat Linux X11 and Wayland as distinct capability environments

**Decision**: **SUGGESTED** — branch by actual display/session environment and dynamically advertise capabilities instead of claiming one blanket Linux feature set.

**Rationale**: X11 permits cross-client inspection/input in ways Wayland deliberately restricts.

**Alternatives**: Support only X11 initially. Valid release-policy choice, but the architecture should not prevent later Wayland support.

**Caveats**: Environment variables identify the session type but do not prove individual protocol/portal capability.

## R-40. Linux/X11 foreground and workspace context: EWMH

**Decision**: **SUGGESTED** — use EWMH `_NET_ACTIVE_WINDOW` and `_NET_CURRENT_DESKTOP` where supported.

**Rationale**: These standardized root-window properties support Kivori's focus/workspace semantics.

**Alternatives**: Desktop-environment-specific APIs only. Keep as enrichment, not baseline.

**Caveats**: Not every X11 window manager is fully EWMH-compliant.

## R-41. Linux/X11 shortcut injection: XTEST candidate

**Decision**: **SUGGESTED** — use XTEST when present and classify dispatch according to observable postconditions rather than assuming success.

**Rationale**: XTEST is the standard X11 mechanism for synthetic input.

**Alternatives**: `xdotool` subprocesses. Useful for spikes, weaker as a product dependency.

**Caveats**: Detect capability at runtime and do not import Windows privilege assumptions onto X11.

## R-42. Linux/Wayland foreground context: compositor/protocol-dependent

**Decision**: **PLATFORM-LIMITED** — treat foreign-toplevel discovery as `SupportedWhenBackendAvailable` rather than universal Wayland behavior.

**Rationale**: Wayland intentionally restricts global cross-client inspection and compositor support varies.

**Alternatives**: Infer foreground application from process activity. Rejected because it guesses context.

**Caveats**: GNOME, KDE, Sway, Hyprland, niri, and others must be validated independently.

## R-43. Linux/Wayland synthetic input: portal permission is a lease, not a permanent entitlement

**Decision**: **SPIKE REQUIRED** — evaluate XDG Desktop Portal RemoteDesktop/EIS for authorized keyboard input, using persistence/restore tokens where supported. Model permission as an explicit runtime capability lease, not an installation-time assumption.

**Rationale**: The portal can persist permission and return a restore token, but restoration may fail and the backend may require fresh user consent. Kivori cannot bypass compositor policy as an ordinary application.

**Alternatives**: `/dev/uinput` through a privileged helper/broker. Possible, but a major security/installation expansion and not the default until portal UX proves unacceptable.

**Caveats**: Kivori should attempt restore once at login/startup. If restore fails, mark shortcut capability `PermissionRequired` and prompt only when the user explicitly chooses to enable it; never trigger repeated OS consent dialogs on each physical gesture. R-84 defines the runtime policy.

## R-44. Linux audio: PipeWire first, PulseAudio compatibility as fallback candidate

**Decision**: **SUGGESTED** — research PipeWire as the modern Linux audio backend, retaining PulseAudio compatibility where supported environments require it.

**Rationale**: PipeWire exposes device/node/stream metadata and event-driven state useful for master/app controls.

**Alternatives**: Shell out to `pactl`/`wpctl`. Good for spikes, weaker as a product API.

**Caveats**: Stream-to-application mapping may be ambiguous, especially with browsers and sandboxed apps.

## R-45. Linux media: prefer MPRIS for observable control

**Decision**: **SUGGESTED** — use MPRIS over user D-Bus for players that implement it and use properties to determine capability/confirmation.

**Rationale**: MPRIS standardizes media control and observable playback state.

**Alternatives**: Media-key injection as an Unverified fallback.

**Caveats**: Multiple players require an explicit target/current-player policy.

## R-46. Linux session and sleep ownership: systemd-logind candidate

**Decision**: **SUGGESTED** — on systemd desktops, use logind D-Bus session/seat state and sleep signals.

**Rationale**: logind already represents active sessions and host sleep transitions.

**Alternatives**: Desktop-specific session APIs as supplements.

**Caveats**: Not all Linux distributions use logind; alternate session managers or reduced support must be represented honestly.

## R-47. Linux DND: desktop-specific; do not invent a universal freedesktop state

**Decision**: **PLATFORM-LIMITED** — treat Linux interruption/DND awareness as desktop-environment-specific until supported APIs are proven.

**Rationale**: The freedesktop notification specification does not define a universal DND query.

**Alternatives**: Infer DND from suppressed notifications. Rejected.

**Caveats**: Kivori's own DND preference remains independent of OS-level integration.

## R-48. Cross-platform serial discovery: keep `serialport`, normalize devices, and treat Linux access policy as installation work

**Decision**: **SUGGESTED** — retain `serialport` unless measurements show a problem; add normalization between OS enumeration and DeviceRegistry. On Linux, provision device access during installation rather than attempting runtime privilege bypass.

**Rationale**: Current code already uses blocking `serialport` successfully. Linux udev/logind `uaccess` can align device-node access with the active seat.

**Alternatives**: Permanent `dialout`/`plugdev` membership as compatibility fallback. World-writable nodes, routine root execution, and setuid Kivori are rejected. Switching to HID solely for permission reasons is not sufficient.

**Caveats**: A Kivori udev rule must match narrowly enough not to grant unrelated Espressif boards. Normal handshake identity is available only after open, so installer/device-identification metadata needs validation.

## R-49. Native-core-owned, schema-versioned, atomic configuration persistence

**Decision**: **SUGGESTED** — keep persistence in native Rust behind a `ConfigStore` interface. Storage must be schema-versioned, migratable, atomic, and per-user.

**Rationale**: This preserves webview least privilege and allows portable configurations that retain unsupported actions without corrupting them.

**Alternatives**: Bundled SQLite or versioned JSON/TOML are both viable candidates.

**Caveats**: Persistence technology should not leak into the behavioral contract.

## R-50. Keep platform-specific runtime loops thread-aware; GUI callbacks are ingress, not work queues

**Decision**: **SUGGESTED** — allow dedicated platform event loops/threads to feed bounded typed events into AppCore. On macOS, observer callbacks must be minimal and non-blocking.

**Rationale**: Several native APIs have message-loop or thread-affinity requirements. The existing dedicated serial thread proves this model fits Tauri.

**Alternatives**: Put every subsystem in arbitrary Tokio tasks or do AppCore work synchronously in native callbacks. Rejected as blanket approaches.

**Caveats**: Queues need overflow/coalescing policy. Event arrival order is not itself truth; R-83 defines revision-based context commitment.

## R-51. Refine offline-first: isolate network access to UpdateManager

**Decision**: **SUGGESTED** — core Kivori operation remains network-independent; firmware/shared crates/webview remain network-free; only approved native update code may access release/update endpoints.

**Rationale**: The product now intentionally includes update discovery while preserving offline operation.

**Alternatives**: Let React fetch release metadata directly. Rejected because it expands webview permissions and duplicates trust logic.

**Caveats**: CI guards should change from “no HTTP anywhere” to a strict network-boundary allowlist.

## R-52. Desktop self-update: Tauri updater is the leading implementation

**Decision**: **SUGGESTED** — use Tauri's signed updater for Desktop releases with explicit status and compatibility ordering managed by native `UpdateCoordinator`.

**Rationale**: Tauri's updater provides mandatory signature verification and cross-platform release support.

**Alternatives**: Custom replace/download logic. Rejected unless official updater limitations require it.

**Caveats**: Signing and installation semantics differ by OS; cross-session running instances need validation.

## R-53. Use one release manifest to coordinate Desktop/firmware/hardware compatibility

**Decision**: **SUGGESTED** — release metadata should express Desktop version, firmware version, hardware revisions, protocol/capabilities, minimum compatible counterpart versions, artifact authenticity metadata, severity, and dependency order.

**Rationale**: Desktop and firmware are independently versioned but cannot be updated blindly.

**Alternatives**: Encode compatibility only in GitHub Release naming. Possible early, brittle later.

**Caveats**: The manifest is never a runtime dependency for normal control.

## R-54. ROM flashing must be a re-enumeration state machine, not a fixed-port operation

**Decision**: **CANDIDATE** — treat Desktop-managed ROM flashing as a lower-complexity early update path but explicitly model `EnteringBootloader -> AwaitingRomDevice -> Flashing -> AwaitingApplication -> Verifying`.

**Rationale**: Native USB reset can make the serial node disappear and return with another COM/tty name. A fixed path is not stable update identity.

**Alternatives**: Application-level A/B update. More robust, much more firmware/partition complexity.

**Caveats**: Re-enumeration delay is expected progress, not immediate Disconnected failure. R-80 replaces topology-only target correlation with HardwareId verification.

## R-55. Firmware update option B: application-level A/B firmware with rollback

**Decision**: **CANDIDATE / SPIKE REQUIRED** — prefer A/B as a robustness target only if real flash/image/bootloader constraints make it practical.

**Rationale**: Inactive-slot writing and rollback provide substantially better power-loss and bad-image behavior than destructive replacement.

**Alternatives**: ROM flashing may remain the right first shipping mechanism.

**Caveats**: Kivori currently has no committed A/B partition layout and may need a custom OTA-capable bootloader.

## R-56. Firmware transfer protocol: explicit offsets/acks if application-level updating is chosen

**Decision**: **SUGGESTED if R-55 is selected** — use explicit image ID, size, digest, chunk offsets, acknowledgements, commit, and verification status.

**Rationale**: Firmware data needs reliable transfer without making ordinary input replayable.

**Alternatives**: Stream once with no resume and restart at byte zero after interruption. Simpler but weaker UX.

**Caveats**: Full-image authenticity/integrity remains separate from frame CRC.

## R-57. Firmware authenticity: staged security, not premature irreversible eFuse policy

**Decision**: **SUGGESTED** — authenticate release metadata/artifacts in Desktop and verify expected firmware digest before flashing; evaluate Secure Boot v2/anti-rollback separately for production manufacturing.

**Rationale**: Hardware secure boot changes recovery/manufacturing and can be irreversible.

**Alternatives**: Enable secure-boot eFuses immediately on prototypes. Rejected.

**Caveats**: A hash is only as trustworthy as the signed/trusted metadata that supplies it.

## R-58. ESP32-C3 ROM recovery: preserve BOOT/EN access below Kivori firmware

**Decision**: **SUGGESTED** — production hardware should expose a reliable physical path into the immutable ROM downloader plus MCU reset.

**Rationale**: Application-observed recovery cannot help if the application image itself is broken.

**Alternatives**: Depend only on the 10-second Kivori firmware hold. Rejected.

**Caveats**: Automatic download entry is useful but cannot be the sole recovery mechanism.

## R-59. Review GPIO2/GPIO8/GPIO9/EN as one boot/recovery hardware problem

**Decision**: **HARDWARE SPIKE REQUIRED** — validate display wiring and reset straps together before final PCB recovery design.

**Rationale**: Current display wiring uses GPIO2 and GPIO8 while ESP32-C3 boot behavior also depends on strap pins including GPIO8/GPIO9.

**Alternatives**: Assume runtime pin setup eliminates strap risk. Rejected because straps are sampled before application initialization.

**Caveats**: This must be proven electrically, not by Rust tests.

## R-60. Build/release matrix: add macOS before claiming macOS product support

**Decision**: **SUGGESTED** — CI/release should build Windows, Linux, macOS arm64, and macOS x86_64/universal strategy before public support claims.

**Rationale**: Current CI lacks macOS product coverage and native API code will rot without native builds.

**Alternatives**: Cross-compile macOS from Linux. Not a substitute for native framework/signing/runtime validation.

**Caveats**: CI success is not physical/platform behavioral evidence.

## R-61. macOS release: signing, notarization, and deployment target are architectural inputs

**Decision**: **SUGGESTED** — treat Apple signing/notarization and minimum OS version as technical design inputs.

**Rationale**: Native API availability can directly change the supported macOS baseline.

**Alternatives**: Treat signing as a final packaging task. Rejected for shipping architecture.

**Caveats**: App Store distribution has different sandbox/entitlement constraints than direct distribution.

## R-62. Linux support policy should name environments, not only “Linux”

**Decision**: **SUGGESTED** — release support should name tested distributions/runtime baselines plus X11/Wayland/backend expectations.

**Rationale**: A Linux package may launch while important OS integration is unavailable due to compositor, portal, audio, serial permission, or library differences.

**Alternatives**: “Best effort on any Linux desktop.” Too vague for reliable product support.

**Caveats**: Package build environment can unintentionally raise glibc/WebKitGTK compatibility baselines.

## R-63. Testing strategy: pure domain tests + backend contract tests + real platform evidence

**Decision**: **SUGGESTED** — use pure deterministic AppCore tests, backend contract tests, and real OS/hardware validation.

**Rationale**: Most User Story edge cases are deterministic, while permissions/session/compositor/hardware behavior cannot be honestly proven by mocks.

**Alternatives**: Rely mainly on end-to-end UI automation. Rejected as the primary approach.

**Caveats**: Simulation evidence must stay labelled as simulation.

## R-64. Add model/property tests for no-stale-replay, gesture ownership, and context revisions

**Decision**: **SUGGESTED** — generate event sequences containing disconnects, focus changes, target loss, reversals, invalid encoder transitions, queue delay/reordering, and reconnects.

**Rationale**: These bugs are primarily ordering/state-machine failures.

**Alternatives**: Only handwritten example tests. Keep them, but supplement with generated sequences.

**Caveats**: Property tests prove the model, not the delivery guarantees of native APIs or physical contacts.

## R-65. Keep permission/restriction truth separate from transport health

**Decision**: **SUGGESTED** — platform adapters publish permission/restriction state independently from DeviceRegistry connection state.

**Rationale**: Losing Accessibility, portal permission, or serial-device authorization must not look like USB disconnect.

**Alternatives**: Return generic action failures. Rejected for UX and diagnosis.

**Caveats**: Permission may disappear mid-gesture; restriction/cancellation rules must win immediately.

## R-66. Platform-specific confirmation should be discoverable by configuration UI

**Decision**: **SUGGESTED** — expose capability/confirmation metadata to React so users know whether actions are Confirmed, Unverified, Permission Required, or Unsupported before testing them.

**Rationale**: Honest configuration UX prevents discovering platform impossibility only after pressing hardware.

**Alternatives**: Allow every abstract action everywhere and fail at runtime. Rejected.

**Caveats**: Portable config should retain actions that are unsupported only on the current machine.

## R-67. `Test Action` should invoke the real ActionEngine with test origin

**Decision**: **SUGGESTED** — Desktop Test Action should use the same adapters, permissions, confirmation, and execution path as physical input, tagged as software-originated deliberate intent.

**Rationale**: A separate fake test path could claim success while real bindings fail.

**Alternatives**: UI-only Success animation. Appropriate only for explicit visual preview, not action execution.

**Caveats**: Test Action bypasses physical gesture ownership but not security/permission/assignment validity.

## R-68. Keep preview/testing presentation separate from production state ownership

**Decision**: **SUGGESTED** — model Preview Buddy State as an explicit preview lease/overlay with expiry and restoration to current truth.

**Rationale**: Preview must not mutate persistent execution/host state.

**Alternatives**: Send an ordinary state mutation and hope a later reset repairs it. Rejected for richer multi-axis state.

**Caveats**: Dev-only Device Studio may retain stronger manual override tools if clearly separated from user preview behavior.

## R-69. Current heartbeat wiring needs verification before richer Degraded semantics

**Decision**: **SPIKE REQUIRED** — audit the production heartbeat scheduling path before building product-level Degraded state on it.

**Rationale**: `Session` exposes ping/timeout APIs, but the inspected production loop does not visibly schedule them.

**Alternatives**: Treat only serial I/O errors as liveness. Too weak for a companion that can sit silently on a stalled link.

**Caveats**: Verify current code/tests before modification because this may change independently.

## R-70. Cross-platform capability matrix is a living evidence artifact

**Decision**: **SUGGESTED** — maintain this matrix as research status, not marketing promise.

| Product capability | Windows | macOS | Linux X11 | Linux Wayland |
|---|---|---|---|---|
| USB Kivori session | `serialport`; exclusive handle | normalize tty/cu | udev/uaccess may be required | same serial layer/policy |
| Foreground app profile | WinEvent | NSWorkspace | EWMH | compositor-dependent |
| Window-specific profile | Win32 metadata | Accessibility permission | EWMH/X11 | platform-limited |
| Workspace transition | public membership + focus | Spaces + revision reconcile | EWMH | compositor-dependent |
| Generic shortcut | SendInput, usually Unverified | permission-dependent synthetic input | XTEST | portal permission/session |
| Master audio | Core Audio endpoint | Core Audio device-dependent | PipeWire/Pulse candidate | PipeWire/Pulse candidate |
| App audio | strong candidate | spike/limited | PipeWire candidate | PipeWire candidate |
| Mic activity | spike | spike | PipeWire candidate | PipeWire candidate |
| Media | GSMTC spike | unresolved public generic control | MPRIS | MPRIS |
| DND/interruption | partial suitability | permission-aware Focus candidate | DE-specific | DE-specific |
| User-session privacy | host lease + WTS | host lease + NSWorkspace | host lease + logind | same |
| Hard active-user USB takeover | broker if arbitrary old-owner suspension must be tolerated | broker candidate for hard guarantee | broker if required by product guarantee | same |
| Sleep/display state | Windows power settings | NSWorkspace/power APIs | logind/DE | logind/DE |
| Overlay classification | rich but heuristic | permission-dependent detail | possible under X11 | restricted |

**Rationale**: The matrix makes product differences visible without pretending feature parity.

**Alternatives**: Maintain only a list of supported OS names. Rejected.

**Caveats**: “Supported” here means research confidence; release support requires CI and real validation.

## R-71. Current code -> research direction map

**Decision**: **SUGGESTED** — migrate incrementally rather than perform an architecture-v2 rewrite.

| Current repository element | Research direction |
|---|---|
| `device/fsm.rs` | Keep transport/link-only |
| `device/session.rs` | Keep; extend capability-gated protocol |
| `device/serial.rs` | Keep blocking serial unless measurements justify change |
| `runtime/device_task.rs` | Evolve toward DeviceRegistry/DeviceActor or broker client |
| `first_candidate()` | Replace with normalized multi-device discovery |
| `orchestrator::Orchestrator { desired }` | Supersede with AppCore + resolver; do not grow into god object |
| `ConnectionState` | Keep transport-only |
| `CompanionState` / `SendableState` | Preserve compatibility; add richer semantic presentation separately |
| `Capabilities::NONE` | Allocate real feature bits |
| canonical renderer/assets | Preserve |
| webview least privilege | Preserve |
| offline network guard | Refine to allow native update boundary only |
| hard-coded DeviceId | Replace with DeviceId + HardwareId model |
| firmware ports | Extend testable-port pattern where useful |

**Rationale**: This lowers implementation risk and keeps Feature 001 value.

**Alternatives**: Broad rewrite. Rejected for now.

**Caveats**: Old simple desired-state resync needs explicit compatibility tests when richer presentation lands.

## R-72. User Story -> primary technical responsibility map

**Decision**: **SUGGESTED** — use this ownership map to keep future work out of arbitrary modules.

| User Story area | Primary technical responsibility |
|---|---|
| US1 | BindingResolver + ActionEngine + firmware validated input |
| US2 | ActionAttempt/ExecutionTracker + platform observability |
| US3 | Firmware GestureStateMachine + ContextEngine cancellation |
| US4 | Platform observers + state axes + PresentationResolver |
| US5 | ContextEngine + SessionOwnershipGate/broker boundary + ConfigStore |
| US6 | Capability/permission model + PresentationResolver |
| US7 | PresentationResolver + canonical renderer |
| US8 | typed capability/state model |
| US9 | platform observers + display policy + firmware display control |
| US10 | DeviceRegistry/Session or broker + ExecutionTracker + recovery/update |
| US11 | ConfigStore + assignment + UpdateCoordinator + React UX |

**Rationale**: It ties research to the normative User Story document without creating a second behavior contract.

**Alternatives**: One subsystem per User Story. Rejected because stories intentionally cross shared components.

**Caveats**: This is ownership guidance, not a prohibition on module collaboration.

## R-73. Linux serial permission is provisioning policy, not runtime privilege bypass

**Decision**: **SUGGESTED / SPIKE REQUIRED** — on logind desktops, prefer a narrowly matched Kivori udev rule using `uaccess`; provide distro-specific fallback where unavailable. Never solve ordinary serial access by running Kivori Desktop as root.

**Rationale**: Device-node access is kernel policy. Installer/package setup is the correct privileged boundary and `uaccess` aligns access with the active seat.

**Alternatives**: `dialout`/`plugdev` membership as compatibility fallback. World-writable nodes and setuid/root Kivori are rejected.

**Caveats**: Matching only Espressif VID/PID may be too broad. Validate stable interface/path/serial metadata and uninstall cleanup.

## R-74. HardwareBroker becomes mandatory only when hard cross-session availability is a product requirement

**Decision**: **CANDIDATE / CONDITIONAL REQUIREMENT** — if the product promises that the newly active user can always use Kivori even when an old user's process is arbitrarily suspended while holding USB, physical ownership must move into a process whose lifecycle is independent of user-session suspension.

**Rationale**: Exclusive serial handles cannot be taken over by another ordinary process until the original owner closes or exits.

**Alternatives**: Keep per-user direct ownership and promise privacy plus graceful/best-effort reassignment only. This remains viable and avoids broker complexity.

**Caveats**: Do not confuse privacy with availability. Firmware host-lease expiry can clear private state while the port remains unavailable to the new user.

## R-75. UpdateCoordinator needs deterministic identity across ROM-mode re-enumeration

**Decision**: **SUGGESTED for ROM flashing** — create an `UpdateLease` containing product `DeviceId`, immutable `HardwareId`, physical USB topology/location evidence, expected hardware revision, and intended firmware artifact.

**Rationale**: Application `DeviceId` disappears in ROM mode, while port names may change. The ESP ROM tooling can read factory chip MAC/eFuse identity; this gives a stronger correlation signal than topology alone.

**Alternatives**: Topology-only matching or “first Espressif ROM device wins.” Both are rejected in a multi-device product.

**Caveats**: Topology remains useful to narrow candidates, but `HardwareId` should verify the ROM target before flashing. If unique identity cannot be read or several candidates remain ambiguous, abort and request that other recovery devices be disconnected.

## R-76. macOS native callbacks must never own AppCore work

**Decision**: **SUGGESTED** — AppKit/Tauri callbacks copy stable identifiers/event type/timestamp/revision, perform a non-blocking bounded handoff, and return. AppCore work belongs outside the callback.

**Rationale**: Blocking the GUI event path can starve window processing and cause context notifications themselves to lag.

**Alternatives**: Resolve profile/action state directly on the main thread. Rejected.

**Caveats**: Overflow should coalesce to latest context, not block AppKit. R-83 defines stale/out-of-order protection.

## R-77. Dropped encoder motion is information loss; acceleration can degrade but cannot infer it

**Decision**: **SUGGESTED** — only completed validated detents affect value and direction. Missing physical motion that produced no validated event is not reconstructable.

**Rationale**: If five physical CW detents yield three validated CW detents, Kivori should act on three and may underestimate speed. Inventing missing ticks would violate observable input truth.

**Alternatives**: Velocity extrapolation, guessed detents, or suppressing one reverse detent at high speed. Rejected.

**Caveats**: High decoder-quality degradation may justify temporarily disabling acceleration, but this is conservative degradation rather than reconstruction.

## R-78. Input conditioning should be selected from measured signal quality, not peripheral preference

**Decision**: **SPIKE REQUIRED** — treat PCNT, GPIO state-machine decoding, high-rate sampling, and RC/Schmitt conditioning as combinable candidates. Select the simplest solution that meets measured false/lost-detent requirements.

**Rationale**: PCNT is useful for counting but its microsecond filter cannot be assumed to clean millisecond mechanical bounce; software full-step qualification can reject many bounce paths.

**Alternatives**: Freeze PCNT or freeze software decoding before measurement. Both are premature.

**Caveats**: Test multiple encoder samples, age/noise if practical, slow reversal, extreme spin, simultaneous press, USB/display load, and recovery hold.

## R-79. Separate cross-user privacy guarantees from cross-user availability guarantees

**Decision**: **SUGGESTED** — define a firmware-side host-session lease/heartbeat expiry so the device can fail closed when its owning Desktop stops making progress. Privacy-sensitive presentation and ordinary control authority must expire independently of whether another process can acquire the serial handle.

**Rationale**: If User A's process freezes while owning the exclusive port, User B may not be able to open it. However, firmware can still notice that the old host lease stopped renewing and transition to a neutral `Waiting`/host-unavailable presentation, suppressing normal mappings and invalidating prior-user visible state.

**Alternatives**: Treat “port remains open” as proof the host is still valid. Rejected because process suspension/stall can preserve the kernel handle while application logic is dead. A broker remains the solution for guaranteed new-user availability, not for the privacy fail-closed mechanism itself.

**Caveats**: This requires real heartbeat/lease wiring; R-69 must be resolved. A firmware lease protects device behavior, not host-side files/processes. The product contract should be precise about whether it promises only privacy or also immediate new-user availability.

## R-80. ROM update identity should be `DeviceId + HardwareId + topology`, not topology alone

**Decision**: **SUGGESTED** — normal Kivori handshake should expose both a product `DeviceId` and immutable `HardwareId`. Before flashing, `UpdateCoordinator` records both plus topology. In ROM mode it narrows by topology, reads the ROM-visible chip identity, verifies `HardwareId`, flashes only on an exact match, then verifies both identities again after application reboot.

**Rationale**: Identical unmanaged hubs can produce visually similar port layouts and application `DeviceId` is unavailable in the ROM downloader. ESP32-C3 factory MAC/eFuse identity is per-chip and readable through Espressif tooling, so it is a better ROM-mode identity anchor than topology.

**Alternatives**: Topology-only matching, serial-node name, VID/PID, or first-responsive ROM loader. Rejected because each can select the wrong unit in a multi-device environment.

**Caveats**: Confirm the exact identity read path in the chosen flashing library/tool and under future security settings. If the identity cannot be read, require an unambiguous one-device recovery environment instead of guessing.

## R-81. ExecutionTracker can outlive device ownership, but not suspension of its own process

**Decision**: **SUGGESTED** — keep long-running job identity and observation independent from Kivori USB ownership. When an OS seat changes, release/lose the device without cancelling already-confirmed host work. If the observer process is suspended, treat the observation gap explicitly and reconcile on resume.

**Rationale**: Linux `uaccess`/seat changes concern the serial node; they do not inherently stop a build/script/process already running in the user's session. `ExecutionTracker` should therefore remain a host concern. If Kivori Desktop itself is frozen, no software can truthfully claim it continued observing during that gap.

**Alternatives**: Cancel all jobs when serial access is lost. Rejected because transport ownership is unrelated to host job lifetime. Pretend the job remained continuously observed during Desktop suspension is also rejected.

**Caveats**: Where possible, retain durable job identity such as PID plus process-start identity, child handle, explicit runner ID, or another re-observable token. If current status cannot be recovered after resume, classify it as Unverified/Execution Unknown rather than replaying historical status.

## R-82. A legal Gray transition is not a validated detent

**Decision**: **SUGGESTED** — the firmware encoder decoder should accumulate/validate a complete mechanically meaningful detent before emitting `ValidatedDetent(CW/CCW)`. Individual legal quarter-step Gray transitions and partial back-and-forth phase motion are input-conditioning detail and must not reach acceleration semantics as detents.

**Rationale**: Mechanical bounce can legitimately traverse adjacent Gray states and return without the knob completing a detent. Treating each legal edge as a detent would make a noisy encoder produce repeated false reversals and destroy tactile acceleration. Full-step or detent-qualified decoding rejects many such partial cycles naturally.

**Alternatives**: Emit one logical tick per legal electrical transition. Rejected for the intended tactile semantics unless the chosen encoder is explicitly configured as quarter-step hardware and UX is designed around that representation.

**Caveats**: A sufficiently degraded contact can still produce a complete false-but-valid reverse detent sequence. At that point software has no truthful basis to call it fake. Hardware conditioning/decoder quality should make completed false detents rare; persistent quality degradation may disable acceleration rather than ignore genuine-looking reverse detents.

## R-83. ContextEngine should commit revisions, not queue arrival order

**Decision**: **SUGGESTED** — every observed foreground/workspace candidate should carry a monotonically increasing `ContextRevision` plus observation time/source. Passive stabilization remembers the candidate revision and commits only if it is still current after the stabilization window. Physical Kivori interaction triggers immediate foreground revalidation and commits the latest valid revision for that gesture.

**Rationale**: Bounded async queues can delay or coalesce AppKit events. Queue order is therefore not a reliable source of present truth. Revision checks prevent an older Finder/Space notification from committing after Safari is already current, while interaction-time revalidation implements the existing rule that deliberate input commits the current valid app immediately.

**Alternatives**: Trust FIFO dequeue order or synchronously resolve every callback on the main thread. Rejected; FIFO can still contain stale observations and synchronous work risks GUI starvation.

**Caveats**: Revision assignment should occur at the native observation boundary or otherwise be monotonic for the backend. Revalidation must run security/protected-context classification before dispatch, and a known target loss still cancels the gesture instead of retargeting.

## R-84. Wayland shortcut authorization is a revocable runtime lease

**Decision**: **SUGGESTED / PLATFORM-LIMITED** — treat XDG RemoteDesktop portal permission as a runtime capability lease. Request persistent authorization where supported, store/rotate the returned restore token, attempt silent restore once after login/startup, and if restoration fails transition the shortcut capability to `PermissionRequired` until the user explicitly chooses to re-enable it.

**Rationale**: Portal persistence does not guarantee that every compositor will silently restore permission after reboot. Kivori cannot override compositor consent policy. Deterministic shortcut execution is only promised while the capability lease is active.

**Alternatives**: Re-prompt automatically whenever the user rotates/presses hardware. Rejected because it creates repeated surprise OS dialogs. A privileged `/dev/uinput` helper/broker can provide a different permission model but significantly expands the trust/install surface.

**Caveats**: Permission loss should be localized: master volume or other unaffected actions remain available while only shortcut-dependent actions show Permission Required. The configuration UI should show the current lease state before Test Action or binding execution.

---

## Implementation-spike-required

The following should be answered with small evidence-producing spikes before the corresponding technical recommendation is frozen:

1. **Windows GSMTC in shipped Tauri packaging** — prove capability declaration/access and event/control behavior in the production packaging model.
2. **Windows microphone activity** — verify capture-session/process coverage across conferencing apps, browsers, and virtual devices.
3. **macOS global media control** — find a supported public mechanism or formally accept synthetic/unverified fallback.
4. **macOS arbitrary app-volume control** — prove a supported implementation or mark the action unavailable.
5. **macOS modern Core Audio minimum version** — determine deployment-target impact.
6. **Wayland foreground context** — test GNOME/Mutter, KDE/KWin, Sway/wlroots, Hyprland, niri, and intended environments.
7. **Wayland portal persistence** — validate `persist_mode`, restore-token rotation, reboot behavior, and failure UX per compositor/backend.
8. **Linux PipeWire mapping** — prove master/app volume, microphone activity, virtual devices, and sandbox metadata.
9. **Fast User Switching normal path** — measure release/reacquisition and state invalidation on Windows/macOS/Linux.
10. **Frozen-handle Fast User Switching** — deliberately suspend the old user process while it owns Kivori and decide whether hard availability requires a broker.
11. **Firmware host lease** — verify privacy state clears even when an old host process is frozen but the kernel serial handle remains open.
12. **Tauri single-instance semantics across simultaneous local users** — verify instances do not incorrectly block other user sessions.
13. **Stable DeviceId/HardwareId provisioning** — choose prototype/production paths and test persistence across reflashes.
14. **ROM-mode HardwareId correlation** — prove the selected flasher can read immutable chip identity in ROM mode on Windows/macOS/Linux.
15. **Firmware update architecture** — compare ROM flashing versus A/B with actual image/flash/update-time data.
16. **USB topology as routing evidence** — validate physical-location identifiers across hubs/docks/re-enumeration without treating them as sole identity.
17. **`espflash` integration/MSRV** — decide library pin, toolchain raise, sidecar, or minimal flasher primitives.
18. **Heartbeat production path** — reconcile code/tests/docs before richer Degraded/host-lease behavior.
19. **Config persistence** — compare versioned JSON versus bundled SQLite using actual schema complexity.
20. **macOS serial normalization** — validate `/dev/cu.*` preference/deduplication with physical ESP32-C3.
21. **macOS callback ingress and revision model** — prove delayed/coalesced events cannot commit stale context during rapid switching plus physical interaction.
22. **Linux Kivori udev policy** — prove narrowly matched `uaccess` rules and fallback behavior.
23. **ExecutionTracker resume reconciliation** — suspend Desktop while a long-running script/build continues and verify re-observation or Unverified fallback.
24. **Cross-session Desktop self-update** — verify behavior when multiple users have Kivori processes running.
25. **Broker feasibility** — if hard takeover is required, prototype the smallest per-OS broker boundary before moving any user semantics into a service.

## Hardware-validation-required (do NOT trust research alone)

1. **HW-040 quadrature/bounce characterization** — measure transition order, bounce duration, detents per logical step, and fastest realistic manual rotation.
2. **Full-detent decoder behavior** — verify partial legal Gray transitions/bounce do not emit logical detents.
3. **Completed false reverse rate** — age/noise multiple encoders and quantify false full-detent reversals at high velocity.
4. **PCNT-only validation/rejection** — measure whether board-level signals are clean enough for PCNT's limited filter; do not assume they are.
5. **Decoder comparison** — compare PCNT + conditioning, GPIO state-machine, sampling, and hybrid approaches for lost/false detents and CPU load.
6. **Encoder quality telemetry** — determine useful invalid/abandoned-phase thresholds and whether conservative 1× degradation improves UX.
7. **Recovery hold under input noise** — prove rotary/auxiliary noise cannot cancel/reset recovery.
8. **Actual flash capacity/layout** — record physical flash size, partition/bootloader footprint, firmware/assets, and margin.
9. **A/B feasibility** — if pursued, prove two image slots + metadata fit and rollback works after a deliberately bad image.
10. **Power-loss firmware update** — remove power at multiple phases and prove documented recovery.
11. **ROM recovery** — deliberately install unusable application firmware and recover through BOOT/EN.
12. **GPIO2/GPIO8/GPIO9/EN straps** — validate reset electrical levels with display/backlight attached.
13. **USB Serial/JTAG re-enumeration** — measure disappearance/reappearance timing and topology through bootloader/reset/hubs/multiple devices.
14. **ROM HardwareId read** — prove immutable identity remains readable in the recovery configuration intended for production.
15. **Multi-device USB** — verify unique DeviceId + HardwareId, assignment, Passive presentation, and port renumbering.
16. **Sustained USB/render latency** — retain existing physical validation for stalls/reconnect and display performance.
17. **Buzzer/display power** — verify wake latency and no-ambiguous-silence states on physical hardware.

## Platform-validation-required

1. **Windows intended baseline** — foreground hooks, WTS lock/switch, power notifications, audio changes, UIPI, fullscreen/game overlays.
2. **Windows graceful Fast User Switching** — verify outgoing process releases hardware under normal session notifications.
3. **Windows frozen-owner case** — suspend old Kivori process while serial handle is open; verify firmware host-lease privacy behavior and new-user availability limitation.
4. **Windows broker spike if required** — prove hard takeover only through a lifecycle independent of user-session suspension.
5. **Windows elevated/protected contexts** — UAC/secure desktop and elevated targets without running Kivori as admin.
6. **macOS Intel + Apple Silicon** — focus, Spaces, Accessibility grant/revoke, session switching, sleep/wake, audio, signing/notarization.
7. **macOS callback revision correctness** — rapid VS Code/Finder/Safari switching plus immediate Kivori interaction must commit only the latest valid context.
8. **macOS multiple audio devices** — built-in, USB, Bluetooth, HDMI, aggregate/virtual where applicable.
9. **Linux X11** — EWMH, PipeWire/Pulse, XTEST, session switching, serial permissions.
10. **Linux udev/uaccess install** — clean-machine install/uninstall, active-seat handoff, no-root normal runtime, safe device matching.
11. **Linux seat switch with running host jobs** — release device while a job continues; verify ExecutionTracker behavior separately.
12. **GNOME Wayland** — foreground reality, portal remote input, permission persistence/restore, PipeWire, logind.
13. **KDE Wayland** — same categories independently.
14. **wlroots-family Wayland** — foreign-toplevel and portal/backend differences.
15. **Wayland reboot reauthorization** — if restore fails, verify one explicit permission prompt path and no repeated prompts from hardware gestures.
16. **Linux packaging** — clean-machine package formats including libudev/WebKitGTK/serial policy/PipeWire/portal dependencies.
17. **Offline operation on every supported OS** — block network completely and confirm core control/config/device behavior remains functional.

## Sources

Primary/vendor/standards references used for this research; exact API/version must be verified again when implementation begins.

### Kivori repository

- [`features/001-device-connection-foundation/architecture.md`](../features/001-device-connection-foundation/architecture.md)
- [`features/001-device-connection-foundation/offline-boundary.md`](../features/001-device-connection-foundation/offline-boundary.md)
- [`features/001-device-connection-foundation/research.md`](../features/001-device-connection-foundation/research.md)
- `apps/desktop/src-tauri/src/runtime/device_task.rs`, `device/session.rs`, `device/serial.rs`, `orchestrator/mod.rs`
- `crates/kivori-protocol`, `crates/kivori-model`
- `firmware/esp32-c3/src/runtime.rs`, `transport.rs`, `physical_st7789.rs`, `profile.rs`

### Tauri

- Tauri v2 Security: <https://v2.tauri.app/security/>
- Capabilities/permissions: <https://v2.tauri.app/security/capabilities/>
- Updater: <https://v2.tauri.app/plugin/updater/>
- Autostart: <https://v2.tauri.app/plugin/autostart/>
- Async runtime: <https://docs.rs/tauri/latest/tauri/async_runtime/>
- `run_on_main_thread`: <https://docs.rs/tauri/latest/tauri/struct.App.html>
- Distribution/signing: <https://v2.tauri.app/distribute/>
- Tauri GitHub Action: <https://github.com/tauri-apps/tauri-action>

### Microsoft / Windows

- `SetWinEventHook`: <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook>
- Fast User Switching: <https://learn.microsoft.com/en-us/windows/win32/shell/fastuserswitching>
- `WTSRegisterSessionNotification`: <https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsregistersessionnotification>
- Communications Resource Handles: <https://learn.microsoft.com/en-us/windows/win32/devio/communications-resource-handles>
- `IVirtualDesktopManager::IsWindowOnCurrentVirtualDesktop`: <https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ivirtualdesktopmanager-iswindowoncurrentvirtualdesktop>
- `OpenInputDesktop`: <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openinputdesktop>
- Power setting GUIDs: <https://learn.microsoft.com/en-us/windows/win32/power/power-setting-guids>
- Core Audio Endpoint Volume: <https://learn.microsoft.com/en-us/windows/win32/coreaudio/endpointvolume-api>
- `IAudioSessionManager2`: <https://learn.microsoft.com/en-us/windows/win32/api/audiopolicy/nn-audiopolicy-iaudiosessionmanager2>
- `IAudioSessionControl2::GetProcessId`: <https://learn.microsoft.com/en-us/windows/win32/api/audiopolicy/nf-audiopolicy-iaudiosessioncontrol2-getprocessid>
- `SendInput`: <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput>
- GSMTC: <https://learn.microsoft.com/en-us/uwp/api/windows.media.control/globalsystemmediatransportcontrolssessionmanager>
- `SHQueryUserNotificationState`: <https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shqueryusernotificationstate>
- Extended window styles: <https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles>
- DWM window attributes: <https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute>
- `windows-rs`: <https://github.com/microsoft/windows-rs>

### Apple / macOS

- `NSWorkspace.didActivateApplicationNotification`: <https://developer.apple.com/documentation/appkit/nsworkspace/didactivateapplicationnotification>
- `NSWorkspace.frontmostApplication`: <https://developer.apple.com/documentation/appkit/nsworkspace/frontmostapplication>
- `NSWorkspace.activeSpaceDidChangeNotification`: <https://developer.apple.com/documentation/appkit/nsworkspace/activespacedidchangenotification>
- `NSWorkspace.sessionDidResignActiveNotification`: <https://developer.apple.com/documentation/appkit/nsworkspace/sessiondidresignactivenotification>
- `NSWorkspace.sessionDidBecomeActiveNotification`: <https://developer.apple.com/documentation/appkit/nsworkspace/sessiondidbecomeactivenotification>
- Accessibility trust: <https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions>
- Core Audio `AudioHardwareSystem`: <https://developer.apple.com/documentation/coreaudio/audiohardwaresystem>
- Core Audio `AudioHardwareProcess`: <https://developer.apple.com/documentation/coreaudio/audiohardwareprocess>
- Core Audio taps: <https://developer.apple.com/documentation/coreaudio/capturing-system-audio-with-core-audio-taps>
- `INFocusStatusCenter`: <https://developer.apple.com/documentation/intents/infocusstatuscenter>
- `MPRemoteCommandCenter`: <https://developer.apple.com/documentation/mediaplayer/mpremotecommandcenter>

### Linux / freedesktop / Wayland

- `open(2)` open-file-description semantics: <https://man7.org/linux/man-pages/man2/open.2.html>
- systemd/logind `sd-login` / `uaccess`: <https://man7.org/linux/man-pages/man3/sd-login.3.html>
- systemd `70-uaccess.rules`: <https://cgit.freedesktop.org/systemd/systemd/tree/src/login/70-uaccess.rules>
- EWMH: <https://specifications.freedesktop.org/wm/latest-single/>
- XTEST: <https://www.x.org/releases/X11R7.5/doc/man/man3/XTestFakeKeyEvent.3.html>
- Wayland foreign-toplevel list: <https://wayland.app/protocols/ext-foreign-toplevel-list-v1>
- XDG RemoteDesktop portal: <https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html>
- PipeWire Rust bindings: <https://pipewire.pages.freedesktop.org/pipewire-rs/pipewire/index.html>
- PipeWire/SPA properties: <https://docs.pipewire.org/devel/group__spa__param.html>
- MPRIS Player: <https://specifications.freedesktop.org/mpris/latest/Player_Interface.html>
- logind D-Bus: <https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html>
- Desktop Notifications specification: <https://specifications.freedesktop.org/notification/latest-single/>

### Espressif / embedded Rust

- `esp-hal` PCNT: <https://docs.rs/esp-hal/latest/esp_hal/pcnt/>
- `esp-hal` eFuse: <https://docs.rs/esp-hal/latest/esp_hal/efuse/>
- ESP32-C3 boot mode: <https://docs.espressif.com/projects/esptool/en/latest/esp32c3/advanced-topics/boot-mode-selection.html>
- esptool basic commands / MAC read: <https://docs.espressif.com/projects/esptool/en/latest/esp32/esptool/basic-commands.html>
- esptool reset/re-enumeration: <https://docs.espressif.com/projects/esptool/en/latest/esp32c3/esptool/advanced-options.html>
- ESP32-C3 USB Serial/JTAG: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/api-guides/usb-serial-jtag-console.html>
- ESP-IDF OTA/rollback: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/ota.html>
- `esp-bootloader-esp-idf` OTA: <https://docs.espressif.com/projects/rust/esp-bootloader-esp-idf/latest/esp32c3/esp_bootloader_esp_idf/ota/index.html>
- ESP32-C3 Secure Boot v2: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32c3/security/secure-boot-v2.html>
- `espflash`: <https://docs.rs/espflash/latest/espflash/>

### Cross-platform serial / persistence candidates

- `serialport`: <https://docs.rs/serialport/latest/serialport/>
- `rusqlite`: <https://docs.rs/rusqlite/latest/rusqlite/>
