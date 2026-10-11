# ADR-0004: Layered scenes and a compiled asset blob

- **Status:** Accepted
- **Date:** 2026-08-04

## Context

Baked frames cost too much flash and fight tile rendering. Byte-reproducible assets keep host and device pixels equal.

## Decision

Scenes are layers with integer keyframes, drawn with `embedded-graphics`, and compiled from SVG into a deterministic blob. Blob format v2 adds alpha4 and the mascot roles, and allows Q8 interpolation for the mascot.
