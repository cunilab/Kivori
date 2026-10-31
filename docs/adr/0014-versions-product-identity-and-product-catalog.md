# ADR-0014: Versions, product identity and the product catalog

- **Status:** Proposed
- **Date:** 2026-10-12
- **Milestone:** M2.7
- **Related:** [ADR-0002](./0002-one-wire-protocol-crate.md), [ADR-0015](./0015-app-updates-through-github-releases.md), [ADR-0016](./0016-locking-a-board-to-its-firmware-in-tiers.md)

## Context

Kivori will be one desktop app for several hardware products, so identity and versions are designed for that now.

## Decision

**Versions.** Four numbers, one meaning each: the **app version** (Cargo workspace version, the `vX.Y.Z` tag, the only thing users install), a **firmware version per product** (that product's firmware crate version, bumped only when it changes), the **protocol version** plus capabilities (the only compatibility rule), and the **config schema version**. No rule ever pairs an app version with a firmware version: devices sit in drawers, and pairings grow into a grid nobody maintains. The app supports the current and previous protocol major and bundles every supported product's firmware, so it can always update a device.

**Identity.** `products/registry.toml` assigns fixed machine ids and `u16` wire numbers to products and boards (`pala` 1, `c3-supermini` 1), never reused or renamed; display names live in the catalog. `HelloAck` is a fixed struct (adding fields is a major change, ADR-0002), so the device sends a new `Identity { product, board, hw_rev, source }` message right after the handshake behind `PRODUCT_IDENTITY_V1`; a device without it is the legacy Kivori Pip on the C3 SuperMini, revision unknown. VID:PID is never identity (a stock C3 always reports `303A:1001`); boards with native USB get their own ids later (Raspberry Pi and Espressif both allocate PIDs to product makers). Identity comes from the firmware build (each build targets one product and board) until provisioning writes it into one-time chip memory (the C3 user eFuse block, the RP2350 OTP): a versioned record (format, product, board, revision, CRC) written by the provisioning tool after showing it and a typed confirmation; from then on firmware reports the burned record (`source = chip`).

**Images.** Every firmware image carries an identity record next to its version marker (product, board, firmware version, minimum protocol); the app checks it against the device before flashing and refuses a mismatch, and firmware compares its build identity with a burned record at boot and shows a wrong-board screen on mismatch. A device that cannot handshake (recovery) is matched by the user picking the product when more than one could fit.

**Catalog.** `products/<id>/product.toml` (display name, image, boards, flash method per board, bundled firmware file with version, SHA-256 and minimum protocol, required capabilities) is compiled into the app; device code reads it instead of assuming one board, and features stay capability-driven, never `if product == …`. Firmware crates live under `firmware/<product>/` sharing `crates/`.

**Releases.** release-please turns conventional commits into one release PR with a version and changelog per component; the app tags `vX.Y.Z` and gets a normal GitHub Release whose title names the bundled firmware; firmware tags `fw-<product>-vX.Y.Z` get no GitHub Release, so "latest release" always means the app.

**As built (M2.8 product artwork).** Catalog `image` is empty or a lowercase kebab-case `.png`
filename directly inside the product directory. The desktop build refuses symlinks and validates
static PNGs with a build-only decoder: at most 512 KiB, 1–1024 pixels per side, no APNG, complete
frame decoding and remaining-chunk checks with bounded decoder allocation. The validated original
bytes (including PNG metadata) are copied to `OUT_DIR` and embedded; Cargo recursively watches
`products/` for metadata and artwork edits. The current connected known product/board exposes only
an optional compiled art key beside its display name/model. A separate `get_product_art` command
returns the allowlisted bytes as a PNG data URL, cached by the frontend; status events and diagnostic
copies carry no image bytes or paths. Home/Welcome/Tour use only that product's art, otherwise a
neutral figure, and suppress stale loads and image errors. Pip's `image` remains empty until actual
artwork exists. This does not implement capability-aware setup controls or multi-image firmware
packaging.

## Consequences

The catalog format leaves room for firmware as its own downloads later.

Naming (2026-10-12): products get an Indonesian-spice codename as machine id (`pala` = Kivori Pip, model KV-P1); customer names live in the catalog.

**As built (step 1).** `products/registry.toml` holds two tables (`[products]`, `[boards]`) of id to `u16`; 0 is reserved for unknown and never assigned. `product.toml` carries `display_name`, `short_name`, `model`, `image`, `required_capabilities` and a `[boards.<id>]` table with `flash`, `usb` (VID:PID hints) and `[boards.<id>.firmware]` (`crate`, `artifact`, `min_protocol`). The firmware version, SHA-256 and identity record are not in the catalog yet: the version stays read from the image's marker, and `build.rs` fails the build if an embedded image differs from the version of the catalog's firmware crate. `apps/desktop/src-tauri/src/catalog.rs` embeds and validates the files (also run by `build.rs`); discovery's USB allowlist and the provisioning helper's required capabilities now come from it.

**As built (step 2).** Identity on the wire is `Identity { session, product, board, hw_rev, source }` (tag 23, `PRODUCT_IDENTITY_V1`, bit 15) with `source` limited to `Build` and `Chip`: the desktop's `legacy` source is not a wire value. It rides protocol minor 6 with the rest of the 1.6 additions, so `PROTOCOL_MINOR` is unchanged. The chip record is 12 bytes (format 1, product, board, revision, two reserved zero bytes, CRC-32 over the first 8) and lives in `kivori_protocol::identity`; this step only reads it, from the C3 user eFuse block through `esp_hal::efuse`, and an invalid record is ignored like an empty one (source `Build`), so a damaged record never bricks a unit. The firmware's `build.rs` reads the numbers and the minimum protocol from `products/` with a small line scanner instead of a `toml` build dependency. The image record is `KIVORI-FW-IDENTITY:p=..;b=..;v=..;m=..`, and `build.rs` of the desktop also checks it against the catalog. A device whose product or board the catalog does not know is shown as unknown, gets no capabilities beyond identity, and is refused by the flashing guard rather than handled through the `Incompatible` connection state, which only the handshake can enter. Restore without a handshake asks the user to pick only when more than one catalog product lists the port's USB ids (`restore_firmware(product)`).

**As built (step 3).** The burn goes through `espefuse` 4.x (`burn_block_data BLOCK_USR_DATA`, which needs a file of the whole 32-byte block, so the 12-byte record is zero padded), is dry-run unless `--burn-identity` is given, and refuses any block that is not all zero. The burn happens before the flash (flashing restarts the unit, so the verification sees the burned record); the provisioning flash also applies the image-versus-device guard, checked with a handshake before anything is written.

**As built (step 4).** `release-please-config.json` has two components. `desktop` (root, `simple` release type, tag `vX.Y.Z` without the component, `CHANGELOG.md`) updates `[workspace.package] version` in `Cargo.toml` through a TOML `extra-files` entry (the `rust` type cannot read a virtual workspace); it ignores `firmware/`, `hardware/` and the simulator folders (since removed), and shared `crates/` count for the app only, so a change in a shared crate bumps the firmware only through a commit that touches `firmware/desk-buddy`. `fw-desk-buddy` (`firmware/desk-buddy`, `rust` type, tag `fw-desk-buddy-vX.Y.Z`) sets `skip-github-release`; because release-please may then neither create that tag nor re-label the release PR, `release-please.yml` creates the missing firmware tag and the `autorelease: tagged` label itself. The app release is created as a draft with `force-tag-creation`, and `release.yml` retitles it, writes its notes and uploads the assets, so one draft exists per version. Title and notes are generated by `scripts/release-info.mjs` from the firmware crate version and the catalog display name. `Cargo.lock` is synced by a CI commit on the release PR branch (`cargo update --workspace`). `bootstrap-sha` is the merge of PR #71 so the M2.7 commits land in the first changelogs (the first firmware bump will therefore be a minor).

**As built (release start).** release-please, its config and manifest, and `release-please.yml` are gone, replaced by a manual Release workflow (`release-start.yml`, owner decision 2026-10-12): a standing release PR kept growing on every merge, while releases should happen only when the owner chooses one. The owner runs it on `main` with an app bump (`patch`, `minor`, `major`; a major below 1.0.0 gives 1.0.0) or an exact version, and a firmware bump (`none` by default) or an exact version, optionally as a dry run. `scripts/release-bump.mjs` reads the versions from `Cargo.toml` (`[workspace.package]`) and `firmware/pala/Cargo.toml`, refuses a version that is not greater or whose tag exists, and writes them; the job refreshes both lockfiles with `cargo update --workspace` (the firmware lockfile also records the shared crates, which carry the app version), commits `chore(release): vX.Y.Z` as `github-actions[bot]`, pushes it with `vX.Y.Z` (and `fw-pala-vA.B.C` when the firmware moved) in one atomic push, and calls `release.yml`. Which component changed is now the owner's call, not path rules. There are no changelog files: `scripts/release-info.mjs` builds the notes from `git log` since the previous `v*` tag, grouped as Features, Fixes, Performance and Breaking changes, with a Firmware section ("unchanged", or the commits touching the firmware folder when its version changed); the draft rule, title and firmware tags are as before, except that the title names the product by the catalog's `short_name` (`Kivori X.Y.Z · Pip firmware A.B.C`). The `bootstrap-sha` point above no longer applies.

**As built (trunk-based releases, 2026-10-20).** The direct-main release push described above is
superseded. `main` is the only permanent development branch; small PRs are squash-merged after CI
and owner review. The manual Release button's `prepare` stage opens a temporary `release/vX.Y.Z`
draft version PR. The owner marks it ready for review to start normal PR CI. After its squash merge and successful host, firmware and
determinism push CI, `finalize` validates that PR and tags its exact merged commit. Only tag refs
are pushed; `main` protection needs no automation bypass. The existing independent firmware tags,
input guards and manual draft publication remain. No standing release PR or per-merge release is
introduced. A versionless prepare request is refused because it cannot produce a reviewable diff.
See [release.md](../release.md#trunk-and-version-tags).

**As built (M2.8 firmware release guard, 2026-10-15).** The Release button now refuses an unchanged
firmware version when its committed build inputs differ from the last firmware tag. The owner
still chooses the version, but the input check replaces the earlier owner-only bump heuristic.
It runs before version/output writes, resolves both lockfiles with `--locked`, and excludes only
harmless workspace-version bookkeeping by content. Full history, ancestry and exact overrides are
checked; details and the hand-pushed-tag limitation are in [release.md](../release.md#release-policy).

**As built (firmware build id, 2026-10-31).** Which exact build a unit runs is reported, not inferred: `BuildInfo { session, commit, dirty }` (tag 24, `BUILD_INFO_V1`, bit 16) follows `Identity`, and `PROTOCOL_MINOR` is 2 under the ADR-0002 discipline for a new post-handshake message. It is a separate message because `HelloAck` is a fixed struct and an unnegotiated peer must never see a new tag; the firmware version, catalog and release semantics are untouched, and the image identity record is unchanged. The commit is `git rev-parse --short=12 HEAD` at build time and `dirty` means uncommitted changes under the firmware's own build paths; without git the commit is unknown. The desktop shows it on Device and warns about uncommitted changes always, and about a commit that differs from the app's own only in developer mode (a release user's older firmware commit is normal). Details in [architecture.md](../architecture.md#firmware-build-id-build_info_v1-adr-0014).

Versions reset (2026-10-12): before the first release, app and firmware restarted at 0.1.0 and the protocol at 0.1; earlier version numbers in these records belong to pre-release development.

## Alternatives considered

- Firmware as its own downloads now: rejected, a second signing chain and server, and code fetched from the network onto a device; the catalog format leaves room for it later.
- Lockstep versions: rejected.
- One version for everything: rejected, every firmware fix would bump every product.

**Amendment (2026-10-30, owner decision): one name.** The product id is now the product's short name
in lowercase, so code, folders and tags use the name people say. `pala` became `pip`: `products/pip/`,
`firmware/pip/`, registry `pip = 1` (the wire number is unchanged, so shipped and legacy units keep
working), desktop `LEGACY_PRODUCT = "pip"`, and new firmware tags `fw-pip-vA.B.C`. The pushed tags
`fw-pala-v0.1.0`–`v0.1.2` stay as history and count as Pip's firmware lineage (release tooling reads
both prefixes and `firmware/pala/` history). Numbers are still never reused or renumbered; an id
rename now happens only by owner decision. Earlier text in this ADR that says ids are never renamed,
or names `pala`, describes the state before this amendment. Future products follow the same rule once
their short names are decided.
