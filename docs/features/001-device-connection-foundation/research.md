# Research: Device Connection Foundation

**Feature:** `001-device-connection-foundation`  
**Original research date:** 2026-07-17  
**Reconciled:** 2026-09-16

**Purpose:** Preserve the technical research that informed Feature 001 while clearly separating original recommendations from what the repository ultimately implemented. Each item uses **Decision · Rationale · Alternatives · Caveats**. Current implementation truth is described in [`architecture.md`](./architecture.md), while unresolved physical evidence is tracked in [`validation-checklist.md`](./validation-checklist.md).

Research is evidence, not permanent architecture law. Where implementation diverged from an early recommendation, the implemented result and current tests take precedence for Feature 001.

---

## R-1. Device transport: ESP32-C3 native USB Serial/JTAG

**Decision:** Use the ESP32-C3 built-in USB Serial/JTAG controller as the host/device transport, without an external USB-UART bridge. Filter the initial physical profile by Espressif USB VID:PID `0x303A:0x1001` and verify the Kivori handshake before treating a candidate as a Kivori device.

**Rationale:** One cable supports power, flashing/debugging, and product serial traffic. It fits the Feature 001 USB-serial requirement with minimal hardware.

**Alternatives:** External CP210x/CH34x-style bridge. This remains possible for future hardware but adds BOM and changes discovery identity.

**Caveats:** USB Serial/JTAG has a small FIFO and transmit can stall when the host is not draining. Sustained-throughput and disconnect/stall recovery remain physical validation items.

## R-2. Firmware HAL: esp-hal, bare-metal `no_std`

**Decision:** Use `esp-hal` for the ESP32-C3 firmware and keep the firmware/runtime `no_std`. Pin embedded dependencies deliberately rather than treating HAL upgrades as routine dependency bumps.

**Rationale:** The product needs GPIO/SPI/USB/time primitives without a full OS/runtime. The separate firmware workspace keeps embedded feature selection isolated from host crates.

**Alternatives:** `esp-idf-hal`/ESP-IDF. Rejected for Feature 001 because its heavier runtime was unnecessary for the connection/display foundation.

**Caveats:** USB Serial/JTAG APIs have historically carried weaker stability guarantees than core HAL APIs. Physical behavior must be revalidated when the embedded stack materially changes.

## R-3. Display driver: MIPI-DCS/ST7789 with windowed writes

**Decision:** Use `mipidsi` and windowed/partial writes rather than requiring a full framebuffer. The physical panel is now verified as **ST7789, 240x240 RGB565**.

**Rationale:** Partial updates fit the shared tile renderer and reduce memory pressure while keeping the renderer controller-agnostic.

**Alternatives:** Controller-specific display crates or a permanent full framebuffer.

**Caveats:** The original research treated GC9A01 vs ST7789 as unknown. Physical validation on 2026-08-11 resolved the production profile to ST7789 with offset `(0,0)`, 90° rotation, RGB order, inversion enabled, SPI2 at 20 MHz Mode 3, SCK GPIO6, MOSI GPIO7, D/C GPIO2, reset GPIO3, and backlight GPIO8 active-high.

## R-4. ESP32-C3 memory and framebuffer strategy

**Decision:** Use tile/band rendering and dirty-region/change detection rather than making a full 240x240 RGB565 framebuffer mandatory.

**Rationale:** A full frame is 115,200 bytes. Tiling keeps memory predictable, works naturally with partial display writes, and leaves headroom for future firmware behavior.

**Alternatives:** A single full framebuffer can be reconsidered if later firmware measurements show it materially simplifies animation without compromising memory headroom.

**Caveats:** Actual RAM headroom is a property of the linked firmware build, not a datasheet estimate.

## R-5. Desktop serial: implementation settled on blocking `serialport` ownership in a dedicated device thread

**Decision:** **Current implementation:** use the blocking `serialport` crate behind the Desktop's dedicated background device thread/actor rather than the original async-first `tokio-serial` recommendation.

**Rationale:** The dedicated thread keeps blocking serial I/O away from the React/webview and Tauri GUI lifecycle while avoiding unnecessary async serial complexity. The resulting implementation has working host tests and Windows startup coverage.

**Alternatives:** `tokio-serial` remains a viable future option if a demonstrated concurrency/scalability problem justifies it.

