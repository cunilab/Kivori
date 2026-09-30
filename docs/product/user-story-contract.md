# Kivori User Story Contract

**Status:** Product behavior baseline  
**Date:** 2026-09-16  
**Product requirements:** [`../PRD.md`](../PRD.md)

## 1. Purpose

This document defines the normative behavioral contract for Kivori's product user stories.

It describes **what users must be able to rely on**, not the internal implementation architecture. Implementation plans, feature specs, protocol revisions, and platform-specific designs should reference this contract rather than re-deciding these behaviors independently.

Implementation status is separate from this contract. A behavior being specified here does not imply it is already implemented.

## 2. Normative Language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

- **MUST / MUST NOT**: required product behavior.
- **SHOULD / SHOULD NOT**: expected default behavior unless a documented platform limitation requires otherwise.
- **MAY**: optional behavior that must not violate any MUST-level rule.

Timing values marked **initial target** are product UX targets and may be tuned after hardware/platform validation without changing the underlying behavioral rule.

## 3. Global Invariants

The following rules apply across all user stories.

1. **Desktop truth wins.** Confirmed observable state overrides Kivori's local prediction, previous command, or requested value.
2. **Observable truth only.** Kivori MUST NOT invent state it cannot observe reliably.
3. **Acknowledgement is not confirmation.** Immediate input feedback MUST NOT be presented as proof of desktop success.
4. **Confirm only what is knowable.** Known success, known failure, and unknown outcome MUST remain distinct.
5. **No stale replay.** Expired input MUST NOT execute after a connection or service recovers.
6. **Explicit action scope.** An action's meaning MUST NOT silently change because a different app profile is active.
7. **Stable context.** Profile changes MUST follow stable OS focus, not mouse hover or momentary focus theft.
8. **Interaction commits intent.** If a pending foreground application is valid but still inside the focus-stabilization window, deliberate Kivori interaction MUST commit that foreground application before dispatching the action.
9. **Security context beats profile fallback.** Protected/secure foreground context MUST be classified before profile commitment and MUST NOT fall back to General merely because normal app actions are unavailable.
10. **Dispatch target is revalidated.** A gesture MUST NOT be silently retargeted if its committed application disappears between input registration and action dispatch.
11. **Known target loss cancels the gesture.** If a committed application target is known to disappear during a continuous gesture, the remaining gesture input MUST be discarded until that gesture ends.
12. **Gesture ownership.** A physical gesture remains bound to the context in which it began.
13. **One gesture owns input in MVP.** Simultaneous inputs MUST NOT implicitly become combo triggers.
14. **Wake gestures are atomic.** A gesture that begins while the display is fully asleep MUST be consumed as wake-only in its entirety.
15. **Rotary reversal resets acceleration.** A direction reversal MUST restart rotary acceleration at baseline before acceleration may build again in the new direction.
16. **Acceleration represents sustained direction.** Acceleration MUST accumulate only across consecutive same-direction detents; rapid oscillation repeatedly resets to baseline.
17. **User isolation.** One OS user's mappings and session-specific presentation MUST NOT remain visible or actionable in another user's session.
18. **Session-private render state becomes non-renderable on ownership change.** Previous-user app/profile names, custom icons, transient feedback, and user-specific indicators MUST be invalidated before another local OS user can see them.
19. **No hidden fallback.** If an action cannot run in the current context, Kivori MUST communicate unavailability rather than silently switch mechanisms.
20. **Localized uncertainty.** Unknown/restricted state SHOULD propagate only as far as necessary.
21. **Known host/session state wins.** Explicit sleep, lock, switching-user, update, or restart state outranks generic communication symptoms.
22. **Passive is not Waiting.** Passive means a usable desktop exists but this device is not assigned as the controller.
23. **Passive is not Monitor Mode.** Passive devices MUST NOT mirror ordinary real-time desktop state in MVP.
24. **Hardware recovery is out-of-band.** Basic MCU recovery MUST remain available independently of desktop software, profile assignment, display-sleep state, and normal action restrictions whenever firmware can still observe the recovery gesture.
25. **Latest deliberate interaction wins transient feedback.** A newer deliberate user action MAY replace an older interaction transient immediately; user-action transients MUST NOT form a mandatory visual queue.
26. **Interaction feedback has priority over ordinary background transients.** Ordinary unsolicited reactions SHOULD NOT interrupt active user-action feedback.
27. **Urgent state may wake the display.** A small defined class of immediate-attention desktop states MAY wake Display Sleep; ordinary background changes MUST NOT.
28. **System/privacy indicators outrank custom indicators.** User customization MUST NOT allow app-specific indicators to displace higher-priority safety/privacy state.
29. **Indicator escalation is immediate; promotion is stabilized.** A newly active higher-priority indicator SHOULD appear immediately, while newly exposed lower-priority indicators SHOULD wait for a short stabilization window before causing layout reflow.
30. **Continuous gestures reconcile at gesture boundaries.** External continuous-value updates SHOULD NOT visually fight an active Kivori rotary gesture; final confirmed state MUST reconcile when the gesture ends.
31. **Execution health is separate from transport health.** A confirmed host-side long-running operation MUST NOT be considered failed solely because the Kivori device link becomes Degraded or Disconnected.
32. **Reconnection restores current truth, not expired presentation.** Reconnecting MUST NOT replay stale Success/Error/Unverified transients for historical events that completed while the device was disconnected.
33. **Recovery hold has absolute input arbitration.** While the primary recovery-capable button remains continuously held and its recovery timer is active, incidental rotary detents MUST NOT cancel, reset, or replace the recovery hold and MUST NOT execute ordinary rotary actions.
34. **Rotary acceleration is bounded.** MVP acceleration MUST NOT exceed the default 5x base-step ceiling; actions MAY use a lower ceiling or disable acceleration.
35. **Explicit desktop test intent may wake Display Sleep.** A user-triggered Test Action or Preview Buddy State command from Kivori Desktop is explicit intent and MAY wake the display without requiring a prior physical wake gesture.
36. **Takeover states preempt active gestures.** When a Layer 1 takeover state makes normal interaction unavailable, Kivori MUST cancel the active normal gesture and surface the takeover without waiting for the gesture-end timer.
37. **Recognized transient overlays do not own profiles by default.** A transient overlay shown over a stable foreground application SHOULD preserve the underlying application's profile unless the overlay becomes a normal independently focused context or is classified as Protected.
38. **No ambiguous silence.** Whenever Kivori is powered and capable of rendering, user-relevant waiting, processing, restricted, transitional, failure, recovery, and update conditions MUST have an intentional visual representation rather than appearing frozen or accidentally blank.
39. **Truthful activity beats fake progress.** When exact progress is not observable, Kivori SHOULD show indeterminate activity and state meaning rather than inventing a percentage or completion estimate.
40. **Short press is release-qualified.** A normal short-press action becomes permanently ineligible once the initial MVP 500 ms hold cutoff is exceeded, preventing aborted recovery attempts from falling through into mapped click actions.
41. **Composite actions report the composite outcome.** A multi-step macro MUST reflect the outcome of all required steps rather than reporting success because an earlier step succeeded.
42. **Only observed hardware input is actionable.** Kivori MUST NOT reconstruct or invent rotary detents that the device did not actually observe.
43. **Workspace transitions are non-profile-owning.** OS virtual-desktop/workspace switch surfaces SHOULD NOT own Kivori profiles; the resulting foreground app follows the normal stabilization/interaction-commit rules.
44. **Recovery-capable Hold actions are release-qualified.** A mapped Hold action on the primary recovery-capable button MUST NOT execute merely because the Hold threshold is crossed; recovery ownership can supersede it before release.
45. **Composite confirmation cannot exceed its least-confirmed required step.** If any required macro step is Unverified, the overall composite MUST NOT be presented as confirmed success unless that step later becomes confirmed.
46. **Input conditioning precedes gesture semantics.** Raw encoder electrical transitions MAY be filtered/decoded before they become logical detents; only validated logical detents participate in direction-reversal and acceleration rules.
47. **Software test wake preserves idle history.** Waking from Display Sleep for Test Action or Preview Buddy State MUST NOT by itself restart the full ambient-idle progression.
48. **Overlay classification is conservative and non-invasive.** Kivori SHOULD use OS-observable context and explicit user rules; ambiguous overlay identity MUST fall back to normal focus stabilization rather than hidden/invasive inspection or guessing.
49. **Composite actions are non-transactional in MVP.** A later macro-step failure MUST NOT silently roll back already completed earlier side effects; current observable desktop state remains authoritative unless compensation was explicitly authored.
50. **Transient presentation restores current underlying truth.** When a transient ends, Kivori MUST return to the actual current underlying state, including Running/Busy when long-running work remains active, rather than defaulting to Idle.
51. **Desktop Test Action is deliberate user intent.** A user-triggered Test Action from Kivori Desktop belongs to the deliberate-interaction feedback class, not ordinary unsolicited/background feedback.
52. **Single-gesture ownership is device-wide in MVP.** While one ordinary physical gesture owns input, other ordinary rotary or button controls, including auxiliary buttons, MUST NOT dispatch a second mapped action unless a future explicit multi-input gesture type defines that combination.
53. **Discrete button mappings do not implicitly key-repeat.** A release-qualified discrete mapping produces one action per valid press/release; repeating while held requires an explicit repeating/continuous action type.
54. **A normal rotary binding owns both directions.** A bidirectional `Rotate` binding reserves clockwise and counter-clockwise detents as one logical control; unrelated scopes MUST NOT silently split the two directions.
55. **Update compatibility is checked before installation.** Kivori Desktop MUST evaluate known desktop/firmware/hardware compatibility constraints and required installation order before changing either component.
56. **Update artifacts are authenticated.** Desktop and firmware update artifacts MUST pass the product's integrity/authenticity checks before installation.
57. **Firmware recovery must exist below the application image.** Production hardware MUST preserve an application-independent ROM/bootloader recovery path so corrupt Kivori firmware does not permanently remove the ability to restore the device.
58. **Update availability is not installation consent.** A normal or recommended available update MUST NOT silently trigger a disruptive firmware flash merely because a newer version exists.

---

# US1 — Physical Desktop Control

## User Story

**As a user, I want to control frequent desktop functions through Kivori, so I can perform common actions without interrupting my current work.**

## Contract

Kivori MUST support an action model where each assigned action has explicit semantics and scope.

Examples include:

- System Volume;
- Play/Pause;
- Microphone Mute;
- keyboard shortcut;
- launch application;
- run script;
- application-specific action;
- configured macro.

`System Volume` and `Discord Volume`, for example, are distinct actions. Activating a Discord profile MUST NOT silently redefine a generic `System Volume` action as Discord process volume.

### Discrete button repeat semantics

Normal discrete button mappings are release-qualified and fire once per valid discrete press/release.

Examples include:

- `Next Track`;
- `Previous Track`;
- one-shot `Volume Up` / `Volume Down` step actions;
- ordinary shortcuts.

Holding a discrete mapping MUST NOT implicitly start OS-style key-repeat merely because the physical button remains down.

A user who wants repeated/continuous behavior must use a distinct explicit action/input type such as **Repeat While Held** if/when that type is supported.

An explicit repeating action type MUST define its own start, repeat-rate, stop, feedback, context, and recovery-arbitration semantics; it MUST NOT be inferred from a normal discrete mapping.

On the primary recovery-capable button, implicit repeat is especially prohibited because an early repeating side effect could occur before recovery ownership is known.

### Rotary action contract

Each rotary-capable action MAY define:

- minimum and maximum range;
- step size;
- sensitivity;
- acceleration behavior;
- acceleration reset interval.

Initial default acceleration-reset target: **250 ms without a new detent -> reset to 1x**.

Acceleration MUST be action-specific and MAY be disabled for precision-oriented actions.

Acceleration MUST build only from consecutive detents in the same direction. Every direction reversal MUST reset the multiplier to baseline before the first detent in the new direction is applied.

Example:

`CW 1x -> CW 2x -> CW 3x -> reverse -> CCW 1x -> CCW 2x -> reverse -> CW 1x`

Rapid one-detent oscillation therefore remains effectively at baseline sensitivity. This is intentional: sustained direction expresses speed intent, while repeated reversal expresses precision intent.

For actions such as frame-accurate or fine audio scrubbing, acceleration MAY be disabled entirely.

### Global rotary binding scope

A normal `Rotate` action is one bidirectional logical control.

If a Global binding assigns:

`Rotate -> Master Volume`

then both directions belong to that Global logical rotary binding:

- clockwise adjusts Master Volume in its positive direction;
- counter-clockwise adjusts Master Volume in its negative direction.

An application profile MUST NOT silently replace only one direction with an unrelated action while leaving the opposite direction owned by the Global binding.

For MVP, an application-specific override MAY replace the complete logical `Rotate` binding according to the configured precedence rules.

A future **Directional Rotary** configuration MAY expose clockwise/counter-clockwise as independent bindings, but only as an explicit mode with visible conflict and precedence behavior.

### Acceleration ceiling

Initial MVP maximum acceleration multiplier: **5x the action's base step**.

Acceleration MAY ramp through implementation-defined intermediate levels, but the effective multiplier MUST NOT exceed 5x in MVP.

An action MAY configure:

- a lower multiplier ceiling;
- no acceleration at all.

Example:

`1x -> 2x -> 3x -> 4x -> 5x -> 5x -> 5x`

The 5x ceiling limits runaway jumps during extreme high-speed continuous rotation while preserving useful coarse adjustment.

### Observed detents only

Kivori MUST apply only rotary detents that the hardware/firmware actually observes.

If extremely fast manual rotation causes some physical detents not to be observed because of encoder bounce characteristics, electrical filtering, polling limits, or other hardware input limits:

- missing/unobserved detents MUST NOT be reconstructed or guessed;
- their absence MUST NOT be treated as an action failure by itself;
- Kivori MUST NOT show Error merely because the user's physical motion may have exceeded what the encoder could report;
- a hardware error MAY be shown only when the device has positive evidence of an actual hardware/input fault.

The principle is: **missing input is not failed input.**

### Validated logical detents

Raw electrical edges are not themselves product-level rotary actions.

Firmware/input processing MAY debounce, decode quadrature transitions, reject electrically invalid transition sequences, and otherwise convert raw encoder activity into validated logical detents before gesture semantics are evaluated.

Direction-reversal and acceleration behavior MUST operate on those validated logical detents:

- electrical bounce/noise rejected before logical-detent formation MUST NOT reset acceleration;
- a validated logical reverse detent MUST reset acceleration to 1x immediately;
- the gesture layer MUST NOT intentionally ignore a valid reverse detent merely to preserve the existing acceleration multiplier.

This keeps electrical noise handling below the semantic rule while preserving immediate real reversals.

### Bounded values

When a bounded action reaches its minimum or maximum:

- the value MUST clamp immediately;
- the first continued tick beyond the limit MAY produce one subtle boundary reaction;
- repeated ticks farther into the same boundary SHOULD NOT spam visual or buzzer feedback;
- the first reverse tick MUST take effect immediately;
- reversing direction MUST NOT require the user to pause or end the gesture;
- the first reverse tick MUST use baseline acceleration, not the accumulated multiplier from the previous direction.

Example:

`98 -> 99 -> 100 -> extra clockwise ticks ignored -> first counter-clockwise tick -> 99`

## Acceptance Criteria

- [ ] Assigned actions expose explicit action identity/scope.
- [ ] App profiles select actions without mutating action meaning.
- [ ] A discrete button mapping emits one action per valid release and does not implicitly repeat while held.
- [ ] Continuous/repeating hold behavior requires an explicit action/input type.
- [ ] A Global bidirectional Rotate binding owns both clockwise and counter-clockwise directions.
- [ ] Application profiles do not silently steal one direction from a normal Global Rotate binding.
- [ ] Any future directional rotary split is explicit rather than inferred.
- [ ] Rotary actions can define action-specific sensitivity and acceleration.
- [ ] Acceleration accumulates only across same-direction detents.
- [ ] Every validated direction flip resets acceleration to 1x.
- [ ] Rapid oscillation remains effectively baseline unless multiple same-direction detents accumulate.
- [ ] Precision-oriented actions can disable acceleration.
- [ ] MVP acceleration never exceeds 5x the action's base step.
- [ ] Actions can choose a lower acceleration ceiling.
- [ ] Unobserved/dropped detents are not invented or reconstructed.
- [ ] High-speed detent loss does not produce Error without positive hardware-fault evidence.
- [ ] Raw electrical bounce can be filtered before direction semantics.
- [ ] A genuine validated reverse detent is not suppressed merely to preserve acceleration.
- [ ] Bounded controls clamp correctly.
- [ ] Boundary feedback is not repeated for every excess tick.
- [ ] Reverse movement exits boundary suppression immediately.
- [ ] Direction reversal resets accumulated acceleration to 1x before the first reverse step.

---

# US2 — Responsive Input and Honest Confirmation

## User Story

**As a user, I want Kivori to respond immediately to physical input while clearly distinguishing acknowledgement from actual action success.**

## Contract

The conceptual interaction pipeline is:

**Input -> Acknowledged -> Executed -> Confirmed when possible**

Kivori MUST distinguish at least three confirmation classes.

### State Confirmed

The resulting state is directly observable.

Example: microphone mute is requested and the desktop subsequently reports the microphone as muted.

### Execution Confirmed

The operation is known to have started or completed, but no persistent resulting state exists.

Example: a configured process launch is known to have started successfully.

### Triggered / Unverified

The action was dispatched but its final effect cannot be verified.

Example: a keyboard macro is emitted but there is no reliable downstream completion signal.

Kivori MUST NOT present Triggered / Unverified as confirmed success.

### Composite/sequential actions

A sequential macro or other multi-step action is one user-level composite action.

If a composite action contains required steps:

- all required confirmation-bearing steps MUST reach the confirmation level needed by the action before the overall action may be presented as Execution Confirmed/Success;
- positive failure of any required step MUST make the overall action **Error / Partial Failure**, even if earlier steps succeeded;
- if any required step is **Triggered / Unverified**, the overall composite MUST be **Triggered / Unverified** unless that step later becomes confirmed;
- a mixture of Execution Confirmed and Unverified required steps therefore remains **Triggered / Unverified** overall;
- a composite made entirely of inherently unverifiable required triggers remains **Triggered / Unverified** when no known failure occurs;
- orchestration-only steps such as a delay do not increase confirmation confidence merely because the delay elapsed;
- optional/non-required steps MAY be reported separately without changing the overall required-step outcome when their failure is explicitly non-fatal by configuration.

Examples:

`Launch App [Execution Confirmed] -> Hotkey [Unverified]` -> overall **Triggered / Unverified**.

`Hotkey 1 [Unverified] -> Delay -> Hotkey 2 [Unverified]` -> overall **Triggered / Unverified**.

`Launch App [Confirmed] -> required readiness/send step [known failure]` -> overall **Error / Partial Failure**.

The overall composite confirmation MUST NOT be stronger than the least-confirmed required step.

### Composite rollback semantics

Composite actions are **not transactional in MVP**.

If a required later step fails after an earlier step already changed observable desktop state successfully:

- the earlier successful side effect MUST remain part of current desktop truth;
- Kivori MUST NOT silently issue an inverse action in an attempt to restore the previous state;
- Kivori SHOULD reconcile and display the actual resulting desktop state alongside the Error / Partial Failure outcome;
- automatic rollback/compensation MUST NOT be inferred from the macro definition merely because the overall macro failed.

Example:

`Mute System Audio [State Confirmed] -> Launch App [known failure]` -> overall **Error / Partial Failure**, while System Audio remains muted and the mute indicator remains truthful.

A compensating step such as `On Failure -> Unmute System Audio` MAY be supported in the future or represented as explicitly authored macro behavior, but it is not an implicit MVP rollback mechanism.

### Initial interactive timing targets

- **< 50 ms:** local input acknowledgement;
- **~500 ms:** unresolved interactive action MAY show Processing / Delayed;
- **~1,500 ms:** unresolved ordinary interactive action reaches timeout handling.

Timeout alone MUST NOT automatically become Error.

- positive evidence of failure -> **Error**;
- dispatched action with unknowable outcome -> **Unverified**;
- unhealthy communication -> **Degraded system health**.

Long-running operations MUST NOT be forced into the ordinary 1,500 ms interactive timeout model; they require an explicit running/pending behavior.

### Long-running host execution

Once a long-running action has been positively confirmed as started by the desktop host, its execution lifetime is distinct from the Kivori device-link lifetime.

Examples include:

- a multi-minute build;
- a long-running script;
- an export/render process.

If the Kivori USB/device link becomes Degraded or Disconnected while the desktop service still observes the job:

- the host-side job MUST NOT be marked failed solely because device transport is unhealthy;
- the job MAY continue running normally on the host;
- device communication health and host execution state MUST remain separate concepts;
- on reconnection, Kivori MUST reconcile to the current observed job state rather than replay the start command.

If the desktop service itself loses the ability to determine whether the job completed, the execution result MUST become **Unverified / Execution Unknown** unless positive evidence of failure exists.

A lost or uncertain execution MUST NOT be automatically restarted merely because the Kivori link later recovers.

### Reconnection and historical completion

Reconnection is a current-state reconciliation event, not a replay of missed presentation events.

If a previously known long-running action completed while Kivori was disconnected:

- Kivori MUST NOT replay an expired Success, Error, or Unverified transient merely because transport reconnects;
- if the job is still running, current Running state MAY be restored;
- if the job completed in the past and no current running state remains, Kivori SHOULD settle directly into the current underlying buddy state;
- historical completion MAY remain available in desktop-side history/logging without occupying the live transient surface.

Example:

`Build starts -> Kivori disconnects -> build succeeds -> five minutes later reconnect -> current state Idle`

The reconnect itself MUST NOT fabricate an 800 ms Success transient for that five-minute-old completion.

### Visual activity during unresolved work

Whenever Kivori is capable of rendering and an action is meaningfully unresolved, the presentation MUST make the state intentional rather than appearing frozen.

- ordinary delayed work MAY show Processing / Delayed after the relevant timing threshold;
- long-running work SHOULD show a persistent Running/Busy presentation while current execution is known;
- if trustworthy progress is available, Kivori MAY show progress;
- if trustworthy progress is unavailable, Kivori SHOULD show indeterminate activity rather than a fabricated percentage;
- known failure, Unverified, and Context Lost states SHOULD produce a distinct visual treatment instead of silently disappearing.

### Transient reaction targets

Initial display targets:

- Success: ~800 ms;
- Triggered / Unverified: ~1.2 s;
- Error / Failed: ~2 s.

A transient reaction MUST return to the actual underlying state rather than blindly returning to Idle.

If a host-side long-running job is still active when an unrelated interaction transient expires, the primary buddy MUST return to the current **Running / Busy** representation (or the truthful equivalent for that job) rather than Idle.

Example:

`Build Running -> Volume 45% transient -> transient expires -> Build Running`

Transient presentation MUST NOT erase, complete, or otherwise mutate the underlying persistent execution state.

### Deliberate interaction collision

A new valid deliberate interaction MAY immediately replace a transient reaction from an earlier deliberate interaction.

Example:

`Action 1 Success overlay begins -> 200 ms later Action 2 occurs -> Action 1 overlay ends -> Action 2 feedback appears immediately`

User-action transients MUST NOT be required to queue sequentially. The newest deliberate interaction owns the interaction-feedback surface.

Takeover states MUST NOT be dismissed merely because another physical input occurred.

### Background transient collision priority

When an ordinary unsolicited background transient occurs while user-action feedback is active:

- the active interaction feedback SHOULD finish first;
- the ordinary background transient MAY be queued and shown afterward if still relevant;
- ordinary background feedback MUST NOT silently overwrite active user-action feedback.

