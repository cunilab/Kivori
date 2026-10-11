# ADR-0007: Interrupt-captured input edges

- **Status:** Accepted
- **Date:** 2026-10-02

## Context

The loop only returns to input between render passes, and composing plus flushing a frame takes longer than the quarter-steps of a detent, so polling once per tick loses detents and mistimes presses.

## Decision

The GPIO interrupt records a timestamped level snapshot on every edge of CLK, DT and SW into a bounded queue; decoding stays above the port.

## Alternatives considered

- Sampling between tile writes: rejected, still blind during composition.
- A timer-sampled ISR: rejected, fixed cost whether or not anything moves.
