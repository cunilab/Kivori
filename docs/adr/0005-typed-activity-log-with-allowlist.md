# ADR-0005: Typed activity log with an allowlist

- **Status:** Accepted
- **Date:** 2026-08-04

## Context

Payloads, identity, paths and tool output are sensitive.

## Decision

Closed types, fixed metadata, hashed device id, raw payloads compiled out, session-only history.

## Consequences

Support loses raw-byte detail in release builds on purpose.
