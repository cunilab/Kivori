# ADR-0001: Two cargo workspaces

- **Status:** Accepted
- **Date:** 2026-08-04

## Context

One cargo workspace unifies features, so a std feature enabled by the desktop would silently break the firmware `no_std` build. The firmware also needs its own target, panic strategy and runner.

## Decision

The root workspace holds the host crates, the desktop, tools and tests. `firmware/esp32-c3` has its own `[workspace]` and reaches shared crates by path.
