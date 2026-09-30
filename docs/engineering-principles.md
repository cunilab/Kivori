# Kivori Engineering Principles

**Status:** Engineering baseline  
**Date:** 2026-09-16

## Purpose and authority

These principles govern how Kivori is engineered. They do **not** redefine the product. Product behavior and user guarantees are authoritative in [`product/prd.md`](./product/prd.md) and [`product/user-story-contract.md`](./product/user-story-contract.md).

When technical evidence changes, implementation details may change. Durable architecture choices belong in [`adr/`](./adr/); [`research/technical-research.md`](./research/technical-research.md) is explicitly non-normative and challengeable. Superpowers specs/plans describe individual pieces of work and must remain consistent with the product contract.

## 1. Observable truth over convenient assumptions

Kivori should represent and act on what the system can actually observe. Acknowledgement is not confirmation, unknown state must stay unknown, stale input must not be replayed, and implementation convenience must not manufacture certainty.

## 2. One canonical visual model

The desktop preview and physical device should consume the same scene semantics, assets, timing model, and renderer wherever practical. The frontend may provide inspection controls, but it must not become an independent implementation of device pixels.

## 3. Deterministic rendering

Given the same declared inputs, rendering should produce byte-identical logical RGB565 output across supported host and firmware targets. Rendering must not depend on wall-clock timing, unseeded randomness, uninitialized state, or platform-specific floating-point behavior.

Changes that intentionally alter canonical pixels should update deterministic/golden evidence in the same change.

## 4. Hardware constraints are first-class

ESP32-C3 RAM, flash, SPI bandwidth, power behavior, USB behavior, boot straps, and recovery limitations are design inputs, not late optimization concerns. Shared firmware-facing crates must preserve their required `no_std`/allocation constraints.

Hardware-dependent claims require hardware evidence. Simulation is valuable, but it must remain labelled as simulation.

## 5. Desktop owns desktop semantics; firmware owns hardware-critical truth

Desktop software should own OS integrations, profiles, bindings, action semantics, configuration, update coordination, and desktop-state reconciliation.

Firmware should own behavior that must remain valid without a healthy Desktop process: raw input conditioning, hardware gesture arbitration, local rendering, display/device health, host-lease failure behavior, MCU recovery, and low-level recovery/update state where applicable.

The boundary may evolve when evidence justifies it; platform- or service-specific business logic should not leak into firmware by convenience.

## 6. Semantic state crosses boundaries

Prefer semantic events/state over drawing commands or UI-specific instructions. Integrations and OS backends should publish facts; presentation logic decides how those facts are represented.

Normal device operation should communicate semantic presentation/input state rather than streaming desktop-rendered frames unless a future feature explicitly justifies a different transport.

## 7. Offline-first core

Already-installed core control, configuration, rendering, device communication, and recovery must remain usable without cloud availability.

Network-dependent capabilities such as update discovery must be isolated, explicit, and unable to make normal startup or core device operation depend on the network.

## 8. Least privilege and safe boundaries

The webview must receive only explicit, typed native capabilities. Raw serial, arbitrary filesystem/shell access, credentials, privileged device access, and unrestricted network access should not be exposed to React by default.

OS permissions should be requested only when a feature requires them. Missing permission is a product capability state, not justification for silent elevation or an unsafe fallback.

Diagnostics should carry allowlisted information and avoid raw payloads, secrets, unnecessary personal data, and stable hardware identity where it is not needed.

## 9. Cross-boundary behavior must be testable

Shared models, protocol framing, state machines, rendering, input semantics, action confirmation, and presentation reconciliation should be testable without a physical device where possible.

Platform-specific and hardware-specific behavior also needs real validation where mocks cannot prove the claim. A test must not claim evidence for a layer it did not exercise.

## 10. Prefer vertical working increments

Build the smallest complete path that proves a product behavior instead of creating large speculative infrastructure layers. Each implementation increment should leave the product buildable and its relevant behavior verifiable.

Extract crates/services only when dependency isolation, reuse, testing, security, or runtime ownership makes the boundary useful.

## 11. Record durable architecture decisions

Hard-to-reverse or cross-cutting decisions should be recorded as ADRs with context, decision, alternatives, consequences, and conditions for reconsideration.

Research is not an ADR. A recommendation becomes an accepted architecture decision only after sufficient evidence and an explicit decision.

## 12. Evidence beats prior recommendations

Research, designs, plans, and previous architecture choices are challengeable. If platform documentation, hardware measurement, prototype results, security findings, maintainability evidence, or tests demonstrate that a different implementation better satisfies the product contract, change the implementation and update the relevant documentation.

Do not preserve an approach merely because it was written first.

## Workflow

New substantial engineering work should use the Superpowers workflow reflected in [`superpowers/`](./superpowers/): brainstorm/research, approve a design, write an implementation plan, implement with tests, verify before claiming completion, and request/review code changes with evidence.

Feature-specific records live under [`features/`](./features/). They preserve requirements, architecture, contracts, evidence, and closure state for an implemented slice, but they do not override the current product contract or accepted ADRs.
