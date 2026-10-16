# ADR-0017: Product contract and capability-driven Desktop

- **Status:** Proposed
- **Date:** 2026-10-15
- **Milestone:** M2.8
- **Related:** [ADR-0014](./0014-versions-product-identity-and-product-catalog.md), [ADR-0013](./0013-device-internals-on-request.md), [issue #76](https://github.com/cunilab/Kivori/issues/76)

## Context

ADR-0014 establishes one app for several products. The compiled catalog and session capability
negotiation already exist, but Desktop artwork, controls, provisioning and build paths still assume
Kivori Pip. A second product without a screen or mascot must not inherit Pip's picture or controls.

There is only one shipping product design. Predicting every future device would add machinery
without evidence. This record defines the supported extension boundary; it does not claim that
catalog data can implement a new transport, flash method or interaction model.

## Decision

**Product contract.** A product using supported capabilities, controls, transport and flash methods
is added through the permanent registry entry, its `products/<id>/product.toml`, referenced art and
its firmware crate. It needs no product-specific branches in shared Desktop behavior. A new kind of
feature needs a separate implementation and design, shared by capability where appropriate.
The permanent registry is a sanctioned product list; the forbidden lists are duplicates in shared
app source or tooling.

The registry owns machine ids and wire numbers. The catalog owns customer names, model, boards,
USB discovery hints, firmware build/artifact paths, required capabilities and optional product art.
Firmware declares its identity and supported capabilities. Build, packaging, provisioning and
release tools consume that metadata rather than choosing a product through a hardcoded default.
The existing hand-maintained catalog embedding list must be replaced by build-time discovery or
generation before claiming catalog-only additions; no runtime product download is introduced.

**Firmware bundles.** M2.8 includes replacing the current single-image bundle with a catalog-driven
set of supported product/board images. Build validation checks each image against its catalog
identity and firmware version; update advice and flashing select the matching image, never the
first catalog entry. Packaging and release tools build and describe the same set. This is explicit
implementation work, not something catalog discovery alone supplies. Test it with two supported
product fixtures, including a wrong-image refusal. It does not add simultaneous multi-device UI.

Recovery with no handshake identity uses catalog USB/board hints only to find candidate images,
never as proof of identity. One candidate may be offered under the existing explicit flash
confirmation. Multiple candidates require an explicit, confirmed product/board choice; no candidate
means refusal. Never pick the first entry or an implicit default. A device that does report an
unknown identity remains refused rather than falling back to this no-handshake path. Test two
products sharing the same USB/board hints and the no-match case.

**Capabilities.** Catalog `required_capabilities` is a minimum product requirement, not the live
feature set. The backend derives effective feature support from current-session negotiation,
resolved identity and required host support, and exposes that support to the UI. While identity
is pending or unknown, ordinary product features are unavailable in both UI and command handling;
identity exchange and guarded recovery remain possible. Legacy identity counts as resolved under
ADR-0014. Whether identity is resolved gates features; which product id it names never selects
feature behavior. Keep backend checks: hiding a control is not a safety boundary.

M2.8 must document and implement a capability-to-surface mapping for navigation, cards, controls
and display views, reusing the current capability definitions. Separate offline configuration from
live device actions. Binding/display configuration remains editable when unplugged, using configured
or last-known product metadata where available, otherwise a neutral configuration state. Retained
metadata never authorizes commands or masquerades as a connected device. Host-only action tests
keep their existing permission/safety rules; they do not require device capabilities.
M2.8 preserves the existing OS-user/machine binding storage and control-id semantics; it does not
add independent per-product profile stores. Switching products must not erase or rewrite bindings
for controls absent on the new product. Product-specific profile storage needs a separate design
when a real product requires it.

| Session state                              | Product presentation and live actions                                                                              |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| Connecting or identity pending             | Neutral art, waiting/status; no ordinary product actions.                                                          |
| Known identity with supported features     | Catalog art or neutral fallback; only effective features and valid host actions.                                   |
| Known identity with identity-only firmware | Catalog art or neutral fallback; no ordinary product controls; offer guarded reflash of matching bundled firmware. |
| Unknown product or board                   | Neutral art; explain that the app may need updating; refuse flashing an image for an unknown identity.             |
| Incompatible protocol                      | Status and explicit recovery/update guidance under the existing version and flashing rules.                        |
| Disconnected                               | Preserve offline configuration and general settings; no live device actions; explicit recovery remains guarded.    |

A previous session's capabilities never authorize a new session. If capability loss hides a live
page, navigate to an available page and move focus there; disconnect must not discard configuration
or hide its editor. Recovery never bypasses product/board checks or existing confirmation rules.

**Art.** Product art is optional and bundled. A missing image uses a neutral frame, never another
product's picture. The Kivori app icon and brand can stay shared. Device previews still use the
shared renderer; metadata must not introduce a second frontend implementation of device pixels.

**Developer tools and support.** A Developer page is always visible, with a master toggle off by
default. It owns Activity, technical protocol details, Device Internals and direct reaction tools.
Device Studio appears there only when the build includes it and developer mode is enabled.
Runtime developer mode in a production build is supported; it cannot enable compile-excluded IPC,
raw-payload logging or Studio. Turning it off removes developer controls and stops internals polling.
The small developer-only Home health summary is the deliberate exception to page consolidation.

Basic Copy diagnostics remains available with developer mode off. The Developer page also exposes
the same action. Internals may join the bundle only while developer mode is on and its visible
instrumentation has a snapshot from the current session. Copying does not start background polling.
Missing, stale or unsupported measurements must be identifiable; never attach a prior device's
snapshot. Bind cached internals to a connection generation/session and clear them at every session
boundary, including reconnect while the page stays mounted. Reject late replies from an old
generation. Capture age/freshness so a failed poll cannot make an old value appear current; omit
stale measurements from the bundle with an explicit unavailable/stale status. Session correlation
stays internal and does not add a raw identifier to copied diagnostics. Keep the diagnostic
allowlist: no raw device ids, MACs, paths, ports or payloads.

**Operating constraints.** Core operation stays offline, with no required account or cloud sync.
Firmware remains bundled with the app; there are no downloaded per-device software modules.
Configuration belongs to the active OS user and machine, not on-device hardware profiles. The
existing isolated signed app updater remains the network exception, not a product plugin system.

**As built (M2.8 capability-aware setup).** Plug-in readiness uses the current known usable-session
rule, including legacy baseline support, with separate pending/unknown/identity-only guidance and
Skip. The Tour is a non-blocking guide filtered by the same effective control policy as Controls.
It no longer infers physical input from host volume or generic `lastAction`, which have no
control/source/session attribution. No completion state survives a product or session change
because no verification state is stored. True attributed input verification needs separate
backend/IPC work. Saved view/Buddy configuration remains editable offline with saved-only notes;
profile guidance is generic and the middle-button pin hint requires physical/context-button
support. Session switches never rewrite configuration. Launch-at-login consent still applies only
on Finish. No protocol, catalog schema, per-product profile store or hardware-test claim is added.

## Consequences

This is a target contract, not an as-built claim. Implementation and checks are tracked in
[M2.8](../roadmap.md#m28-desktop-organization-and-product-contract). Existing legacy identity rules
and wrong-product/board flashing refusals remain mandatory.

Acceptance requires frontend rendering and navigation tests for Pip, a test-only product without
screen/mascot/media, and mixed capability sets. Cover old firmware, missing art, pending/unknown
identity, known identity-only recovery, offline configuration, disconnect and switching sessions.
Test a command attempted before identity resolves, and a reconnect with a late internals reply
while the card stays mounted. A test must show supported controls remain usable as well as
unsupported ones disappearing. Rust fake-catalog identity tests alone do not prove frontend behavior.
Catalog/build tests must also prove a supported product can be registered and embedded without
editing a product list in shared app source, and that each firmware image is selected by matching
product/board identity rather than catalog order.
Keep fake catalog entries outside shipping discovery roots and inject them only in tests. Synthetic
images may exercise metadata/selection checks but are not hardware evidence or shipping firmware.
Release validation must prove that bundled images match the shipping catalog's supported
product/board set and exclude test entries. Registry numbers reserved for retired/future products
do not themselves require a bundled image. Acceptance also covers preserving bindings when a
product without those controls connects.

A scoped CI identifier guard supplements those tests. Derive product names/ids from the catalog,
and allow explicit contract files, product firmware/art, legacy handling, tests, docs and the guard's
own fixtures. Document exceptions. Include a positive control that inserts a known leak and proves
the guard fails. A clean grep cannot detect unnamed assumptions about screen or button layout.

## Alternatives considered

- Universal catalog-only support for arbitrary hardware: rejected; data cannot supply missing
  transport, protocol or interaction implementations.
- Product-id branches throughout the app: rejected; names and artwork belong in metadata, and
  shared behavior belongs behind supported capabilities.
- Runtime plugins or downloaded firmware/modules: rejected; a compiled catalog and bundled
  firmware keep installation, offline operation and update trust bounded.
- Cloud/account configuration or on-device hardware profiles: rejected; configuration belongs to
  the active OS user and machine, and the core must work offline.
- Identifier grep as the sole acceptance test: rejected; it misses behavior assumptions and can
  reject harmless references. Behavioral and catalog/build checks supply the actual evidence.