A defined urgent state MAY preempt user-action feedback when immediate attention is more important than preserving the transient.

Default transient priority is:

**Urgent state -> newest active interaction feedback -> ordinary unsolicited transient**

## Acceptance Criteria

- [ ] Local acknowledgement is distinguishable from action success.
- [ ] State Confirmed, Execution Confirmed, and Unverified outcomes remain distinct.
- [ ] Composite actions do not report Success solely because an early required step succeeded.
- [ ] Known required-step failure becomes Error / Partial Failure.
- [ ] Any required Unverified step caps the overall composite at Triggered / Unverified.
- [ ] A composite of only unverifiable required triggers remains Triggered / Unverified when no known failure occurs.
- [ ] Mixed confirmed + unverified required steps remain Triggered / Unverified overall.
- [ ] A later macro failure does not silently roll back successful earlier state changes.
- [ ] Partial macro failure reconciles to the actual remaining desktop state.
- [ ] Automatic compensation occurs only when explicitly authored/supported, not as hidden MVP rollback.
- [ ] Timeout logic distinguishes known failure from unknown outcome.
- [ ] A target app crash during a pending action becomes Error when the crash is known.
- [ ] Missing callback without known failure becomes Unverified.
- [ ] Confirmed long-running host execution is not failed solely because the device link degrades.
- [ ] Reconnection reconciles long-running job state without replaying the start command.
- [ ] Historical job completion does not generate a stale transient on reconnection.
- [ ] A still-running job may restore current Running state after reconnection.
- [ ] Unknown host-job outcome becomes Unverified rather than automatic failure or retry.
- [ ] Unresolved/long-running work has an intentional active presentation when rendering is available.
- [ ] Unknown progress is represented indeterminately rather than with invented percentages.
- [ ] Transient reactions restore the actual underlying current state.
- [ ] A Running/Busy job remains Running/Busy after an unrelated transient expires.
- [ ] A newer deliberate interaction may truncate and replace an older interaction transient immediately.
- [ ] Deliberate user-action transients do not form a mandatory visual queue.
- [ ] Ordinary background errors do not interrupt active user-action feedback.
- [ ] Urgent states can preempt transients when defined as immediate-attention conditions.

---

# US3 — Smooth, Atomic Physical Interaction

## User Story

**As a user, I want physical interaction with Kivori to remain smooth and deterministic during rapid input, simultaneous input, context changes, or temporary communication problems.**

## Contract

### Rapid rotary input

Rapid rotary movement MUST behave as a continuous interaction rather than requiring a full request/confirmation round trip for every detent.

Kivori MAY:

- render an immediate local target preview;
- combine/coalesce rapid updates;
- reconcile the final preview against confirmed desktop state.

Example:

`local preview 90% -> desktop confirms 88% -> display settles at 88%`

Confirmed state MUST win.

### Rotary gesture boundary

Initial MVP rotary gesture-end target: **250 ms without a new rotary detent**.

Until that inactivity threshold is reached, consecutive detents MAY be treated as one continuous rotary gesture.

This gesture boundary is distinct from action semantics even if it initially shares the same 250 ms target as acceleration reset.

### Button press/hold classification

Normal short-press actions are **release-qualified** for MVP.

Initial short-press hold cutoff: **500 ms**.

A normal discrete press MUST NOT begin implicit key-repeat while waiting for release classification. Repeat-while-held behavior is a separate explicit action type, not a property of an ordinary short-press mapping.

For the primary recovery-capable button, a mapped **Hold** action is also release-qualified rather than firing immediately at threshold.

Initial recovery-ownership threshold: **~2 seconds of continuous hold**. The final MCU recovery threshold remains **~10 seconds total from the original key-down**.

The primary recovery-capable button therefore behaves as follows while awake and normal interaction is available:

- release at or before 500 ms -> mapped short-press action MAY execute;
- after 500 ms and before ~2 s -> short press is suppressed; a mapped Hold action MAY be armed and the display SHOULD make the pending release behavior understandable;
- release after 500 ms but before ~2 s -> the mapped Hold action MAY execute if one is configured and current context permits it;
- reaching ~2 s without release -> any mapped Hold action for that gesture MUST be cancelled before execution and recovery ownership begins;
- from ~2 s until the ~10 s recovery threshold -> ordinary short/Hold actions remain suppressed and recovery visual feedback owns the gesture;
- release after recovery ownership begins but before ~10 s -> no mapped desktop action executes;
- reaching ~10 s total hold -> MCU recovery executes according to US10.

Example:

`button down -> 500 ms -> Hold armed, not executed -> 2 s -> recovery owns gesture -> release at 9 s -> no desktop action`

This prevents a recovery attempt from causing an irreversible mapped Hold side effect several seconds before the reboot occurs.

The short-press cutoff and recovery-ownership threshold are MVP input-classification rules and SHOULD NOT be user-configurable by profile in MVP.

If the gesture began in Display Sleep, US9's wake-only rule suppresses ordinary short/Hold execution for the entire physical gesture; recovery may still take ownership if the user continues holding.

### Gesture context ownership

A gesture MUST remain attached to the profile/context that was committed when the gesture began.

If application focus changes during a rotary burst, the current rotary burst MUST continue using its starting context. The newly focused profile becomes eligible for the next gesture.

### Continuous gesture target loss

If the committed target application/window is known to close, crash, or otherwise become invalid while a continuous gesture is active:

- any detents already dispatched remain historical and MUST NOT be replayed or undone automatically;
- the current gesture MUST be marked cancelled for application-scoped execution;
- all later detents belonging to that same gesture MUST be ignored for desktop execution;
- later detents MUST NOT independently retarget to General, the previous app, or the app that gains focus next;
- Kivori SHOULD surface **Error / Context Lost** once for the cancelled gesture rather than producing an error per ignored detent;
- a new application context may become eligible only after the cancelled gesture ends and a new gesture begins.

Example:

`VS Code gesture -> detents 1-3 execute -> VS Code exits -> Context Lost -> detents 4-10 ignored -> gesture ends -> next new gesture may bind to new context`

This preserves the invariant that one physical gesture has one context.

### Mid-gesture desktop state changes

For a continuous value that is actively controlled by a Kivori rotary gesture, an external desktop update to that same continuous value SHOULD NOT immediately overwrite the local in-progress preview.

The active gesture temporarily owns the continuous preview until the gesture ends, after which Kivori MUST reconcile to the latest confirmed desktop value.

Important discrete state changes MAY still appear immediately without destroying the continuous preview. For example, an independently observed mute/unmute transition may appear as a status indicator while a volume gesture remains in progress.

This exception MUST NOT allow Kivori to preserve a stale preview after the gesture completes.

### One gesture at a time

For MVP, gesture ownership is **device-wide across ordinary physical controls**.

The first active ordinary gesture owns physical interaction until it ends.

Examples:

- rotation starts -> primary or auxiliary button input does not become a second action;
- primary button hold starts -> rotary movement does not become another implicit action;
- primary button hold starts -> pressing a secondary/auxiliary button does not dispatch its mapped action;
- an auxiliary-button gesture starts -> another ordinary button or rotary control does not become a parallel mapped action.

Suppressed secondary input MAY receive a subtle local busy/blocked reaction when useful, but Kivori MUST NOT present normal action acknowledgement or success feedback that could imply the suppressed action was dispatched.

Future combinations such as `Hold + Rotate`, `Hold Primary + Press Auxiliary`, or other simultaneous controls MAY be introduced only as explicit input types with explicit configuration.

The hardware recovery hold is an explicit exception to ordinary gesture arbitration. Once recovery ownership begins on the primary button:

- rotary detents MUST NOT cancel or reset the recovery timer;
- rotary detents MUST NOT execute ordinary mapped rotary actions;
- auxiliary/secondary button presses MUST NOT execute ordinary mapped actions;
- other ordinary controls MUST NOT replace recovery ownership with another gesture;
- ordinary mapped short/Hold actions for that same button gesture MUST remain cancelled;
- releasing the primary button before the final recovery threshold ends the recovery attempt normally without executing a mapped action.

This rule makes recovery robust on push-encoders and future multi-button hardware where incidental or simultaneous control input can occur while the user is holding the recovery-capable control.

### Interrupted communication

Kivori MUST NOT queue stale user intent for later replay.

During **Reconnecting**:

- local acknowledgement MAY still occur;
- desktop action MUST NOT be deferred for later execution;
- old intent MUST be discarded.

If Reconnecting or Disconnected becomes known while an ordinary continuous gesture is active:

- the active normal gesture MUST be cancelled immediately;
- remaining detents in that physical gesture MUST be discarded for desktop execution;
- the Layer 1 takeover presentation MUST appear without waiting for the 250 ms gesture-end threshold;
- a new actionable gesture MUST NOT begin until the takeover condition has cleared and normal interaction is available again.

During **Degraded** communication:

- an action MAY be attempted while communication is still usable;
- unresolved/expired actions MUST NOT execute later after recovery;
- Degraded alone does not require gesture cancellation if the current interaction remains valid and communication is still usable.

For a currently active continuous gesture, the system MAY retain the latest target while the gesture and connection are still valid, but MUST NOT replay the historical tick sequence later.

## Acceptance Criteria

- [ ] Rapid rotary use does not require one full confirmation round trip per tick.
- [ ] Final display reconciles against confirmed state.
- [ ] 250 ms without a detent ends a rotary gesture for the initial MVP target.
- [ ] A short press is eligible only when released within the initial 500 ms cutoff.
- [ ] Ordinary short-press mappings do not emit implicit repeat events while held.
- [ ] A mapped Hold on the primary recovery-capable button is armed after the Hold threshold but does not execute merely on threshold crossing.
- [ ] Releasing after 500 ms but before recovery ownership may execute the configured Hold action.
- [ ] Crossing the initial ~2 s recovery-ownership threshold cancels the mapped Hold for that gesture.
- [ ] Releasing after recovery ownership begins but before ~10 s executes no mapped desktop action.
- [ ] A Display Sleep-originated press never executes ordinary short/Hold actions from that same physical gesture.
- [ ] Device-wide single-gesture ownership suppresses ordinary secondary/auxiliary controls while another physical gesture is active.
- [ ] Suppressed secondary input does not produce misleading dispatched/success feedback.
- [ ] Multi-control combinations require an explicit future input type rather than implicit concurrent execution.
- [ ] Known target loss mid-gesture cancels the remaining stream.
- [ ] Remaining detents after target loss are ignored until that gesture ends.
- [ ] Target loss does not cause per-detent retargeting to another application.
- [ ] Context Lost feedback is not spammed once per ignored detent.
- [ ] External updates do not visually fight the same continuous value during an active gesture.
- [ ] Important discrete state changes may still surface during a continuous gesture.
- [ ] Implicit chord actions are not produced in MVP.
- [ ] Rotary and auxiliary inputs during recovery ownership do not cancel/reset recovery and do not execute ordinary actions.
- [ ] Reconnecting/Disconnected cancels an active normal gesture immediately.
- [ ] Layer 1 connection takeover does not wait for the rotary gesture-end timer.
- [ ] Inputs during Reconnecting are not replayed after recovery.
- [ ] Expired inputs during Degraded communication do not execute later.

---

# US4 — Real and Observable Desktop State

## User Story

**As a user, I want Kivori to represent desktop state it can actually observe, so I can trust what the buddy shows.**

## Contract

Kivori SHOULD update from observable state changes regardless of origin, including:

- Kivori itself;
- keyboard;
- mouse;
- OS controls;
- application UI;
- supported external hardware.

Kivori MUST NOT assume its last requested state remains true when the desktop reports otherwise.

If a physical device changes behavior without exposing a trustworthy state signal, Kivori MUST represent the observable state rather than infer hidden physical reality.

Example: if a headset's hardware mute switch electrically blocks a microphone but the OS continues reporting the microphone as active, Kivori MUST NOT invent a muted state unless a trustworthy hardware/device signal becomes available.

