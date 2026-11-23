# ADR-0014: Versions, product identity and the product catalog

- **Status:** Proposed
- **Note:** Superseded in part by [ADR-0019](./0019-rp2040-zero-replaces-esp32-c3.md) (#179): the C3 eFuse record and its provisioning tool were removed; identity on the RP2040-Zero comes from the build.
- **Date:** 2026-10-12
- **Related:** [ADR-0002](./0002-one-wire-protocol-crate.md), [ADR-0015](./0015-app-updates-through-github-releases.md), [ADR-0016](./0016-locking-a-board-to-its-firmware-in-tiers.md)

## Decision

The desktop app, firmware, wire protocol and configuration schema have independent versions.
Compatibility is negotiated through the wire protocol and capabilities; firmware images are
selected by product and board, not by an assumed pairing with the app version.

`products/registry.toml` assigns product and board wire numbers; `products/pip/product.toml`
contains display names, supported boards, capabilities and firmware build metadata. USB identifiers
nominate ports and do not establish device identity. The firmware reports its product, board,
revision and identity source after the handshake. The desktop rejects unknown identities for flashing
and selects only the matching firmware image. RP2040-Zero firmware reports its build identity; the
recognized legacy C3 board has no bundled firmware.

Each firmware image carries a version and identity marker. The build checks the image against the
catalog, and the desktop checks the target before flashing. Recovery uses the available product and
board candidates and asks the user to choose when necessary. See [architecture](../architecture.md#product-identity-product_identity_v1-adr-0014)
and [release policy](../release.md#release-policy) for the current implementation and release rules.

## Consequences

Product and board wire numbers remain stable. The catalog keeps board-specific build and flashing
information outside shared product behavior. Independent releases allow a firmware version to remain
unchanged when its build inputs have not changed.

## Alternatives considered

- Lockstep app and firmware versions: rejected because devices can run older compatible firmware.
- Separate firmware downloads: deferred; the catalog leaves room for them later.
