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

## Evidence and remaining work

Frontend tests cover a test-only dial without screen/reactions/media messages, mixed capabilities,
identity pending/unknown/recovery, offline preview, binding retention and focus after switching
sessions. Native session tests cover control-specific input refusal and Mirror refusing to stage
state while pending or disconnected. Fake devices are injected in tests, never registered for shipping.

Catalog artwork delivery and capability-aware setup are implemented as described above. Product
acceptance, provisioning/tool defaults, multi-image packaging and the identifier guard remain
separate M2.8 work. Catalog metadata does not implement a new interaction model. See
[ADR-0017](./adr/0017-product-contract-and-capability-driven-desktop.md) and the
[M2.8 checklist](./roadmap.md#m28-desktop-organization-and-product-contract).
