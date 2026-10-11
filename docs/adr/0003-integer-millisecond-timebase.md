# ADR-0003: Integer-millisecond timebase

- **Status:** Accepted
- **Date:** 2026-08-04

## Context

Accumulating 33.33 ms as a float or 33 ms as an integer drifts and differs across platforms.

## Decision

Time is `u32` ms, and Device Studio derives it from a step index with `(n*1000+15)/30`.

## Consequences

Any frame is directly addressable.
