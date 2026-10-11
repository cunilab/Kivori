# ADR-0014: Versions, product identity and the product catalog

- **Status:** Proposed
- **Date:** 2026-10-12
- **Milestone:** M2.7
- **Related:** [ADR-0002](./0002-one-wire-protocol-crate.md), [ADR-0015](./0015-app-updates-through-github-releases.md), [ADR-0016](./0016-locking-a-board-to-its-firmware-in-tiers.md)

## Context

Kivori will be one desktop app for several hardware products, so identity and versions are designed for that now.

## Decision

**Versions.** Four numbers, one meaning each: the **app version** (Cargo workspace version, the `vX.Y.Z` tag, the only thing users install), a **firmware version per product** (that product's firmware crate version, bumped only when it changes), the **protocol version** plus capabilities (the only compatibility rule), and the **config schema version**. No rule ever pairs an app version with a firmware version: devices sit in drawers, and pairings grow into a grid nobody maintains. The app supports the current and previous protocol major and bundles every supported product's firmware, so it can always update a device.

**Identity.** `products/registry.toml` assigns fixed machine ids and `u16` wire numbers to products and boards (`desk-buddy` 1, `c3-supermini` 1), never reused or renamed; display names live in the catalog. `HelloAck` is a fixed struct (adding fields is a major change, ADR-0002), so the device sends a new `Identity { product, board, hw_rev, source }` message right after the handshake behind `PRODUCT_IDENTITY_V1`; a device without it is the legacy Desk Buddy on the C3 SuperMini, revision unknown. VID:PID is never identity (a stock C3 always reports `303A:1001`); boards with native USB get their own ids later (Raspberry Pi and Espressif both allocate PIDs to product makers). Identity comes from the firmware build (each build targets one product and board) until provisioning writes it into one-time chip memory (the C3 user eFuse block, the RP2350 OTP): a versioned record (format, product, board, revision, CRC) written by the provisioning tool after showing it and a typed confirmation; from then on firmware reports the burned record (`source = chip`).

**Images.** Every firmware image carries an identity record next to its version marker (product, board, firmware version, minimum protocol); the app checks it against the device before flashing and refuses a mismatch, and firmware compares its build identity with a burned record at boot and shows a wrong-board screen on mismatch. A device that cannot handshake (recovery) is matched by the user picking the product when more than one could fit.

**Catalog.** `products/<id>/product.toml` (display name, image, boards, flash method per board, bundled firmware file with version, SHA-256 and minimum protocol, required capabilities) is compiled into the app; device code reads it instead of assuming one board, and features stay capability-driven, never `if product == …`. Firmware crates live under `firmware/<product>/` sharing `crates/`.

**Releases.** release-please turns conventional commits into one release PR with a version and changelog per component; the app tags `vX.Y.Z` and gets a normal GitHub Release whose title names the bundled firmware; firmware tags `fw-<product>-vX.Y.Z` get no GitHub Release, so "latest release" always means the app.

## Consequences

The catalog format leaves room for firmware as its own downloads later.

**As built (step 1).** `products/registry.toml` holds two tables (`[products]`, `[boards]`) of id to `u16`; 0 is reserved for unknown and never assigned. `product.toml` carries `display_name`, `image`, `required_capabilities` and a `[boards.<id>]` table with `flash`, `usb` (VID:PID hints) and `[boards.<id>.firmware]` (`crate`, `artifact`, `min_protocol`). The firmware version, SHA-256 and identity record are not in the catalog yet: the version stays read from the image's marker, and `build.rs` fails the build if an embedded image differs from the version of the catalog's firmware crate. `apps/desktop/src-tauri/src/catalog.rs` embeds and validates the files (also run by `build.rs`); discovery's USB allowlist and the provisioning helper's required capabilities now come from it.

**As built (step 2).** Identity on the wire is `Identity { session, product, board, hw_rev, source }` (tag 23, `PRODUCT_IDENTITY_V1`, bit 15) with `source` limited to `Build` and `Chip`: the desktop's `legacy` source is not a wire value. It rides protocol minor 6 with the rest of the 1.6 additions, so `PROTOCOL_MINOR` is unchanged. The chip record is 12 bytes (format 1, product, board, revision, two reserved zero bytes, CRC-32 over the first 8) and lives in `kivori_protocol::identity`; this step only reads it, from the C3 user eFuse block through `esp_hal::efuse`, and an invalid record is ignored like an empty one (source `Build`), so a damaged record never bricks a unit. The firmware's `build.rs` reads the numbers and the minimum protocol from `products/` with a small line scanner instead of a `toml` build dependency. The image record is `KIVORI-FW-IDENTITY:p=..;b=..;v=..;m=..`, and `build.rs` of the desktop also checks it against the catalog. A device whose product or board the catalog does not know is shown as unknown, gets no capabilities beyond identity, and is refused by the flashing guard rather than handled through the `Incompatible` connection state, which only the handshake can enter. Restore without a handshake asks the user to pick only when more than one catalog product lists the port's USB ids (`restore_firmware(product)`).

**As built (step 3).** The burn goes through `espefuse` 4.x (`burn_block_data BLOCK_USR_DATA`, which needs a file of the whole 32-byte block, so the 12-byte record is zero padded), is dry-run unless `--burn-identity` is given, and refuses any block that is not all zero. The burn happens before the flash (flashing restarts the unit, so the verification sees the burned record); the provisioning flash also applies the image-versus-device guard, checked with a handshake before anything is written.

## Alternatives considered

- Firmware as its own downloads now: rejected, a second signing chain and server, and code fetched from the network onto a device; the catalog format leaves room for it later.
- Lockstep versions: rejected.
- One version for everything: rejected, every firmware fix would bump every product.
