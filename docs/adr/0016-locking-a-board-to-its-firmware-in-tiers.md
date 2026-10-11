# ADR-0016: Locking a board to its firmware, in tiers

- **Status:** Proposed
- **Date:** 2026-10-12
- **Related:** [ADR-0010](./0010-v1-controller-with-native-usb.md), [ADR-0014](./0014-versions-product-identity-and-product-catalog.md)

## Context

The reason for locking is safety, not copy protection: a device that can act as a USB keyboard could type into its host if malicious firmware ran on it.

## Decision

- **Tier A (beta):** the app's wrong-product guard and the firmware's wrong-board check (ADR-0014) stop accidents, all in software and reversible.
- **Tier B (v1 production units with native USB, ADR-0010):** secure boot, so the chip boots only firmware signed with Kivori's key (RP2350 secure boot; ESP32-C3 Secure Boot V2 if the C3 stays).
- **Tier C:** stopping copies on clone boards (flash encryption plus chip-held keys) is not planned.

Not applied to beta C3 units.

## Consequences

Tier B is irreversible per chip, so development boards stay unlocked and the signing key is kept offline in two places; losing it means locked units can never be updated.

## Alternatives considered

- Tier C, flash encryption plus chip-held keys: not planned, it costs most and protects least.
- Permanent fuses on beta C3 units: not applied, permanent fuses on units that cannot act as a keyboard buy little and slow every flash.