The gesture-local preview rule in US3 is temporary presentation ownership only. It MUST NOT weaken the requirement that the final represented state converges to observable desktop truth.

## Acceptance Criteria

- [ ] External desktop changes can update Kivori state without requiring a Kivori-originated command.
- [ ] Confirmed desktop state overrides local previews and requested values after gesture reconciliation.
- [ ] Unobservable hardware-only states are not fabricated.
- [ ] Stale state is not presented as current after the source becomes invalid.

---

# US5 — Application and User-Aware Controls

## User Story

**As a user, I want Kivori's controls to adapt to the active application and OS user while ignoring momentary or ambiguous context changes.**

## Contract

### Focus definition

The active application profile MUST follow OS keyboard/window focus.

Mouse hover alone MUST NOT change profiles.

Initial passive focus-stabilization target: **300-500 ms** before committing to a new profile.

Momentary overlays and brief focus theft SHOULD NOT produce visible profile thrashing.

Transient system overlays MAY be classified as non-profile-owning contexts.

### Global/application binding precedence

Binding precedence operates on the configured **logical input scope**, not raw electrical direction fragments.

For MVP:

- a Global bidirectional `Rotate` binding reserves the complete logical rotary gesture when no profile override replaces it;
- an application profile MAY explicitly replace that complete `Rotate` binding according to normal profile precedence;
- an application profile MUST NOT silently override only clockwise or only counter-clockwise while the other direction remains owned by the unrelated Global binding;
- binding-conflict UI SHOULD explain when an application override replaces a Global logical control.

A future directional rotary mode MAY expose each direction independently, but that mode must be explicit and cannot be inferred from conflicting Global/profile assignments.

### Virtual desktop/workspace transitions

OS virtual-desktop/workspace switch surfaces such as Windows Task View or macOS Spaces transition UI SHOULD be non-profile-owning.

During the transition:

- the transition surface SHOULD NOT become a normal application profile;
- the previously committed app profile MAY remain presentation context until the switch resolves, but new app-scoped execution MUST follow the current valid foreground context rules;
- after the switch completes, the newly visible/focused app enters the normal **300-500 ms** passive stabilization window;
- if the user deliberately interacts with Kivori during that window, the pending newly focused app MUST commit immediately under the normal interaction-commit rule;
- General MUST NOT be used merely as an intermediate profile because a workspace animation is occurring.

### Transient in-app and game overlays

Recognized transient overlays displayed over a stable foreground application SHOULD be non-profile-owning by default.

Examples include temporary in-game overlays such as communication, platform, capture, or GPU-control overlays that appear above a borderless/fullscreen game while the game remains the underlying stable activity.

Overlay classification MUST be conservative and non-invasive.

Kivori Desktop MAY consider multiple ordinary OS-observable signals together, such as:

- current foreground/focus ownership;
- process/application identity;
- window owner/root-owner relationships;
- activation/visibility/tool-window/topmost-style metadata where the platform exposes it;
- whether the surface is transient or persistent across focus changes;
- recent foreground-transition history;
- explicit user-configured overlay classification.

No single generic window style/flag SHOULD be treated as proof that a surface is an overlay.

When an overlay is confidently classified as transient/non-profile-owning:

- the underlying stable application's profile SHOULD remain active;
- opening or closing the overlay SHOULD NOT restart the 300-500 ms profile-stabilization cycle merely because the overlay surface became visible;
- Kivori MUST NOT require process injection, anti-cheat hooks, memory inspection, or hidden game instrumentation solely to preserve profile ownership;
- classification SHOULD rely on ordinary OS-observable context and/or explicit user configuration;
- if the overlay becomes a normal independently focused persistent application/window, normal focus-stabilization rules MAY apply;
- if the overlay or game context is Protected/restricted, Protected behavior MUST override the underlying game profile.

If overlay classification is ambiguous, Kivori MUST NOT guess that it is transient merely to preserve the game profile. The ambiguous window/context follows the normal focus-stabilization rules unless the user has explicitly configured a supported overlay rule.

An explicit user rule such as **Treat this application/window match as transient overlay** MAY be supported. Such a rule MUST NOT bypass Protected/restricted context classification.

This preserves game-control continuity where the evidence is strong without requiring invasive hooks or turning heuristic window metadata into false certainty.

### Interaction commits pending focus

Focus stabilization protects against passive focus flicker; it MUST NOT cause deliberate Kivori input to execute against a stale foreground application.

If Application B currently owns valid normal OS foreground focus but remains inside the passive stabilization window, a deliberate Kivori interaction MUST:

1. classify whether the pending foreground context is normal or protected/secure;
2. for a normal context, commit Application B as the interaction context immediately;
3. select Application B's profile if one exists, otherwise the appropriate fallback profile;
4. bind the complete gesture to that committed context.

The action MUST NOT execute under previously committed Application A solely because the stabilization timer has not expired.

General MUST NOT be used merely as an intermediate race-condition profile when a valid pending foreground app is known.

### Protected pending foreground context

If the newly focused context is UAC, a secure desktop, a protected administrator surface, or another context classified as Protected, deliberate Kivori input during the stabilization window MUST commit the **Protected** behavior immediately.

Protected context MUST NOT fall back to General merely because no ordinary application profile may execute there.

Security/restriction classification takes precedence over ordinary app-profile selection.

### Target revalidation before dispatch

After an interaction context is committed but before an application-scoped action is dispatched, the target context SHOULD be revalidated when practical.

If the committed target application/window terminates before dispatch and that loss is known:

- the gesture MUST NOT be silently retargeted to General;
- the gesture MUST NOT be silently retargeted to the previous app;
- the gesture MUST NOT be silently transferred to whatever app gains focus next;
- the action SHOULD end as **Error / Context Lost** or an equivalent known-failure result.

If target validity cannot be determined reliably, the action follows the normal Unverified rules rather than guessing.

If a target becomes invalid after a continuous gesture has already begun, US3's gesture-level cancellation rule applies: the remaining gesture stream is discarded until gesture end rather than revalidating and retargeting every later detent independently.

### Foreground/background rule

The committed foreground application owns app-specific physical mappings.

Background applications MAY contribute observable status but MUST NOT steal physical mapping ownership merely because they remain active in the background.

### Neutral desktop context

When no normal application profile applies, neutral desktop/taskbar contexts SHOULD use the General profile.

Protected or secure contexts MUST NOT be treated as ordinary General context.

### Application identity

Kivori SHOULD prefer stable application identity over volatile window text.

Generic hosts such as `chrome.exe`, `python.exe`, or `cmd.exe` use their host profile by default.

Users MAY create more specific targeting with available identifiers such as:

- executable identity;
- binary path;
- package/application identity;
- explicit window matching.

Kivori MUST NOT automatically guess that ambiguous windows inside a generic host are separate logical applications.

### OS user scope

Configuration MUST belong to the active OS user.

When user ownership changes:

- the previous user's mappings MUST stop applying;
- the next user's configuration MAY activate only after that user's Kivori session is available;
- one user's mappings MUST NOT carry into another user's desktop session;
- previous-user session-specific render state MUST become non-renderable before the next user can observe the device.

MVP operation is scoped to the user's normal interactive session rather than requiring the entire Kivori desktop application to run permanently as SYSTEM/root.

## Acceptance Criteria

- [ ] Mouse hover does not switch profiles.
- [ ] Passive focus changes require stable OS focus before visible profile commitment.
- [ ] Very brief focus theft does not produce a committed profile swap.
- [ ] A Global bidirectional Rotate binding reserves both directions unless the complete binding is explicitly overridden.
- [ ] Profile precedence does not silently split opposite rotary directions across unrelated scopes.
- [ ] Binding-conflict behavior is explainable in Desktop.
- [ ] Virtual-desktop/workspace transition UI does not own a normal app profile.
- [ ] Newly visible app after workspace switch uses normal stabilization.
- [ ] Deliberate Kivori input during workspace-switch stabilization commits the pending app immediately.
- [ ] Recognized transient in-game overlays preserve the underlying game profile by default.
- [ ] Overlay classification uses non-invasive OS-observable context and/or explicit user rules.
- [ ] No single generic window flag is sufficient evidence by itself to classify an overlay.
- [ ] Ambiguous overlay classification falls back to normal focus stabilization rather than guessing.
- [ ] Overlay handling does not require process injection, memory inspection, or anti-cheat hooks solely for profile ownership.
- [ ] A persistent independently focused overlay/application may enter normal focus stabilization.
- [ ] An explicit transient-overlay override does not bypass Protected classification.
- [ ] Protected overlay/context overrides the game profile.
- [ ] Deliberate interaction during the stabilization window commits the valid pending normal foreground app before execution.
- [ ] A pending Protected context enters Protected behavior immediately rather than falling back to General.
- [ ] The previous app profile is not used for a new gesture merely because the stabilization timer is still running.
- [ ] An app that disappears before dispatch does not cause the gesture to be silently retargeted.
- [ ] An app that disappears mid-gesture cancels the remaining gesture stream.
- [ ] Known target loss is surfaced as known failure/Context Lost.
- [ ] Background apps do not take physical control ownership.
- [ ] Generic-host sub-app behavior is explicit rather than guessed.
- [ ] User A mappings cannot remain active after User B becomes the active user.
- [ ] User A session-specific visual data is not renderable to User B.

---

# US6 — Restricted, Protected, and Permission-Limited Contexts

## User Story

**As a user, I want Kivori to clearly communicate when the operating system prevents certain actions, so permission problems are not mistaken for connection failures.**

## Contract

Restricted/protected contexts include:

- UAC or equivalent secure prompts;
- secure system screens;
- lock screen;
- protected administrator context;
- missing or revoked OS permissions.

During protected contexts for MVP:

- normal app-specific custom actions MUST be suspended;
- custom macros MUST NOT execute;
- local physical acknowledgement MAY still occur;
- unavailable actions MUST be communicated clearly;
- Kivori MUST NOT silently change execution mechanism;
- the hardware recovery gesture defined in US10 MUST remain available whenever firmware can still observe it.

Protected/secure context classification MUST occur before any General-profile fallback.

When rendering is available, Protected and Permission Required states MUST have an intentional visual treatment; they MUST NOT look like a frozen normal buddy or unexplained blank screen.

### Permission restrictions

If Kivori Desktop remains connected while a capability permission is missing:

- connection state MUST remain Connected unless communication itself is lost;
- only affected capabilities SHOULD become unavailable;
- unaffected capabilities SHOULD continue working;
- blocked actions SHOULD NOT be retried continuously without a permission/state change.

If restrictions prevent most useful operation, Kivori MAY escalate to a **Permission Required / Setup Required** takeover state.

Permission failure MUST remain conceptually distinct from connection failure.

## Acceptance Criteria

- [ ] Connected-but-restricted is not reported as Disconnected.
- [ ] Capability restrictions can remain localized.
- [ ] Protected contexts suspend unsafe/custom mappings.
- [ ] A newly focused Protected context is not treated as General during focus stabilization.
- [ ] Blocked actions do not silently fall back to another mechanism.
- [ ] Repeated blocked actions are not spam-retried without state change.
- [ ] Protected context does not disable the hardware MCU recovery gesture.
- [ ] Protected/Permission Required presentation is visibly intentional when rendering is available.

---

# US7 — Clear Visual State Hierarchy

## User Story

**As a user, I want Kivori to communicate multiple desktop and device conditions without making the display cluttered or ambiguous.**

## Contract

Kivori uses four visual layers.

### Intentional visual feedback standard

When Kivori is powered and the renderer is available, every user-relevant state that could otherwise be mistaken for a frozen device MUST have an intentional presentation.

This includes, at minimum:

- Waiting;
- Host Starting / Resuming;
- Switching User;
- Reconnecting;
- Disconnected;
- Degraded system health;
- Protected;
- Permission Required;
- Firmware Updating;
- Processing / Delayed;
- Running / Busy;
- Error / Failed;
- Triggered / Unverified;
- Context Lost / cancelled action where feedback is useful;
- Passive / Unassigned;
- active hardware recovery hold;
- firmware rollback/recovery mode when known.

