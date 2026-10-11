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

## Alternatives considered

- Firmware as its own downloads now: rejected, a second signing chain and server, and code fetched from the network onto a device; the catalog format leaves room for it later.
- Lockstep versions: rejected.
- One version for everything: rejected, every firmware fix would bump every product.
