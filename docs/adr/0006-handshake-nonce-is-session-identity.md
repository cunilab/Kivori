# ADR-0006: The handshake nonce is the session identity

- **Status:** Accepted
- **Date:** 2026-09-30

## Context

A complete stale gesture pair buffered across a disconnect would otherwise pass the observed-`GestureStarted` check and change the volume.

## Decision

One OS-random `u32` per connection attempt stamps `InputEvent` and `Presentation`, and `revision` is session-scoped. `gesture_id` only needs to be unique inside a session.

## Alternatives considered

- A dedicated `SessionEpoch` message: rejected because the nonce already has the right lifecycle.