**Caveats:** The original July 2026 research recommended `tokio-serial` with a blocking fallback. Feature 001 is an example of why research is challengeable: implementation evidence supported the simpler blocking path. Device removal is still detected through I/O/session/discovery behavior rather than relying on port names as identity.

## R-6. Protocol payloads: serde + postcard in one shared protocol crate

**Decision:** Keep payload types in the shared `kivori-protocol` crate using `serde`/`postcard`, with fixed-size/no-alloc-friendly operation on firmware.

**Rationale:** Both peers compile against the same message definitions, reducing schema drift and supporting `no_std` firmware.

**Alternatives:** Hand-written binary layouts, JSON, protobuf, or a larger RPC framework.

**Caveats:** Postcard enums are not field-tagged/self-describing. Current protocol evolution therefore treats message-enum ordering as wire-significant and uses explicit protocol version/capability negotiation. See [ADR-0002](../../adr/0002-wire-protocol.md).

## R-7. Tauri v2 binary preview transport

**Decision:** Keep canonical rendering in native/shared Rust and send rendered preview bytes through typed Tauri IPC/channel surfaces; the canvas remains a blit/inspection surface rather than a second renderer.

**Rationale:** This preserves visual parity and avoids JSON/base64 overhead for frame data.

**Alternatives:** Re-render scenes in TypeScript or use JSON/base64 frame payloads.

**Caveats:** IPC/webview copying and RGB conversion remain measurable costs. Device Studio performance should be tuned from evidence rather than assumed unlimited.

## R-8. Drift-free integer timing

**Decision:** Canonical rendering time is integer `elapsed_ms`; inspection stepping derives elapsed time from an integer step index rather than repeatedly adding a floating-point frame duration.

**Rationale:** This keeps rendering deterministic and avoids accumulated floating-point drift.

**Alternatives:** Accumulated floating-point/microsecond deltas.

**Caveats:** Timing policy is an engineering model, not a requirement that every future animation run at one fixed FPS. See [ADR-0003](../../adr/0003-timing-model.md).

## R-9. Deterministic asset compilation

**Decision:** Rasterize source assets at build time into the canonical runtime representation and verify deterministic output through hashes/golden evidence.

**Rationale:** Firmware should not parse/render SVG at runtime, and host/device should consume the same compiled visual data.

**Alternatives:** Runtime SVG parsing or independent host/device asset pipelines.

**Caveats:** Rasterizer/toolchain upgrades may intentionally change bytes and therefore require explicit golden review. See [ADR-0004](../../adr/0004-asset-format.md).

## R-10. Firmware workspace isolation

**Decision:** Keep `firmware/esp32-c3` in a separate Cargo workspace while sharing path dependencies into the `crates/` foundation.

**Rationale:** This prevents host/std feature unification from silently changing firmware builds and lets the embedded target own its toolchain/runner configuration.

**Alternatives:** One root workspace for all targets.

**Caveats:** Two workspaces add some command/dependency-management overhead. The isolation is intentional and enforced through compile/dependency checks. See [ADR-0001](../../adr/0001-two-workspace-cargo-split.md).

---

## Hardware-validation-required

The following claims cannot be established by code review, host simulation, or Wokwi alone and remain in [`validation-checklist.md`](./validation-checklist.md):

1. sustained native USB Serial/JTAG throughput and transmit-stall recovery;
2. physical disconnect/reconnect behavior and rapid-cycle stability;
3. sustainable physical ST7789 frame/update rate;
4. controlled plug-in, state-change, and reconnect latency measurements;
5. physical Device Studio/renderer parity checks;
6. pinned `esp-hal` USB Serial/JTAG behavior on the actual ESP32-C3 hardware.

The physical panel identity, pin map, geometry, orientation, color order, and inversion settings are no longer research unknowns; they were verified on 2026-08-11 and are recorded in [`validation-checklist.md`](./validation-checklist.md).

## Sources

The original research used Espressif/esp-hal documentation, ESP-IDF USB Serial/JTAG and memory guidance, `mipidsi`, `serialport`/`tokio-serial`, `postcard`, and Tauri v2 IPC documentation. Current repository behavior and physical evidence are the authority for the implemented Feature 001 baseline where they differ from the original recommendation.
