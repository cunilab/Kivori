# Kivori architecture

A built-system overview of Feature 001, Device Connection Foundation: the desktop app, the ESP32-C3 firmware, and the shared crates they both build on. Design rationale lives in the [ADRs](../../adr/); Feature 001 requirements live in [`requirements.md`](./requirements.md).

This document describes the implemented Feature 001 foundation. It is not the future product-wide architecture contract; current cross-platform implementation research lives in [`../../research/technical-research.md`](../../research/technical-research.md).

## Two workspaces, one shared core (ADR-0001)

```text
kivori/
├── crates/                     # shared, no_std, no-alloc — the single source of truth
│   ├── kivori-model            # state axes, device profile, timeline, version/caps, geometry/color, scene schema
│   ├── kivori-protocol         # COBS + header + CRC-32 + postcard wire codec, handshake, negotiation, seq policy
│   ├── kivori-framebuffer      # RGB565 tile bands + content hashing (change detection)
│   ├── kivori-renderer         # deterministic scene compositor (embedded-graphics DrawTarget)
│   └── kivori-assets           # zero-copy runtime reader for the compiled asset blob
├── apps/desktop/
│   ├── src-tauri/              # kivori-desktop: native core (root Cargo workspace member)
│   └── src/                    # React frontend
├── tools/asset-compiler/       # SVG → RGB565 blob host build tool
├── tests/golden-frames/        # cross-OS frame-hash determinism harness
└── firmware/esp32-c3/          # separate Cargo workspace (feature-isolation firewall)
```

Two Cargo workspaces are deliberate: the firmware is `no_std`/no-alloc for `riscv32imc`, and keeping it out of the host workspace prevents Cargo feature unification from pulling std-only features into the device build. Shared crates are consumed by both the desktop and firmware via path dependencies, so the renderer that draws Device Studio preview is the renderer that drives the panel. A direct-dependency firewall ([`scripts/check-crate-boundaries.sh`](../../../scripts/check-crate-boundaries.sh)) fails CI if a shared crate gains a std/host/OS dependency; the RISC-V compile is the hard backstop.

## Three Feature 001 state axes

- **Companion** (6): `booting, idle, happy, busy, sleeping, offline` — what the device shows.
- **Sendable** (4): `idle, happy, busy, sleeping` — the subset the desktop may command. `booting` and `offline` are device-originated.
- **Connection** (5): `disconnected, connecting, connected, incompatible, error` — the desktop↔device link lifecycle.

These are Feature 001 state models, not a claim that all future product state belongs in these enums.

## Data flows

**Wire (desktop ↔ device, USB serial — ADR-0002).** Frames are `COBS(magic ‖ ver ‖ seq ‖ len ‖ postcard-payload ‖ CRC-32) ‖ 0x00`. `MAX_PAYLOAD = 512`. Compatibility is major-version gated; minor + capabilities are negotiated. Messages currently include `Hello/HelloAck/Ready/Bye`, `SetState/StateReport`, `Ping/Pong`, `Health/Diagnostic/Error`.

**IPC (native core ↔ webview — [`contracts/ipc.md`](./contracts/ipc.md)).** The webview may call only a fixed set of typed commands and subscribe to status/diagnostic channels. No raw serial, filesystem, shell, or credential surface is exposed. The canvas is a pure blit target; canonical rendering remains native/shared.

**Asset pipeline (ADR-0004, build-time).** `kivori-asset-compiler` rasterizes layered SVGs to deterministic RGB565 and emits a compiled asset blob. Desktop and firmware read the same runtime representation.

**Render path.** `render_scene(blob, scene, elapsed_ms, tile_band)` composites a scene into an RGB565 tile using integer-only timing. Firmware renders tile-by-tile and flushes changed tiles; host-side tests prove stitched tiles match canonical full-frame output.

## Runtime shape

- **Firmware** ([`firmware/esp32-c3/src`](../../../firmware/esp32-c3/src)): a `no_std` core written against hardware-neutral transport/display/clock ports plus the physical ESP32-C3 runtime and host simulation adapters.
- **Desktop core** ([`apps/desktop/src-tauri/src`](../../../apps/desktop/src-tauri/src)): discovery, handshake verification, connection/reconnect state, session handling, typed session activity, window lifecycle, and preview rendering.
- **Frontend** ([`apps/desktop/src`](../../../apps/desktop/src)): React UI consuming typed IPC and native-rendered preview data.

## Cross-cutting guarantees

- **Deterministic rendering:** integer-based, golden-frame tested, and shared across host/device paths.
- **Offline-first Feature 001 boundary:** see [`offline-boundary.md`](./offline-boundary.md).
- **Least privilege + privacy:** ADR-0005 and [`activity-log.md`](../../activity-log.md) define the typed session activity boundary (the earlier `diagnostics` module and `diagnostics-and-logging.md` were replaced by the activity log in PR #3).
