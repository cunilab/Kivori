# ADR-0002: One wire protocol crate

- **Status:** Accepted
- **Date:** 2026-08-04
- **Amended:** 2026-10-15 (M2.8 additive wire-change discipline)

## Context

Both peers share one schema. COBS resyncs after corruption where a length prefix cannot, and postcard is `no_std` with a stable format.

## Decision

One wire protocol crate: COBS framing, CRC-32, postcard payloads, an append-only `Message` enum, major-gated compatibility, minor and capability negotiation.

Change discipline (M2.8): adding a post-handshake wire message or appending a variant to a wire
enum increments `PROTOCOL_MINOR` in the change that introduces it and declares its capability gate.
Both peers must negotiate support before the new message or variant is sent. The minor is a
descriptive wire revision shown in diagnostics, not a feature gate: an
unbumped minor does not make a capability invisible, and a minor bump alone does not enable it.
Capability intersection remains the feature-negotiation mechanism. Pre-negotiation handshake
messages (`Hello`, `HelloAck`, `Ready`) cannot rely on a negotiated capability to protect a layout
change; changing their fixed layouts requires a major change.
Changing an existing fixed message layout, or reordering/removing variants or fields, requires a
protocol major change. This rule applies to new work; earlier capability-only additions recorded
in ADR-0014 and architecture.md describe history, not an exception for future changes.

## Consequences

Reordering or removing anything is a major change.