The presentation MAY combine buddy pose/expression, iconography, text, motion, progress treatment, or other visual language appropriate to the hardware.

**Motion communicates activity; state treatment communicates meaning.**

Kivori MUST NOT use fabricated progress values. When progress is not trustworthy, use indeterminate activity.

A static presentation is acceptable when static is semantically appropriate, but it MUST still make the state understandable rather than looking like a stale frame.

Deliberate Display Sleep is exempt because blanking is itself the intended state. Low-level conditions where normal rendering is technically unavailable are also exempt from the normal-renderer requirement, but any reliable fallback indication SHOULD be used where available.

## Layer 1 — Takeover State

Takeover states replace normal buddy presentation when normal desktop interaction is unavailable or intentionally suspended.

Defined takeover concepts include:

- Sleeping / Locked;
- Switching User;
- Protected;
- Permission Required;
- Host Starting / Resuming;
- Firmware Updating;
- Firmware Recovery / Restoring;
- Reconnecting;
- Disconnected;
- Waiting;
- Passive / Unassigned.

When a Layer 1 takeover condition becomes known and normal interaction is no longer valid, the takeover presentation MUST preempt lower visual layers without waiting for an active rotary gesture to reach its 250 ms end boundary.

If a normal gesture is active, US3's cancellation rules apply: remaining ordinary gesture input is discarded and the takeover becomes visible immediately.

### Switching User

When the OS is known to be transitioning between users, Kivori SHOULD preserve a Switching User/Locked transition state rather than briefly falling through Waiting.

Transition ends when:

- the next user's Kivori session is confirmed; or
- the normal startup grace period expires without a usable session, after which Waiting is appropriate.

During Switching User, the display MUST use neutral takeover presentation and MUST NOT render previous-user session-specific app/profile names, custom icons, transient feedback, or user-specific secondary indicators.

### Passive / Unassigned

Passive means a usable desktop session exists but this Kivori is not the active controller.

In MVP, Passive devices:

- MUST NOT execute normal mappings;
- MUST NOT mirror ordinary real-time desktop indicators;
- SHOULD show a calm connected-but-unassigned presentation;
- MUST still permit the hardware recovery gesture when firmware can observe it.

Passive MUST NOT be overloaded into visual Monitor Mode.

### Host Starting / Resuming

Initial startup/resume grace target: **15-30 seconds**.

During this grace state, absence of the user's Kivori session MUST NOT immediately be presented as an unexpected disconnect.

When rendering is available, Host Starting / Resuming SHOULD visibly communicate that Kivori is waiting for the host session rather than appearing idle or frozen.

### Firmware Updating and recovery presentation

Firmware update MUST be represented as an intentional state rather than an unexplained crash.

When phase information is known, presentation SHOULD distinguish meaningful update stages such as:

- Preparing;
- Downloading / Transferring;
- Firmware Updating;
- Verifying;
- Restarting;
- Restoring Previous Firmware;
- Recovery Mode.

While the normal renderer remains available:

- Firmware Updating SHOULD use a clear update/activity presentation;
- trustworthy progress MAY be shown when available;
- if exact progress is not trustworthy, Kivori SHOULD use indeterminate update activity rather than inventing a percentage.

If a new firmware image fails validation and rollback restores the previous image, Kivori Desktop MUST NOT call the update successful; it SHOULD clearly report **Update Failed — Previous Firmware Restored** or equivalent.

If the device is in application-independent bootloader/recovery mode, Desktop SHOULD present an explicit recovery flow rather than treating the device as an ordinary Disconnected unit.

If full rendering becomes unavailable during low-level update, Kivori SHOULD use the simplest reliable hardware-supported indication available. A temporarily blank display is acceptable only when the normal renderer and any other reliable device indication are genuinely unavailable; Kivori Desktop SHOULD present update state/progress where possible.

## Layer 2 — System Health

System health describes trustworthiness of current communication/state freshness.

- Healthy does not need permanent visual real estate.
- Degraded SHOULD receive dedicated visual priority.
- Degraded MUST NOT consume an ordinary secondary-indicator slot.
- Degraded SHOULD have an intentional visible cue when active rather than silently looking Healthy.

## Layer 3 — Primary Buddy State

Primary buddy states include concepts such as:

- Idle;
- Active;
- Busy;
- Success;
- Error;
- Unknown.

Only one primary buddy state should dominate at a time.

## Layer 4 — Secondary State Indicators

Secondary indicators may include:

- microphone muted;
- call active;
- master audio muted;
- media active;
- supported app-specific persistent state.

The default priority baseline is:

1. **Microphone state**;
2. **Call Active**;
3. **Master Audio Mute**;
4. **Media Playing/Active**;
5. **Custom app-specific indicators**.

This baseline is intentionally not fully user-configurable. System/privacy indicators MUST NOT be displaced by lower-priority custom app indicators.

Users MAY configure ordering/visibility within the custom app-specific tier, and future product settings MAY expose limited adjustments that preserve the protected system/privacy priority guarantees.

When the visible indicator budget is exceeded, lower-priority indicators SHOULD be condensed or hidden rather than shrinking all content indefinitely.

### Indicator layout stabilization

Indicator priority and layout stabilization use asymmetric behavior.

When a newly active indicator has higher priority than something currently visible:

- it SHOULD claim the required indicator slot immediately;
- Kivori SHOULD NOT delay urgent/high-priority information merely to preserve layout stability.

When a higher-priority indicator disappears and exposes a previously hidden lower-priority indicator:

- the lower-priority promotion SHOULD wait for an initial **~250 ms stable period** before reflowing the layout;
- multiple lower-priority changes during that window SHOULD be batched into one resulting layout update;
- if another higher-priority state appears during the stabilization window, the pending lower-priority promotion SHOULD be recalculated or cancelled.

Example with two visible slots:

`[Mic] [Call] -> Call ends -> hold current layout transition for ~250 ms -> [Mic] [Audio Mute]`

If another higher-priority indicator becomes active during that ~250 ms period, it may preempt immediately instead of waiting.

The product principle is:

**Priority escalation is immediate; lower-priority promotion is stabilized.**

Active profile information SHOULD normally appear transiently after a profile change rather than occupy a permanent indicator slot.

## Acceptance Criteria

- [ ] Takeover states cannot be confused with normal buddy state.
- [ ] User-relevant waiting/processing/recovery/restriction/failure states do not appear accidentally frozen when rendering is available.
- [ ] Unknown progress uses indeterminate activity rather than fabricated percentages.
- [ ] Display Sleep is clearly the deliberate blank-state exception.
- [ ] Layer 1 takeover becomes visible immediately when normal interaction becomes unavailable.
- [ ] An active rotary gesture does not delay Reconnecting/Disconnected takeover presentation.
- [ ] Degraded health has distinct visual allocation from secondary indicators.
- [ ] Switching User presentation contains no previous-user session-specific visual data.
- [ ] Passive displays do not mirror ordinary desktop status in MVP.
- [ ] Passive assignment does not disable MCU recovery.
- [ ] Fast User Switching does not unnecessarily flash through Waiting.
- [ ] Firmware update does not look like an unexplained disconnect or frozen screen when any reliable indication is available.
- [ ] Firmware rollback/recovery is distinguishable from successful update completion.
- [ ] Low-level recovery mode receives an intentional Desktop recovery presentation when detectable.
- [ ] Mic and other protected system/privacy indicators cannot be displaced by custom app indicators.
- [ ] A newly active higher-priority indicator can preempt immediately.
- [ ] Lower-priority indicator promotion waits for the stabilization window before causing layout reflow.
- [ ] Multiple changes during the promotion window are batched when practical.
- [ ] Secondary-state overload is handled through fixed baseline priority plus condensation/hiding.

---

# US8 — Localized Uncertainty

## User Story

**As a user, I want uncertainty or capability loss to affect only the information that cannot be verified or controlled, so one problem does not make the whole device appear broken.**

## Contract

If one secondary state cannot be verified while other states remain trustworthy, Kivori SHOULD preserve the trustworthy states and localize uncertainty to the affected state.

Example:

- desktop communication: confirmed healthy;
- master audio: confirmed muted;
- Discord microphone: unknown.

Expected result:

- the normal primary buddy remains;
- confirmed audio status remains visible;
- only the Discord microphone state is marked unknown/unavailable.

The entire primary buddy SHOULD enter Unknown only when the primary represented state itself cannot be determined reliably.

Permission-limited capability SHOULD likewise remain localized unless the restriction is broad enough to justify a takeover state.

For long-running execution, device-link uncertainty MUST NOT automatically propagate into job-failure state when the desktop host still has reliable job status.

When uncertainty is user-relevant and rendering is available, the uncertain state SHOULD be visually explicit rather than silently omitted when omission would make the device appear stale or misleading.

## Acceptance Criteria

- [ ] One unknown secondary state does not force whole-device Unknown.
- [ ] Confirmed states remain visible when unrelated state is uncertain.
- [ ] Whole-buddy Unknown is reserved for primary-state uncertainty.
- [ ] Permission/capability loss follows the same localization principle.
- [ ] Device-link degradation does not falsely convert known-running host work into Error.
- [ ] User-relevant uncertainty does not masquerade as a frozen/stale known state.

---

# US9 — System, Audio, Feedback, and Display-Idle Awareness

## User Story

**As a user, I want Kivori to adapt its controls, feedback, and presentation to important operating-system conditions.**

## Contract

### Audio routing

Global audio controls MUST follow the OS logical default endpoint rather than silently bypassing configured routing.

This includes virtual endpoints when the OS treats them as the default.

Application-specific audio controls MUST be explicitly configured and MUST remain distinct from global audio actions.

### Do Not Disturb / Focus Mode

Kivori distinguishes two feedback classes:

**User-initiated feedback**

Examples:

- button acknowledgement;
- rotary acknowledgement;
- deliberate `Test Action` feedback initiated by the user from Kivori Desktop.

This follows Kivori's own feedback settings and MAY remain enabled during OS Do Not Disturb.

A deliberate `Test Action` is software-originated but user-initiated. It therefore participates in the deliberate-interaction transient priority rather than being treated as an unsolicited background event.

**Background feedback**

Examples:

- unsolicited desktop event;
- background notification.

Background buzzer feedback SHOULD respect OS Do Not Disturb / Focus Mode by default.

### Monitor idle presentation

When the host remains awake while its monitor blanks, Kivori MUST remain Connected if the desktop session is still healthy.

Presentation MAY progress:

**Normal -> Dim -> Low Motion -> Display Sleep**

Burn-in/glare protection MAY include:

- reduced brightness;
- reduced animation;
- pixel repositioning;
- eventual complete display blanking.

### Ambient progression timing

The transitions into Dim, Low Motion, and Display Sleep SHOULD be independently configurable in Kivori Desktop for MVP.

Firmware/device logic MAY enforce hardware-safe minimum/maximum bounds so a user cannot configure values that meaningfully compromise panel/device safety.

The timings SHOULD therefore be preferences within a safe operating envelope rather than immutable firmware-only constants.

### Display wake semantics

When Kivori is fully in Display Sleep:

- deliberate button press MAY wake it;
- deliberate valid rotary detent MAY wake it;
- passive vibration/motion/USB activity MUST NOT wake it by default;
- future motion/proximity sensors MAY become explicit configurable wake sources.

A physical gesture that **begins** while fully in Display Sleep MUST be consumed as wake-only for the entire gesture.

For rotary input, the initial MVP wake-gesture boundary is **250 ms without a new detent**.

Therefore, if a user begins rotating while the display is asleep and continues rotating continuously for two seconds, all detents in that continuous burst are wake-only. The first actionable rotary input may occur only after the wake gesture ends and a new gesture begins.

Examples:

- a button press that wakes the screen MUST NOT also execute its assigned desktop action;
- a five-detent rotary burst that begins while asleep MUST wake the screen but MUST NOT execute ticks 2-5 as an action;
- a continuously spinning rotary wake gesture remains swallowed until at least 250 ms without another detent marks the gesture end;
- after the wake gesture ends, the next new gesture MAY execute normally.

When Kivori is merely Dim or Low Motion, the interaction SHOULD execute normally while restoring full presentation.

Display Sleep is an intentional presentation-only blanking state. Normal Waiting, Processing, Reconnecting, Protected, Error, or recovery conditions MUST NOT use unexplained blank output when normal rendering is available.

### Explicit desktop configuration test commands

`Test Action` and `Preview Buddy State` initiated deliberately from Kivori Desktop are explicit user test intent, not passive background activity and not physical wake gestures.

When the targeted device is in Display Sleep:

- `Preview Buddy State` SHOULD wake the display and render the requested preview without executing an ordinary desktop mapping;
- `Test Action` SHOULD wake the display and MAY execute the requested test immediately when the current assignment, permission, connection, and protection rules permit it;
- these software-originated tests do not require the user to perform a separate physical wake gesture first;
- the physical wake-only rule MUST NOT consume or defer a software-originated test solely because the panel was asleep;
- a test command MUST NOT bypass Protected, permission, connection, device-assignment, or higher-priority takeover restrictions;
- a higher-priority takeover state MAY refuse or interrupt a preview when normal preview presentation is inappropriate.

A software-originated wake from Display Sleep uses a **temporary wake lease** rather than resetting the entire ambient-idle sequence.

Initial post-feedback wake grace target: **~1 second** after the requested preview/test feedback has completed.

During this temporary wake lease:

- the pre-existing idle age SHOULD be preserved;
- Kivori SHOULD remain visibly awake for the active preview/test feedback plus the short post-feedback grace;
- if no deliberate physical interaction or higher-priority state occurs, Kivori SHOULD return directly to Display Sleep after the grace period rather than restarting Normal -> Dim -> Low Motion -> Display Sleep;
- a deliberate physical interaction while the temporary wake lease is active MAY reset normal ambient-idle timing and execute normally because the display is already awake;
- a higher-priority takeover/urgent state follows its own presentation rules and may extend/replace the temporary test presentation.

This keeps configuration testing visible without repeatedly defeating panel burn-in protection.

### Display Sleep and hardware recovery hold

The hardware recovery detector is out-of-band from ordinary desktop button-action handling.

If the primary hardware button is pressed while Kivori is fully in Display Sleep:

- key-down SHOULD wake the display immediately so the device visibly acknowledges the interaction;
- ordinary configured short/Hold actions MUST remain suppressed because the gesture began as a wake gesture;
- the hardware recovery hold timer MUST continue running independently after the display wakes;
- incidental rotary movement while the button remains held MUST NOT reset/cancel recovery and MUST NOT execute ordinary rotary actions;
- if the button remains held through the recovery-ownership and final recovery thresholds, the MCU MUST reboot according to US10;
- if the user releases before the recovery threshold, the display remains awake and no ordinary desktop action from that press is executed.

Example:

`Display Sleep -> button down -> display wakes -> normal actions suppressed -> incidental rotation ignored -> recovery owns hold -> ~10 s -> MCU reboot`

Early release example:

`Display Sleep -> button down -> display wakes -> release at 4 s -> no desktop action`

### Urgent desktop wake events

A small explicit class of immediate-attention desktop states MAY wake Display Sleep without physical interaction.

Examples include:

- an incoming call that requires timely user awareness;
- a microphone state transition to active when that transition is considered privacy/safety relevant.

Ordinary background changes MUST NOT wake the fully sleeping display merely because state changed. Examples include routine app-focus changes, media progress, ordinary volume changes, and non-urgent status refreshes.

Urgent wake classification SHOULD be narrow and deterministic. It MUST NOT become a generic notification-wakes-display mechanism.

## Acceptance Criteria

- [ ] Global audio follows the current OS logical endpoint.
- [ ] App-specific audio remains explicitly scoped.
- [ ] Background buzzer feedback respects DND by default.
- [ ] Desktop Test Action is classified as deliberate user-initiated feedback rather than ordinary unsolicited background feedback.
- [ ] Monitor blanking does not become Sleeping or Disconnected by itself.
- [ ] Dim, Low Motion, and Display Sleep timings are independently configurable within hardware-safe bounds.
- [ ] Display Sleep can fully blank the panel.
- [ ] Display Sleep is not confused with an unexplained blank normal state.
- [ ] A wake gesture that starts while asleep is swallowed in its entirety.
- [ ] A multi-tick rotary wake gesture does not partially execute desktop actions.
- [ ] Continuous rotary wake input remains wake-only until 250 ms of rotary inactivity ends the gesture.
- [ ] Explicit Preview Buddy State can wake a sleeping display without a physical wake gesture.
- [ ] Explicit Test Action can wake a sleeping display and execute immediately only when current restrictions permit.
- [ ] Desktop test commands do not bypass Protected/permission/assignment/takeover restrictions.
- [ ] Software test/preview wake preserves previous idle age instead of restarting the full ambient progression.
- [ ] Test/preview wake returns directly to Display Sleep after feedback plus the short grace when no new deliberate interaction occurs.
- [ ] Physical interaction during the temporary software wake may reset normal ambient timing.
- [ ] A recovery-button key-down wakes the display immediately from Display Sleep.
- [ ] Waking the display does not cancel the recovery hold timer.
- [ ] Incidental rotary detents during the recovery hold do not cancel recovery or execute normal rotary actions.
- [ ] Releasing a recovery-capable wake press before ~10 s does not execute ordinary mapped short/Hold actions.
- [ ] Passive incidental events do not wake Display Sleep by default.
- [ ] Narrowly defined urgent desktop states may wake the display.
- [ ] Ordinary background state changes do not wake Display Sleep.

---

# US10 — Connection, Power, Recovery, Session Transitions, and Execution Continuity

## User Story

**As a user, I want Kivori to distinguish between normal absence, startup, intentional interruption, sleep, user switching, passive assignment, communication failure, hardware recovery, firmware recovery, and host execution state.**

## Contract

### Connected

A usable desktop session exists and this Kivori is assigned for normal use.

### Waiting

Kivori is powered but no usable Kivori Desktop session is available.

Examples:

- wall power;
- secondary PC without Kivori Desktop;
- desktop software intentionally exited;
- current OS user has no Kivori session.

Waiting MUST NOT silently become a driverless macro mode.

When rendering is available, Waiting SHOULD have a calm intentional waiting presentation rather than a stale/frozen last-known desktop frame.

### Passive / Unassigned

A usable desktop session exists but this Kivori is not currently assigned as the active controller.

Passive MUST remain distinct from Waiting.

Normal mappings are disabled in Passive, but the hardware recovery gesture MUST remain available whenever firmware can observe it.

### Host Starting / Resuming

The host is booting/waking and Kivori is waiting for the user's desktop component.

Initial grace target: **15-30 seconds**.

If a usable session appears, transition to Connected. If it does not appear after the grace period, Waiting is appropriate.

A failure/disconnect semantic SHOULD apply only after a healthy session had actually been established and was then lost unexpectedly.

### Switching User and privacy boundary

When the OS is known to be transitioning between user sessions, Kivori SHOULD preserve the transition until the next user session is confirmed or the startup grace expires.

At the moment the previous user's ownership ends, Kivori MUST invalidate session-specific presentation data so it cannot be rendered to the next user.

Session-specific presentation includes, at minimum:

- previous-user app/profile names;
- user-specific custom icons/assets currently cached for display;
- transient action feedback tied to the previous session;
- user-specific buddy/secondary state that is not safe as a neutral machine-level state.

The product contract requires that previous-user session data becomes **non-renderable immediately**. Implementations SHOULD clear or invalidate relevant MCU/session buffers, but this product contract does not require forensic memory-erasure guarantees beyond preventing cross-user presentation or reuse.

If User B logs in and no Kivori Desktop session becomes available after the normal grace period, Kivori may transition from Switching User to Waiting only after the previous-user visual/session state has already been invalidated.

### Reconnecting

The desktop service is intentionally restarting, for example during a software update.

Actions MUST NOT be buffered for execution after Reconnecting completes.

Reconnection MUST restore current observable state rather than replaying expired UI feedback from events that occurred while disconnected.

If Reconnecting becomes known during an active ordinary gesture, Reconnecting takeover MUST surface immediately and the active gesture MUST be cancelled according to US3 rather than waiting for gesture-end inactivity.

When rendering is available, Reconnecting SHOULD visibly communicate ongoing reconnection activity rather than remaining on a frozen prior state.

For host-side jobs:

- a currently running job MAY restore a Running representation after reconnect;
- a job that completed in the past MUST NOT cause a stale Success/Error transient merely because the device reconnected;
- historical results MAY remain available in desktop-side logs/history;
- the original job command MUST NOT be reissued on reconnect.

### Degraded

Communication remains partially alive but is late/unhealthy.

Degraded SHOULD appear as System Health rather than automatically replacing the buddy with Disconnected.

Degraded device transport MUST NOT automatically mean a previously confirmed host-side long-running action has failed.

### Disconnected

A previously healthy device connection is lost unexpectedly.

Examples:

- service crash;
- USB data failure;
- unexpected heartbeat loss.

Disconnected describes device/session communication, not necessarily the lifetime of a host-side process that was already confirmed started.

If Disconnected becomes known during an active ordinary gesture, the gesture MUST be cancelled and Disconnected takeover MUST render immediately; Kivori MUST NOT wait for the 250 ms gesture-end inactivity threshold.

If Kivori later reconnects while the desktop service can still observe that process/job, current execution state MUST be reconciled without reissuing the original command or replaying expired historical transients.

When rendering is available, Disconnected MUST have a clear intentional presentation and MUST NOT leave the previous active desktop state frozen on screen as though it were still current.

### Sleeping / Locked

The host is known to be sleeping, hibernating, or locked.

Known sleep/lock/session state MUST outrank later generic heartbeat-loss symptoms.

### Hardware recovery

MVP recovery gesture:

**Hold the primary hardware button for approximately 10 seconds total from key-down -> force MCU reboot.**

Recovery MUST:

- work without Kivori Desktop;
- not depend on healthy USB communication;
- not erase normal configuration;
- remain available when the host PC is frozen or unavailable, provided firmware can still observe the recovery gesture;
- remain available while the device is Passive / Unassigned;
- remain available during Protected/restricted desktop contexts;
- remain available while the display is in Display Sleep;
- bypass normal profile/action suppression because it is an out-of-band device recovery path.

### Application-independent bootloader recovery

The ~10 second hold depends on firmware being healthy enough to observe the input. It is not the final recovery layer.

Production Kivori hardware MUST expose a documented application-independent recovery path into the MCU's built-in ROM/download bootloader or equivalent immutable recovery environment.

That path MUST:

- remain available even if the Kivori application image cannot boot;
- not depend on Kivori Desktop already having a healthy application-level session with the device;
- provide a practical way to reset/enter recovery through the final PCB/enclosure/support design;
- preserve service/test access required to restore firmware.

For the ESP32-C3 baseline, implementation specifications may map this requirement to the MCU's documented boot/reset strapping and ROM serial-download mechanism. The product contract intentionally does not hard-code one final enclosure gesture or PCB pad layout.

When Kivori Desktop can identify a device in this low-level recovery mode, it SHOULD present **Firmware Recovery / Restore Device** rather than ordinary Disconnected.

### Rollback-safe firmware update

Where the MCU/platform supports it, normal firmware installation SHOULD use a rollback-safe inactive-slot/A-B strategy rather than destroying the only known-good application image in place.

A newly installed firmware image SHOULD remain pending/unconfirmed until it reaches the product-defined boot/health validation point.

If the new image fails that validation and the platform can recover automatically:

- the previous known-good image SHOULD be restored/booted;
- Kivori Desktop MUST report the attempted update as failed/restored rather than successful;
- the failed update MUST NOT loop indefinitely on every reboot.

