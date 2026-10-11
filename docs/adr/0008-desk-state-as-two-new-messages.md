# ADR-0008: Desk state as two new messages

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

`Presentation` is a fixed wire struct (adding fields is a major change) whose job is the rotary overlay, and persistent truth and transient outcomes expire differently.

## Decision

Display mode plus monitored values travel as `Status` (tag 15, the whole snapshot on every change) and action outcomes as `Feedback` (tag 16, transient, newest wins), each behind its own capability.

## Alternatives considered

- A per-field delta protocol: rejected, more states to get wrong for a payload under 64 bytes.
