# ADR-0002: One wire protocol crate

- **Status:** Accepted
- **Date:** 2026-08-04

## Context

Both peers share one schema. COBS resyncs after corruption where a length prefix cannot, and postcard is `no_std` with a stable format.

## Decision

One wire protocol crate: COBS framing, CRC-32, postcard payloads, an append-only `Message` enum, major-gated compatibility, minor and capability negotiation.

## Consequences

Reordering or removing anything is a major change.
