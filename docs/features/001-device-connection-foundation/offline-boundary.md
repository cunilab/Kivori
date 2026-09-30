# Offline runtime boundary

**Scope**: Feature 001 / current implemented foundation  
**Requirements**: FR-029, SC-006 | **Enforced by**: `scripts/check-offline-deps.sh`, `scripts/check-frontend-offline.mjs`, `apps/desktop/src-tauri/tests/offline_smoke.rs`

> This document describes the network boundary implemented by Feature 001. It is not a permanent prohibition on all future native networking. The current product PRD permits network-backed update discovery while requiring already-installed core functionality to remain offline-first. When update delivery is implemented, dependency guards should be narrowed deliberately so approved native update code can use the network without granting network access to firmware/shared crates/the webview or making normal startup depend on connectivity.

Kivori is **offline-first**. In the Feature 001 foundation, the desktop/device link is USB serial and core behavior does not depend on the internet, a cloud service, a CDN, telemetry, or a licence check.

## What must work with no network

Everything in the Feature 001 foundation. With the machine fully offline:

- Device discovery and handshake.
- Connection lifecycle, reconnect, and within-process desired-state restoration.
- Shared rendering, compiled assets, and Device Studio preview.
- State control and connection/typed-session-activity surfacing.
- App launch without waiting on network I/O.

## What is prohibited by the current Feature 001 implementation boundary

- **First-party network clients.** No current first-party crate may directly depend on an HTTP/socket client. Future update networking requires an explicit isolated native boundary rather than weakening the whole application.
- **Remote frontend assets.** The webview must not load remote scripts/styles/fonts/images or perform ordinary remote fetch/XHR/WebSocket requests. Development localhost is the exception.
- **Telemetry / analytics / phone-home**, and any startup network wait.
- Cloud-account or licence-server dependency for core operation.

Automatic update checks are not implemented by Feature 001. Future update discovery follows the product contract and [`../../research/technical-research.md`](../../research/technical-research.md).

## Enforcement

| Guard | Checks |
|---|---|
| `scripts/check-offline-deps.sh` | No current first-party host/firmware crate directly depends on a network-client crate. |
| `scripts/check-frontend-offline.mjs` | No remote URLs in the desktop frontend outside the local dev origin. |
| `apps/desktop/src-tauri/tests/offline_smoke.rs` | Native core + host-sim start and reach idle without external services or startup network wait. |

These run in host CI and do not require external services.
