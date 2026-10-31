# Desktop capability surfaces

This is the current main-window mapping for M2.8. Capability names refer to the existing wire
contract in `kivori-model`; they describe implemented messages, not inferred product traits.
The backend supplies current-session effective capabilities. Live UI additionally requires a
connected session with a known identity. Retained metadata never authorizes a live action.

## Navigation and configuration

Home, Controls, Display and Device stay available in every connection state. Controls and Display
contain settings saved on this computer, so disconnecting or changing products never hides their
editors. All control-id bindings stay stored and editable, including controls absent on the current
device. Unsupported control rows are marked **Saved only** after identity resolves. Host-only
**Test** actions retain the existing action catalog, permission checks and countdown; they do not
require a connected device.

Developer is always visible. Its master toggle defaults off and owns Diagnostics, Activity,
Social reactions and Device Studio. Studio also requires its static frontend build gate and native
build flag; its renderer, playback and reaction previews work offline. Device and Developer both
expose basic Copy diagnostics with the toggle off. Only the active tool mounts, so leaving Internals
or switching the toggle off stops its polling. A removed focused control returns focus to the page
title; Developer itself stays available when its tools are turned off.

Copy diagnostics reads the safe base on demand. Developer mode may add only internals already
observed by the active instrumentation in the same connection generation. The optional envelope
labels fresh (at most five seconds old), stale or unavailable data; stale and unavailable entries
carry no measurements. Report age starts at native receipt, so cached IPC reads do not renew it.
Session transitions, polling failures and leaving instrumentation invalidate the frontend cache;
late replies cannot restore it. Copy never starts internals polling.


Home shows a small Link health card only while developer mode is on. It uses current connection
status for link state and consecutive retries, and the existing safe diagnostics cache for the
native reconnect total since app start (excluding the first connection). Cache reads occur about
once a second only while the card is mounted and the window is visible. Missing, failed or
mismatched-session totals remain unavailable. Home never requests device internals or activity
history, and turning developer mode off or leaving Home stops these reads.

Catalog name/model and an optional artwork key are exposed on `DeviceInfoDto.presentation`, never
inside copied diagnostics. Shipping builds reject names/models/art keys over the IPC limit of 96
UTF-16 units. Home, Welcome and Tour show those labels and only that product's bundled artwork for
a known current connected product/board. Missing/pending/unknown/disconnected identities never
borrow another product's picture; missing/failed artwork uses a neutral figure. The fixed
hardware-body illustration is removed; Display keeps its existing software-view illustrations.

Catalog `image` is empty or a lowercase kebab-case `.png` filename directly inside its product
directory. The build refuses symlinks, missing or malformed files, APNG, files over 512 KiB and
dimensions outside 1–1024 pixels per side. It fully decodes with a bounded build-only decoder and
checks the remaining chunks, then embeds a copy of the validated original bytes (including PNG
metadata). Cargo's recursive catalog watch covers artwork changes. `get_product_art` serves only
compiled keys as PNG data URLs: no runtime filesystem access/downloads, and no image bytes in status
updates or diagnostic copies. Lookups are cached in the frontend; product changes, disconnects and
image errors cannot retain the wrong image. Pip currently declares no artwork and stays neutral.

Setup uses the same effective control rules as Controls. Plug-in Continue requires a known usable
current session, including compatible legacy baseline sessions; a transport connection alone is
insufficient. Pending identity waits, unknown identity suggests updating the app, and identity-only
firmware points to Device after setup. Skip remains available and BOOT recovery is unchanged.

The Tour is a non-blocking guide, not a hardware test. It lists rotary input, push-switch and context
buttons only when their effective capabilities are present. It has no completion ticks or retained
progress: host volume changes and generic action results cannot identify physical controls or the
session that produced them. Continue remains available, including offline and when no controls are
available. The step list stays stable through capability/session changes (only the existing host
permission filter applies). Attributed input verification would require separate backend/IPC work.