The application-independent bootloader path remains the final recovery layer if application-level rollback cannot execute.

### Recovery ownership and mapped Hold actions

The primary recovery-capable button uses an initial **~2 second recovery-ownership threshold** before the final ~10 second reboot threshold.

Before recovery ownership begins, a configured Hold action MAY be armed but MUST remain release-qualified.

When the continuous hold reaches the recovery-ownership threshold:

- any mapped Hold action for that same gesture MUST be cancelled before execution;
- recovery becomes the exclusive owner of the button gesture;
- incidental rotary detents MUST NOT cancel, reset, pause, or restart the recovery timer;
- incidental rotary detents MUST NOT execute ordinary mapped rotary actions;
- auxiliary/secondary button presses MUST NOT execute ordinary mapped actions;
- ordinary short/Hold actions for that button gesture MUST remain suppressed;
- releasing before the final threshold ends the recovery attempt without executing a mapped desktop action.

The final ~10 second recovery threshold is measured from the original key-down, not from the ~2 second recovery-ownership transition.

This arbitration takes precedence over ordinary gesture rules because recovery is an escape path rather than a normal configurable input gesture.

### Recovery hold presentation

When the renderer is available:

- after the short-press cutoff but before recovery ownership, Kivori SHOULD make an armed mapped Hold action understandable without falsely showing it as already executed;
- once recovery ownership begins, recovery visual feedback MUST take precedence over any pending Hold-action preview/transient;
- because the final recovery threshold is deterministic, Kivori MAY show truthful elapsed/remaining hold progress such as a ring or countdown toward reboot;
- releasing after recovery ownership but before the final threshold SHOULD visibly return to the appropriate underlying device/desktop state without executing the mapped action;
- reaching the final threshold SHOULD transition into an explicit Rebooting/recovery indication when possible before/reset as the MCU restarts;
- recovery MUST NOT look like an unexplained multi-second freeze.

When the recovery-capable button is pressed from Display Sleep:

- the display SHOULD wake immediately at key-down;
- ordinary mapped short/Hold execution MUST remain suppressed for that entire wake gesture;
- waking the display MUST NOT reset or cancel the recovery hold timer;
- reaching the recovery-ownership threshold MAY transition directly into recovery presentation;
- reaching the final recovery threshold MUST reboot the MCU;
- releasing before the final threshold MUST NOT retroactively execute any mapped short/Hold action.

When the button is pressed while already awake/Connected:

- a release within the initial 500 ms short-press window MAY execute the mapped short-press action;
- after 500 ms but before recovery ownership, release MAY execute the mapped Hold action when configured and permitted;
- once recovery ownership begins, mapped short/Hold execution is permanently suppressed for that gesture;
- releasing after recovery ownership but before ~10 s results in no mapped desktop action.

Factory reset MUST use a separate, harder-to-trigger mechanism.

If application firmware is so compromised that it cannot observe the normal recovery gesture, the low-level bootloader recovery path above is the required next recovery layer.

## Acceptance Criteria

- [ ] Waiting, Passive, Reconnecting, Degraded, and Disconnected remain semantically distinct.
- [ ] Intentional restart does not immediately appear as a crash.
- [ ] Startup/wake receives a grace state.
- [ ] Explicit sleep/lock state outranks heartbeat-loss symptoms.
- [ ] Switching User invalidates previous-user session-specific presentation before another user can see the device.
- [ ] Transition to Waiting after a user switch does not restore previous-user visual state.
- [ ] Degraded/Disconnected transport does not automatically mark confirmed host execution as failed.
- [ ] Reconnection reconciles current host execution without replaying its start command.
- [ ] Reconnection does not replay stale completion transients for historical host jobs.
- [ ] Reconnecting/Disconnected takeover preempts and cancels active normal gestures immediately.
- [ ] Waiting/Reconnecting/Disconnected look intentional rather than frozen when rendering is available.
- [ ] Recovery gesture works without desktop communication.
- [ ] Recovery remains available in Passive state.
- [ ] Recovery remains available during Protected/restricted desktop contexts.
- [ ] Recovery remains available in Display Sleep and wakes the display immediately on key-down.
- [ ] Production hardware provides a recovery path that does not require valid Kivori application firmware.
- [ ] A device in detectable ROM/bootloader recovery mode is distinguishable from ordinary Disconnected.
- [ ] Rollback-safe firmware installation preserves/reverts to a known-good image when platform support permits it.
- [ ] Failed validation after an update is not reported as successful merely because the device rebooted.
- [ ] A mapped Hold on the primary recovery-capable button does not execute merely at threshold crossing.
- [ ] Releasing before recovery ownership may execute an armed Hold action when configured and permitted.
- [ ] Crossing recovery ownership cancels the pending Hold action for that gesture.
- [ ] Recovery visual feedback supersedes pending Hold-action presentation once recovery owns the gesture.
- [ ] Rotary/auxiliary inputs during recovery ownership do not cancel/reset recovery or execute normal mappings.
- [ ] Recovery hold provides visible in-progress feedback when rendering is available.
- [ ] A 9-second hold released after recovery ownership does not execute mapped short/Hold actions.
- [ ] Display Sleep-originated recovery presses never execute mapped short/Hold actions from that gesture.
- [ ] Recovery does not erase configuration.
- [ ] Factory reset is not accidentally triggered by the ordinary recovery gesture.

---

# US11 — Desktop App Configuration, Machine Scope, Device Assignment, and Updates

## User Story

**As a user, I want Kivori Desktop to act as the control center while keeping machine, user, device ownership, versions, and update status explicit.**

## Contract

Kivori Desktop SHOULD provide configuration for:

- connected-device visibility;
- active/passive assignment;
- device and system-health status;
- General controls;
- application profiles;
- advanced app targeting;
- explicit action assignment;
- rotary sensitivity;
- acceleration behavior/reset timing;
- global bindings;
- binding conflict detection;
- visual feedback;
- buzzer feedback;
- Do Not Disturb behavior;
- Normal-to-Dim timing;
- Dim-to-Low-Motion timing;
- Low-Motion-to-Display-Sleep timing;
- configurable display-wake sources where supported;
- custom indicator visibility/order within the allowed custom tier;
- buddy-state preview;
- action testing;
- optional explicit transient-overlay classification rules;
- installed Desktop version;
- connected-device firmware version;
- connected-device hardware revision/identity where observable;
- update availability/status;
- desktop application updates;
- firmware updates/recovery;
- configuration reset.

The desktop configuration window MUST NOT need to remain visible for ordinary background operation.

Ambient timing controls MAY be constrained by hardware-safe minimum/maximum values. The UI SHOULD make such limits clear rather than silently ignoring invalid timing choices.

### Explicit test and preview behavior

User-triggered configuration commands such as `Preview Buddy State` and `Test Action` are explicit test intent.

When the selected Kivori is in Display Sleep:

- Preview Buddy State SHOULD wake the selected device and show the preview without requiring a physical wake gesture;
- Test Action SHOULD wake the selected device and MAY execute immediately when the target action is currently permitted;
- these commands MUST NOT be treated as passive background events;
- these commands MUST NOT bypass Protected state, missing permissions, unavailable connection, assignment restrictions, or other higher-priority takeover rules;
- Preview Buddy State MUST NOT execute an ordinary mapped desktop action merely because it woke the display;
- the temporary software wake SHOULD preserve the device's pre-existing idle age and return directly to Display Sleep after feedback plus the configured/product grace unless deliberate physical interaction or a higher-priority state occurs.

### Desktop test feedback priority

A user-triggered `Test Action` is deliberate user intent for feedback-priority purposes.

- its acknowledgement/result MAY immediately replace an older ordinary deliberate-interaction transient;
- it MUST NOT be classified as an unsolicited/background transient merely because it originates in software rather than a physical control;
- urgent/privacy/safety state and Layer 1 takeover/recovery presentation continue to outrank it;
- `Preview Buddy State` is likewise deliberate user intent, but it is a preview presentation rather than proof that a desktop action succeeded.

The configuration UI SHOULD make a blocked test distinguishable from a test that was actually dispatched.

### Overlay classification overrides

Kivori Desktop MAY expose an advanced rule such as **Treat this application/window match as transient overlay** for cases where ordinary OS metadata is insufficient.

Such an override:

- SHOULD be explicit and inspectable by the user;
- MUST NOT rely on process injection or hidden anti-cheat/game hooks;
- MUST NOT bypass Protected/restricted-context classification;
- SHOULD be scoped narrowly enough to avoid turning unrelated windows from the same host process into overlays accidentally.

### Update discovery and release metadata

Kivori Desktop SHOULD use one authoritative product release source/manifest (or equivalent trusted release metadata) to coordinate Desktop and firmware versions.

The update metadata SHOULD be sufficient to determine at least:

- available Desktop version(s) and platform-specific artifact;
- available firmware version(s);
- compatible hardware revision(s);
- minimum/compatible Desktop version required by firmware;
- minimum/compatible firmware requirements where Desktop behavior depends on them;
- release notes or meaningful change summary;
- update importance such as Normal, Recommended, Critical/Security, or Compatibility Required;
- artifact integrity/authentication information.

Kivori Desktop MUST NOT treat a version string alone as enough evidence that an artifact is safe/applicable to the connected device.

### Version and update presentation

Kivori Desktop SHOULD make installed/current versions easy to inspect, including:

- Desktop version;
- device firmware version;
- hardware revision/identity where observable;
- whether each component is up to date;
- last successful update check when useful;
- available release notes and compatibility requirements.

Routine update checks MAY happen in the background and SHOULD NOT create unnecessary interruption when nothing actionable changed.

When an update is available, the UI SHOULD make the distinction between available, downloading, ready, installing, restarting, verifying, restored/rolled back, recovery-required, and completed states understandable.

### Update consent and severity

For Normal and Recommended firmware releases:

- update availability MUST NOT itself authorize an immediate flash;
- Kivori Desktop SHOULD ask for clear user intent such as `Update Kivori` / `Update All` before disruptive installation begins.

Critical/Security or Compatibility Required updates MAY use stronger messaging or block incompatible feature use, but any mandatory-update policy MUST be explicit rather than silently flashing during ordinary activity.

Desktop self-update behavior MAY follow platform-appropriate updater semantics, but update installation/restart state MUST remain understandable to the user.

### Compatibility and `Update All`

Before installing Desktop or firmware updates, Kivori Desktop MUST evaluate known compatibility constraints.

If the desired firmware requires a newer Desktop version, Desktop MUST be updated first.

If a Desktop version requires a particular firmware range for a capability, the UI MUST communicate that requirement rather than pretending the incompatible pair is fully supported.

`Update All` MUST:

1. resolve the required dependency order;
2. show the user what components will change;
3. install one dependency stage at a time when required;
4. re-evaluate compatibility/state after a component restarts;
5. avoid launching a dependent firmware flash if the prerequisite Desktop update failed.

Kivori MUST NOT intentionally install a known-incompatible Desktop/firmware pairing merely because both artifacts are individually newer.

### Artifact authentication

Desktop and firmware update artifacts MUST be authenticated/integrity-checked before installation according to the implementation's signed-update/security design.

An artifact that fails verification MUST NOT be installed and MUST be reported as an update verification/integrity failure rather than ordinary network failure or successful update.

The contract does not require one specific signing scheme, but it requires that production update delivery be designed around authenticated artifacts rather than trusting an unauthenticated downloaded binary solely because it came from an expected filename/URL.

### Desktop application update behavior

When a Desktop update is installed and the Kivori background service/application intentionally restarts:

- the physical device SHOULD use Reconnecting / Host Updating semantics rather than unexpected Disconnected when that intent is known;
- physical inputs MUST NOT queue for later replay during the update restart;
- after Desktop restarts, current device/desktop truth MUST be reconciled;
- stale pre-update transients MUST NOT be replayed.

### Firmware update flow

Before firmware installation, Kivori Desktop SHOULD verify:

- selected/connected target device identity;
- hardware revision compatibility;
- Desktop/firmware compatibility;
- artifact authentication/integrity;
- usable transport/update prerequisites;
- whether a recoverable installation path is available for the target hardware.

During installation:

- normal mappings MUST be suspended for the target device;
- the device/Desktop SHOULD expose truthful update phase/progress as defined in US7;
- the original firmware-update command MUST NOT be blindly replayed after an uncertain disconnect;
- the update result MUST be based on actual post-update validation/reconciliation, not simply the fact that a reboot occurred.

If rollback restores the previous firmware, Desktop MUST surface that outcome explicitly.

If the device enters application-independent bootloader recovery mode, Desktop SHOULD offer a **Restore Firmware** workflow and the documented physical procedure required to enter that mode when automatic detection is unavailable.

### Machine scope

MVP profiles are local per machine and per OS user.

Cloud synchronization is not required.

Future portability MAY distinguish:

**Portable configuration**

- logical application identity;
- action choices;
- preferences.

**Machine-local configuration**

- executable/file paths;
- scripts;
- hardware/device identifiers;
- machine-specific overrides.

### Multiple devices

MVP supports **one Active Kivori per desktop session**.

Additional connected devices remain **Passive / Unassigned**.

When Device B becomes Active:

- Device B becomes Active;
- the previous Device A becomes Passive / Unassigned;
- Device A MUST NOT become Waiting solely because assignment changed;
- Device A MUST NOT silently mirror Device B;
- Device A MUST NOT execute ordinary mappings;
- Device A MUST NOT behave as Monitor Mode;
- Device A MUST retain the hardware recovery gesture.

Future multi-device roles MAY be introduced only through explicit assignment semantics.

## Acceptance Criteria

- [ ] Configuration UI can identify which device is Active.
- [ ] Activating a second device demotes the previous Active device to Passive.
- [ ] Passive devices do not execute ordinary mappings.
- [ ] Passive devices do not mirror ordinary desktop state.
- [ ] Passive devices retain hardware recovery.
- [ ] Normal-to-Dim, Dim-to-Low-Motion, and Low-Motion-to-Display-Sleep timings are independently configurable within safe limits.
- [ ] Preview Buddy State can explicitly wake a selected sleeping device without executing a mapped action.
- [ ] Test Action can explicitly wake a selected sleeping device when testing is otherwise permitted.
- [ ] Test Action feedback is treated as deliberate user interaction rather than unsolicited background feedback.
- [ ] A newer Test Action may replace an older ordinary deliberate-interaction transient while remaining below urgent/takeover/recovery presentation.
- [ ] Test/preview software wake preserves idle history and returns to Display Sleep unless new deliberate activity occurs.
- [ ] Test/preview commands do not bypass Protected/permission/connection/assignment restrictions.
- [ ] Explicit overlay rules can be configured without invasive hooks and cannot bypass Protected classification.
- [ ] Desktop, firmware, and hardware revision/version information is visible when observable.
- [ ] Update metadata encodes compatibility and artifact verification information, not only a latest version string.
- [ ] Normal/recommended firmware updates require clear user intent before disruptive flashing begins.
- [ ] `Update All` computes dependency order and does not flash firmware after a failed required Desktop update.
- [ ] Known-incompatible Desktop/firmware combinations are not silently installed as if supported.
- [ ] Desktop and firmware artifacts are authenticated/integrity-checked before installation.
- [ ] Failed artifact verification prevents installation and is surfaced distinctly.
- [ ] Intentional Desktop updater restart uses Reconnecting/Host Updating semantics when known.
- [ ] Firmware update success is validated after reboot rather than inferred from reboot alone.
- [ ] Automatic rollback is reported as update failure/restoration, not success.
- [ ] Detectable bootloader recovery mode offers a restore/recovery workflow.
- [ ] System/privacy indicator priority cannot be overridden by custom app indicator ordering.
- [ ] Profiles are scoped per machine and OS user in MVP.
- [ ] Desktop background behavior continues without the configuration window being visible.

---

# 4. State Reference

| State | Meaning | Normal controls? |
| --- | --- | --- |
| Connected | Healthy usable desktop session; device assigned | Yes |
| Waiting | Powered, but no usable Kivori Desktop session | No |
| Passive / Unassigned | Desktop session exists; this device is not the active controller | No ordinary mappings; recovery remains available |
| Host Starting / Resuming | Host session is expected to become available | No |
| Switching User | Known OS user-session transition; previous-user session presentation invalidated | No |
| Reconnecting | Intentional temporary service interruption | No deferred execution; immediate takeover; restore current truth only |
| Degraded | Device communication unhealthy but partially alive | Limited/current device actions only; host execution may remain valid |
| Disconnected | Established device/session connection unexpectedly lost | Immediate takeover; no new device actions; confirmed host work may continue independently |
| Sleeping / Locked | Host explicitly inactive/restricted | No normal custom actions |
| Protected | Secure/protected system context | No normal custom actions; recovery remains available |
| Permission Required | Broad capability restriction requiring user intervention | Only unaffected capabilities |
| Firmware Updating | Intentional device firmware update | No normal controls; truthful update phase/progress where available |
| Firmware Recovery / Restoring | Update rollback or application-independent restore path is active/required | No normal controls; restore/recovery only |

# 5. Initial Timing Reference

| Behavior | Initial target |
| --- | ---: |
| Local input acknowledgement | < 50 ms |
| Short-press maximum hold | 500 ms |
| Recovery-ownership hold threshold | ~2 s |
| Delayed/processing indication | ~500 ms |
| Ordinary interactive unresolved timeout | ~1,500 ms |
| Success transient | ~800 ms |
| Triggered/Unverified transient | ~1,200 ms |
| Error/Failed transient | ~2,000 ms |
| Desktop test post-feedback wake grace | ~1 s |
| Passive stable-focus commit | 300-500 ms |
| Rotary acceleration inactivity reset | 250 ms |
| Rotary gesture-end inactivity | 250 ms |
| Lower-priority indicator promotion stabilization | ~250 ms |
| Host startup/resume grace | 15-30 s |
| Hardware recovery reboot threshold | ~10 s total from key-down |

These values are validation targets, not protocol constants. Tuning them MUST preserve the semantics defined by the relevant user story.

A deliberate Kivori interaction is not required to wait out the passive 300-500 ms focus-stabilization target when a valid pending foreground application is known; that interaction commits the pending context immediately after security/protection classification.

A direction reversal resets acceleration immediately even when the prior direction has not been idle for 250 ms, but only after the input layer has formed a validated logical reverse detent.

Acceleration builds only while validated detents continue in the same direction; repeated valid direction flips therefore repeatedly restart at baseline.

Initial MVP rotary acceleration multiplier ceiling: **5x the action's base step**. Actions may choose a lower ceiling or disable acceleration, but MUST NOT exceed 5x in MVP.

A rotary wake gesture remains one wake-only gesture until the 250 ms rotary gesture-end inactivity target is reached.

Lower-priority secondary indicators use an initial ~250 ms promotion-stabilization target; higher-priority escalation is not delayed by that timer.

For the primary recovery-capable button, crossing 500 ms suppresses short press, while crossing the initial ~2 s recovery-ownership threshold cancels any still-pending mapped Hold action. The final ~10 s reboot threshold remains measured from the original key-down.

Software-originated Test Action / Preview Buddy State wake uses the initial ~1 s post-feedback grace and preserves prior idle age rather than restarting the full ambient sequence.

# 6. Default Priority References

## Transient Reaction Priority

1. Urgent immediate-attention state;
2. newest deliberate user-interaction feedback, including user-triggered Kivori Desktop `Test Action` feedback;
3. ordinary unsolicited/background transient.

A new deliberate interaction replaces older interaction feedback rather than waiting in a visual queue.

A user-triggered Desktop `Test Action` participates in this deliberate-interaction tier even though the command originates in software. `Preview Buddy State` is deliberate presentation intent at the same interaction level, but MUST NOT be interpreted as action-success confirmation.

Reconnection does not replay expired historical action transients.

## Secondary Indicator Priority

1. Microphone state;
2. Call Active;
3. Master Audio Mute;
4. Media Playing/Active;
5. custom app-specific indicators.

Custom indicator settings MAY reorder or hide indicators within the custom tier but MUST NOT displace protected system/privacy indicators.

Priority escalation SHOULD appear immediately. When a higher-priority indicator disappears and exposes a lower-priority indicator, lower-priority promotion SHOULD use the initial ~250 ms stabilization window before reflowing the layout.

## Visual Feedback Priority

When rendering is available, the product SHOULD preserve this presentation intent:

1. safety/privacy/urgent state;
2. Layer 1 takeover/recovery/update state;
3. active deliberate user interaction or running action feedback, including Desktop Test Action feedback;
4. system-health indication;
5. primary buddy state;
6. secondary indicators.

Once recovery ownership begins, recovery presentation outranks any pending mapped Hold preview for the same button gesture.

Firmware Updating / Firmware Recovery presentation outranks ordinary interaction feedback for the target device.

This priority does not authorize false progress or hidden state mutation; it only governs which truthful information gets visual precedence.

# 7. MVP Boundaries

The following are explicitly outside this contract's MVP guarantees:

- cloud profile synchronization;
- independent virtual-pet progression;
- generic news/weather/dashboard behavior;
- workflow orchestration as the flagship use case;
- automatic inference of ambiguous sub-app identity;
- implicit simultaneous-input chords;
- simultaneous independent ordinary actions from multiple physical controls while another gesture owns input;
- implicit OS-style key-repeat from an ordinary release-qualified discrete button mapping;
- silently splitting one normal bidirectional rotary control between unrelated Global and application-profile actions;
- automatic transactional rollback of successfully completed earlier macro steps after a later step fails;
- acceleration above 5x the configured action base step;
- reconstructing rotary detents the hardware did not observe;
- treating raw invalid electrical bounce as a semantic rotary reversal;
- profile ownership by virtual-desktop/workspace transition UI;
- user-configurable short-press or recovery-ownership threshold in MVP;
- firing a primary-button mapped Hold action merely because its threshold is crossed before the gesture's recovery intent is known;
- replaying physical input after communication recovery;
- delaying Reconnecting/Disconnected takeover presentation until an active rotary gesture naturally ends;
- replaying historical Success/Error presentation transients after reconnection;
- automatic restart of an uncertain long-running host action after device reconnection;
- silent retargeting of a gesture after its committed application disappears;
- continuing application-scoped execution after known target loss within the same gesture;
- process injection, memory inspection, anti-cheat hooks, or hidden game instrumentation solely to preserve profile ownership across transient overlays;
- assuming one generic OS window flag proves that a surface is a transient overlay;
- restarting the full ambient-idle sequence solely because a desktop Test Action / Preview Buddy State temporarily woke Display Sleep;
- multiple simultaneously Active Kivori controllers;
- Passive devices acting as Monitor Mode;
- driverless macro/media fallback without Kivori Desktop;
- host wake-from-Kivori;
- visibility into physical device states that are not exposed through a trustworthy signal;
- fully user-defined priority that can demote system/privacy indicators below custom app indicators;
- forensic guarantees about erasing every historical RAM byte during local OS-user switching, beyond the requirement that previous-user session state becomes non-renderable and non-reusable;
- fabricated progress percentages when trustworthy progress is unavailable;
- silently flashing a Normal/Recommended firmware update solely because a newer release exists;
- installing update artifacts that fail authenticity/integrity verification;
- intentionally installing a known-incompatible Desktop/firmware/hardware combination as though it were supported;
- treating a reboot by itself as proof that firmware update succeeded.

# 8. Change Control

Any implementation or future feature that intentionally violates a MUST-level rule in this contract should update this contract and the root PRD in the same product decision/PR, including the reason for the behavior change.

Implementation-specific timing, APIs, protocols, storage formats, signing schemes, bootloader mechanics, update endpoints, or OS adapters may evolve without changing this contract as long as the user-observable behavior remains compliant.
