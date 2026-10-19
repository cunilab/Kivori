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

Catalog customer name/model metadata is exposed separately on `DeviceInfoDto.presentation`, never
inside copied diagnostics. Shipping builds reject display names/models over the IPC limit of 96
UTF-16 units. Home, Welcome and Tour use a neutral product figure, with those labels
only for a known current connected product/board. Missing/pending/unknown/disconnected metadata
never borrows another product's picture. The fixed hardware-body illustration is removed; Display
keeps its existing software-view illustrations. No art is delivered yet: a non-empty catalog `image`
is a build error until the bundled-art step implements validation and delivery. The existing Tour
control checks and customization assumptions remain separate capability/onboarding work; this
presentation step does not claim that setup supports every product's controls.

## Surface mapping

| Surface | Current rule |
| --- | --- |
| Home computer monitoring, microphone, media and last action | Host readings and host action outcomes; independent of device capabilities. Unknown readings retain their existing unavailable explanations. |
| Home product figure and setup figures | Catalog name/model only for a current connected identity with both product and board known. Neutral figure in every case until bundled artwork delivery exists; never infer hardware art from capabilities. |
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

## Evidence and remaining work

Frontend tests cover a test-only dial without screen/reactions/media messages, mixed capabilities,
identity pending/unknown/recovery, offline preview, binding retention and focus after switching
sessions. Native session tests cover control-specific input refusal and Mirror refusing to stage
state while pending or disconnected. Fake devices are injected in tests, never registered for shipping.

Catalog-provided product art/control metadata, catalog-driven onboarding
and diagnostic snapshot freshness are separate M2.8 items. This mapping does not claim those are
complete or that catalog metadata can implement a new interaction model. See
[ADR-0017](./adr/0017-product-contract-and-capability-driven-desktop.md) and the
[M2.8 checklist](./roadmap.md#m28-desktop-organization-and-product-contract).