Customization keeps saved view/Buddy editors available in every session state, with `Saved only`
notes whenever the current session lacks their features, including offline. Profile guidance is
generic; middle-button pin guidance requires physical/context-button support. Product switches
change guidance only and never write configuration. Launch-at-login consent is still applied only
on Finish; Skip setup leaves it unchanged.

## Surface mapping

| Surface | Current rule |
| --- | --- |
| Home computer monitoring, microphone, media and last action | Host readings and host action outcomes; independent of device capabilities. Unknown readings retain their existing unavailable explanations. |
| Controls Test / rotary Test | Host action execution, available offline and before identity resolves. Normal action validation and protected-foreground rules still apply; device feedback uses effective session support. |
| Home product figure and setup figures | Catalog name/model only for a current connected identity with both product and board known. Only that product’s bundled artwork, otherwise a neutral figure; never infer hardware art from capabilities. |
| Setup Plug-in Continue | Known current usable session, including legacy baseline; pending/unknown/identity-only cannot pass as ready. Skip stays available. |
| Setup Tour | Non-blocking guidance using the same rotary/push-switch/context-button rules as Controls; no hardware verification or progress inferred from host status. |
| Setup saved view/Buddy | Always editable; `Saved only` without the applicable current feature, including offline. Pin hint only with physical/context-button support. |
| Home Showing chip, Change view | `deskStatusV1`. |
| Home Buddy report | A current, resolved, usable session with an actual reported state. This is baseline protocol data, not proof of transient reaction support. |
| Developer Social reactions | Developer mode and `mascotInteraction`. Direct reaction controls are absent from Home and Display. |
| Display Show now | `deskStatusV1`, checked again by the backend. All five modes belong to this capability, including Buddy and Media. |
| Saved default and secondary view | Always editable. Default view requires `deskStatusV1` to reach the device; secondary view requires physical input, push-switch and double-press support to be invoked. |
| Saved brightness | Always editable; sent through `BuddySettings` only with `idleLifeV1`. |
| Saved buddy settings | Always editable; used by device idle life (`idleLifeV1`) or desktop reactions (`mascotInteraction`). |
| Rotary inputs | `physicalInputV1`. |
| Push-switch press and hold | `physicalInputV1` and `buttonInputV1`. |
| Push-switch double press | Physical input, push-switch and `doublePressV1`. |
| Context buttons | `physicalInputV1` and `contextButtonsV1`. |
| Device Studio reaction preview | Host renderer always; request device playback only with `mascotInteraction`. |
| Device Studio Mirror | Sendable state and a usable live session. Pending, unknown, disconnected and identity-only recovery sessions cannot change a later device's desired state. |
| Developer Device Internals | Active Diagnostics tool with developer mode on, plus effective `deviceInternalsV1`; the existing visible-card polling and backend snapshot rules remain. |

Production desired-state changes retain the user's choice while offline or unresolved, then sync it
only to a usable session. Studio Mirror deliberately does not stage state for a later device.
Live display changes require current effective `deskStatusV1` and an idle flash workflow; an ordinary
flash requires a connected identity and its exact validated bundled image. Known identity-only
firmware can be flashed without authorizing ordinary display or mascot actions.

Queued live commands are checked against the session present when the device thread executes them;
they carry no originating connection generation. Activity request entries can precede refusal and
do not prove that a device applied an action. Binding requests to their originating session or
changing activity outcome semantics would require a separate implementation change.

`mediaInfoV1` adds now-playing title/artist messages. It is not a prerequisite for the Media view,
whose playback state is in `Status`. `mascotInteraction` enables transient actions and acknowledgments;
it is not a substitute for a screen or baseline-mascot capability. No new protocol bits are added.
Baseline `SetState` remains part of the current compatible protocol, including legacy firmware with
no advertised feature bits. This step does not invent a per-product baseline-state model.

The remaining message capabilities do not hide host readings or saved editors:

| Capability | Surface or behavior |
| --- | --- |
| `presentationV1` | The native rotary pipeline's transient device overlay; no separate desktop navigation item. |
| `actionFeedbackV1` | Device-side action outcome messages. Home and Controls still show native host outcomes without it. |
| `controlLabelsV1` | Labels sent to the device. Saved binding labels remain editable without it. |
| `hostTakeoversV1` | Native sleep/update lifecycle messages; no feature page is hidden by this bit. |
| `quietAckV1` | Native trigger-feedback rendering policy; no extra desktop control. |
| `micStateV1` | Device microphone indicator messages. Host microphone readings remain available independently. |
| `productIdentityV1` | Identity exchange and legacy fallback under ADR-0014; not an ordinary interactive feature. |

Known identity-only firmware shows firmware attention guidance and links to Device for the existing
guarded matching-image workflow. Unknown identity shows app-update guidance and remains refused by
flashing. Pending identity shows waiting status. These states keep configuration and host monitoring.

## Evidence and scope

Frontend tests cover a test-only dial without screen/reactions/media messages, mixed capabilities,
identity pending/unknown/recovery, offline preview, binding retention and focus after switching
sessions. Native session tests cover control-specific input refusal and Mirror refusing to stage
state while pending or disconnected. Fake devices are injected in tests, never registered for shipping.

Catalog artwork delivery, capability-aware setup, product acceptance, provisioning/tool selection,
multi-image packaging and the identifier guard are implemented. The
[software closure audit](./validation.md#m28-software-closure-audit) maps the product-contract and
capability-UI criteria to their evidence. Hardware acceptance and PR review/merging remain separate.
Catalog metadata does not implement a new interaction model. See
[ADR-0017](./adr/0017-product-contract-and-capability-driven-desktop.md) and the
[M2.8 checklist](./roadmap.md#m28-desktop-organization-and-product-contract).

## macOS Accessibility

macOS lets an app press keys only with the Accessibility permission. Kivori asks by itself exactly
once, and only while macOS reports it off. Already on means nothing is shown anywhere: no setup
step, no card, no prompt (AXIsProcessTrusted is read right before anything shows, and again when
the window regains focus, so anything showing disappears once it is granted).

- First run: setup has an Accessibility step, shown only while it is off. **Allow** shows the macOS
  prompt once and opens the Accessibility list; the step turns green and continues by itself once
  macOS says on. **Not now** moves on and is listed as still open on the last step.
- An install that finished setup before Kivori asked (an update, or setup skipped): on start, if it
  is off and Kivori never asked, one in-app card explains why (shortcuts need it; media and volume
  do not) with **Allow** and **Not now**.
- "Asked" is stored once (`onboarding.accessibilityAsked` in `config.json`) the moment the step or
  card shows, and by any button press that asks. After that nothing asks by itself again, whatever
  the person chose; "Reset all settings" keeps it.
- After that the just-in-time notices stay: the Controls page shows the request when a keyboard
  shortcut is bound, and Home shows it when an action failed for lack of it. Settings, General has a
  permanent Accessibility row (macOS only): Allowed, or Not allowed with the ask.
- Each button press asks once; nothing prompts repeatedly. If macOS still reports it off after the
  user came back from System Settings, the notice offers "Reset and ask again", which runs
  `tccutil reset Accessibility` for Kivori's own bundle identifier only, asks once and opens the
  settings pane.

| Action | Needs Accessibility on macOS | Why |
| --- | --- | --- |
| Play/pause, next, previous | No | Sent to the now-playing app through the bundled adapter (Apple's own `perl`). Only if the adapter is unavailable or fails does the media key event take over, and then it needs the permission |
| System volume, output mute, knob volume | No | CoreAudio |
| Microphone mute, app mute | No | CoreAudio / session controls |
| Launch an app | No | Opens the app through the OS |
| Keyboard shortcut (a button or the knob in a shortcut profile) | Yes | A synthesized key event is the only way to type into another app |
| Macro | Only if a step is a keyboard shortcut | Same |

The default bindings (push switch: play/pause and mute; buttons: previous, play/pause, next; knob:
volume) need none. The built-in Browser, Code, Zoom and Teams profiles use shortcuts, so they need it
while those apps are in front. The Diagnostics "Input permission" row reads Not needed while the
active profile presses no keys. Windows has no such permission and is unchanged.
