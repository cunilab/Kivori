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

**As built (M2.8 provisioning QA policy).** The explicitly bundled image selects the catalog QA
product/board, required capabilities, discovery hints and input groups. Identity arguments assert
that target even without a burn. Required groups come from catalog minima, not negotiated omissions;
unsupported required inputs fail instead of timing out or disappearing. Newly flashed firmware
must report the matching Build/Chip identity and image version. Baseline state/health/heartbeat
checks remain unconditional. Identity burning is restricted to the implemented `c3-supermini`
ESP32-C3 backend; that board binding is an explicit hardware-backend exception. Previewing identity
still flashes/checks firmware and does not burn eFuses. Build recipes now use the catalog metadata described below; whole-catalog packaging is implemented below.

**As built (M2.8 firmware build recipes).** Optional `firmware.build` metadata carries target and
features; shipping boards must supply it. One Bun helper builds the explicitly selected catalog pair
(or the sole active pair), with no default based on catalog order. Local build/desktop/provision/bundle
recipes, PowerShell and the release firmware job consume its verified artifact path. Builds pin the
crate-local target directory, use the firmware crate's working directory, disable default features,
require the selected binary and preserve target linker flags by refusing global Rust flag overrides.
Provision identity flags remain assertions, separate from build selection. Board-specific development
flash/test recipes remain explicit. Whole-catalog packaging is implemented below; pair-level recovery
choices and release-set metadata are implemented below.

**As built (M2.8 recovery target guard).** A live session identity associated with the selected port takes
precedence over USB hints; mismatching/unknown identities cannot fall back to identity-free recovery.
Recovery refuses a missing/unknown image identity, a port
with no catalog candidates, a product choice outside that port's candidates. Multiple possible boards now require
an explicit pair choice as described below. Image availability never resolves hardware identity or catalog order.
Without USB ids the pure resolver considers the full catalog; runtime still requires exactly one
USB-hinted port. Multiple candidates use the explicit pair chooser described below. The runtime slice below
replaces single-image mismatch refusal with exact lookup. Candidate refusals use the existing generic failure
under the existing recovery rules. Queued requests cannot replace an active flash workflow.
Per-device image selection and pair-level recovery choices are implemented below.

**As built (M2.8 image-set foundation).** Explicit legacy-path or manifest input is validated as an
immutable image set. Exact product/board lookup never falls back to catalog order; duplicate keys,
ambiguous markers and invalid catalog/version/protocol records are refused. Generated sources are
ordered by wire pair. Native consumers now select the exact current identity's image; no bundle
entry acts as a hardware default. Recovery resolves a single allowlisted port and its catalog target
before lookup, refuses pending live identities and requires an explicit choice for ambiguous boards, and cannot borrow another
port's identity. Missing target images are unavailable. Status version/size/advice follow the selected
identity, while accepted operations retain their image and idle refresh preserves queued reservations.
Incompatible recovery offers carry no assumed version. Provisioning flags select an exact embedded
pair, and no-argument provisioning requires a sole image. Multiple images can now be embedded;
hardware acceptance remains open; pair-level recovery and release orchestration are implemented below. Synthetic fixtures are not shipping products.

**As built (M2.8 catalog firmware packaging).** Desktop development/bundle recipes, PowerShell and
release CI build every active pair in deterministic wire order. Each successful build is copied
before a later board can overwrite the same Cargo output. Fresh staging directories and exclusive
manifest creation prevent stale output reuse. Only ELF files cross runner boundaries; Windows
checks the exact expected filenames and recreates absolute paths locally. Native manifest mode
independently requires exact equality with the active catalog identity set and validates all image
records before emission. The single-image path remains available for targeted development and
provisioning. Targets derive from catalog recipes; release CI expects stable crate toolchains.
Single-target selectors cannot narrow desktop packages.

**As built (M2.8 catalog release orchestration).** Release tooling reads the same complete catalog
and names every bundled product/version in wire order. Independent per-product override maps permit
mixed bumps/exact versions in one release. All product/crate guards run before any version/output
write, including unchanged products and planned manifest edits. Each product's boards require one
explicit package version; firmware crates are standalone workspace roots. Shared firmware crates
require consistent resolved versions across their product lineages. Every unique lockfile is
refreshed, only planned version/lock paths may be staged, and the app plus new firmware tags are
pushed atomically. Single-product title/notes behavior and legacy firmware history are preserved.
Synthetic two-product fixtures execute planning, notes and the actual workflow shell against local
bare remotes; tests do not trigger a real release. Hardware acceptance remains open; pair-level recovery is implemented below.

**As built (M2.8 recovery pair choices).** Ambiguous no-handshake recovery offers exact catalog
product/board pairs. Each button opens the existing BOOT confirmation with both names visible;
cancel never queues a restore. The selected pair must still be present in the latest failed-choice
status, and connection changes close the dialog. The existing `chooseProduct` token is retained;
its payload now includes board id/name and the UI validates complete, unique pairs.

The device thread re-enumerates on every confirmed request and requires exactly one allowlisted
port. USB candidates are narrowed by product and board; invalid/stale selections and a board without
a product are refused. Product-only callers remain supported when one board is unambiguous. A
same-port live identity overrides USB hints and refuses any differing selection; pending or unknown
identities cannot fall back to the chooser. Images are looked up only after target resolution, and
missing images never select a different pair. Busy reservations remain exclusive. Choice metadata
contains public catalog ids/names, never port paths. Onboarding retains its generic BOOT restore;
the Device firmware card hosts the pair chooser. These fixtures perform no device writes.

## Consequences

**As built (M2.8 software closure audit).** Product contract and capability-driven UI implementation
are verified against the [acceptance and closure evidence](../validation.md#m28-software-closure-audit).
The merged implementation completes the software checklist, including catalog-driven provisioning,
packaging and release selection and current-session feature gating. Supported-feature additions need
no shared-app product list; new hardware backends and interaction models remain separate feature work.
PRs #77–#103 are merged into `main`, with post-merge CI passing as recorded in the closure evidence.
This does not change this ADR's Proposed status or claim a release, a second physical product or
completion of hardware acceptance.

**As built (M2.8 automated product acceptance).** The app-shell scenarios and native session/runtime
admission checks now cover the planned synthetic product matrix. Existing catalog and image-set
generators compile their temporary multi-product fixtures and exact indexes without shared product
lists. The [acceptance evidence map](../validation.md#m28-product-acceptance-evidence) identifies each
check and its limits. Hardware rows remain open. Live queued commands use the execution-time
session; activity request entries are not proof of application. Host action testing and staging the
user's desired state remain available offline, unlike device reactions, live views and Studio Mirror.

Implementation and checks for this contract are tracked in
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

**As built (M2.8 identifier guard).** `just check-identifiers` runs the Bun guard and its fixtures;
Ubuntu/Windows host CI run both. Identifiers come from all registry ids (including reservations),
active catalog names/models/board labels and firmware crate paths. Compound board-label words such
as `SuperMini` are derived only when the catalog contains that camel-cased word; chip tokens are
not added on their own. Case, camelCase, underscore, space, kebab and compact spellings are checked
in file paths and UTF-8 text. This is a token check, not an AST or a proof of product-independent
behavior: escaped literals, numeric suffixes and compounds without camel/separator boundaries and unnamed layout assumptions still need review
and behavioral tests.

Scope is `apps/desktop/`, `crates/`, `tools/`, `scripts/`, workflow and Cargo configuration, plus
root manifests/toolchain/justfile. Git supplies tracked and untracked, nonignored files; tracked
deletions are skipped. Product firmware crate contents, docs, test paths/suffixes, vendored code,
lockfiles and binary contents are excluded. Binary filenames remain checked. Rust `#[cfg(test)]`
module bodies are excluded with bounded braces after masking literals and nested comments only
for structural detection; original production strings/comments and code after test modules remain
checked. Unsupported test attributes are conservatively scanned. Unterminated structural inputs
fail instead of masking to end-of-file.

`scripts/product-identifier-exceptions.json` is the complete literal exception inventory: exact
path, trimmed line, occurrence count and reason. A new duplicate, stale allowance or unlisted
matching line fails; no runtime/mock source file is exempted wholesale. The 21 reviewed entries
cover fixed legacy constants/history, the implemented board-specific identity-burn backend,
browser-only mock fixtures, Python package-manager guidance and existing hardware development/CI
feature matrices. The latter are deliberate debt until a catalog-driven hardware matrix exists;
shared desktop build/packaging/release selection is already catalog-driven. Generic documentation
examples no longer name the shipping product. A changed exception requires review, not an automatic
baseline refresh.

The offline dependency guard also uses packaging's validated TOML resolver for every unique
firmware workspace and its source root, using NUL-delimited paths to preserve spaces. Discovery
failure stops the check. Its fixtures verify a second custom crate is visited and a network API
there is rejected. The identifier positive controls inject leaks in isolated Git repositories and
require actual CLI failure, including untracked files, camelCase and production code after test
modules. A separate test asserts the real repository is scanned and clean. These guards supplement
the behavioral acceptance evidence; they do not complete hardware/recovery rows.

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
