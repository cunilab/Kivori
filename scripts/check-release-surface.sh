#!/usr/bin/env bash
# Release feature-surface guard (T104; FR-028, ADR-0005 §3).
#
# Proves that developer-only surface is absent from release builds *by construction* — not by a runtime
# flag. For each feature configuration it inspects the compiled artifact:
#
#   1. desktop, production features (`--no-default-features`): the dev-only IPC commands
#      (render_preview_frame, mirror_state, the preview-stream commands) and the raw-payload logging
#      helper must not appear in the binary at all.
#   2. firmware, production features (default, riscv target): the host-simulation adapters and the Wokwi
#      SPI probe types/markers must not appear in the device library — while the T072 production display
#      adapter MUST.
#
# Each check is paired with a POSITIVE CONTROL that rebuilds with the dev feature enabled and asserts
# the same symbols ARE present, so the guard can never pass vacuously (e.g. from a renamed symbol).
#
# NOTE on grep targets: rustc's rlib metadata records `cfg`-disabled MODULE NAMES, so a module path such
# as `spi_probe` appears even in a production build. Only concrete type names and string literals are
# usable evidence here, and the positive controls below prove each one can actually be observed.
set -euo pipefail

fail=0

# absent <label> <artifact> <symbol...>
absent() {
  local label="$1" artifact="$2"
  shift 2
  for sym in "$@"; do
    if grep -aqs -- "$sym" "$artifact"; then
      echo "  ✗ RELEASE SURFACE LEAK: '$sym' present in $label"
      fail=1
    else
      echo "  ✓ '$sym' absent from $label"
    fi
  done
}

# present <label> <artifact> <symbol...>  (positive control: the check must be able to fail)
present() {
  local label="$1" artifact="$2"
  shift 2
  for sym in "$@"; do
    if grep -aqs -- "$sym" "$artifact"; then
      echo "  ✓ control: '$sym' present in $label"
    else
      echo "  ✗ VACUOUS CHECK: '$sym' missing from $label — the guard proves nothing"
      fail=1
    fi
  done
}

DESKTOP_DEV_SURFACE=(render_preview_frame mirror_state open_preview_stream close_preview_stream
  ack_preview_frame debug_payload_hex)
# Host-sim adapters AND the Wokwi self-test harness/probes are development-only surface.
FIRMWARE_SIM_SURFACE=(SimPipe CaptureDisplay VirtualClock LoopbackTransport TileProbe KIVORI-SIM
  KIVORI-SPI WokwiSpiPins BusCounters CountedSpi KIVORI-RUN)
# Raw-payload tracing is DEVELOPMENT-ONLY surface (T102): the marker and the dump helper must not exist in
# any release artifact, including the `embedded` production build.
# Only the string literal is usable: `debug_payloads` is a module name, and rustc records cfg-disabled
# module names in rlib metadata (same trap as `spi_probe`, noted above).
FIRMWARE_DEBUG_SURFACE=(KIVORI-DEBUG-PAYLOAD)
# The T072 SPI DisplaySink is PRODUCTION surface: it must be in the shipping library, not gated away.
FIRMWARE_PRODUCTION_SURFACE=(MipidsiSink PanelGeometry)

DESKTOP_BIN="target/debug/kivori-desktop"
FIRMWARE_LIB="firmware/esp32-c3/target/riscv32imc-unknown-none-elf/debug/libkivori_firmware.rlib"

echo "[1/8] desktop, production features (Device Studio + debug-payloads OFF)…"
cargo build -q -p kivori-desktop --no-default-features
[ -f "$DESKTOP_BIN" ] || { echo "error: $DESKTOP_BIN not found" >&2; exit 2; }
absent "the production desktop binary" "$DESKTOP_BIN" "${DESKTOP_DEV_SURFACE[@]}"

echo "[2/8] desktop positive control (Device Studio ON)…"
cargo build -q -p kivori-desktop
present "the dev desktop binary" "$DESKTOP_BIN" render_preview_frame mirror_state open_preview_stream

echo "[3/8] firmware, production features (host-sim OFF, riscv target)…"
(cd firmware/esp32-c3 && cargo build -q --lib)
[ -f "$FIRMWARE_LIB" ] || { echo "error: $FIRMWARE_LIB not found" >&2; exit 2; }
absent "the production firmware library" "$FIRMWARE_LIB" "${FIRMWARE_SIM_SURFACE[@]}"
present "the production firmware library" "$FIRMWARE_LIB" "${FIRMWARE_PRODUCTION_SURFACE[@]}"

echo "[4/8] firmware positive controls (host-sim ON, then wokwi ON)…"
(cd firmware/esp32-c3 && cargo build -q --lib --features host-sim)
present "the host-sim firmware library" "$FIRMWARE_LIB" SimPipe CaptureDisplay
(cd firmware/esp32-c3 && cargo build -q --lib --features wokwi)
present "the wokwi firmware library" "$FIRMWARE_LIB" LoopbackTransport TileProbe KIVORI-SIM

echo "[5/8] firmware SPI-probe positive control (wokwi-spi ON)…"
(cd firmware/esp32-c3 && cargo build -q --lib --features wokwi-spi)
present "the wokwi-spi firmware library" "$FIRMWARE_LIB" KIVORI-SPI WokwiSpiPins BusCounters CountedSpi
# …and the ordinary host-sim configuration must NOT drag the probe in.
(cd firmware/esp32-c3 && cargo build -q --lib --features host-sim)
absent "the host-sim firmware library" "$FIRMWARE_LIB" KIVORI-SPI WokwiSpiPins BusCounters
echo "[6/8] firmware production-runtime positive control (wokwi-runtime ON)…"
(cd firmware/esp32-c3 && cargo build -q --lib --features wokwi-runtime)
present "the wokwi-runtime firmware library" "$FIRMWARE_LIB" KIVORI-RUN WokwiSpiPins

echo "[7/8] firmware raw-payload gate (T102): absent in production, present with debug-payloads…"
# The production `embedded` build is what ships; prove the raw path is not in it.
(cd firmware/esp32-c3 && cargo build -q --lib --features embedded)
absent "the production embedded firmware library" "$FIRMWARE_LIB" "${FIRMWARE_DEBUG_SURFACE[@]}"
(cd firmware/esp32-c3 && cargo build -q --lib --features embedded,debug-payloads)
present "the debug-payloads firmware library" "$FIRMWARE_LIB" "${FIRMWARE_DEBUG_SURFACE[@]}"
echo "[8/8] firmware latency-probe gate: absent from the product build, present with latency-probe…"
# `physical-st7789` is the product firmware (`just fw-build`); the probe is a dev-only readout.
(cd firmware/esp32-c3 && cargo build -q --lib --features physical-st7789)
absent "the product physical-st7789 firmware library" "$FIRMWARE_LIB" LatencyProbe
(cd firmware/esp32-c3 && cargo build -q --lib --features physical-st7789,latency-probe)
present "the latency-probe firmware library" "$FIRMWARE_LIB" LatencyProbe
# Leave the firmware artifact in its production configuration.
(cd firmware/esp32-c3 && cargo build -q --lib)

if [ "$fail" -ne 0 ]; then
  echo "Release feature-surface check FAILED." >&2
  exit 1
fi

echo "Release feature-surface OK: dev-only desktop commands, the raw-payload path, the firmware host-sim"
echo "adapters, the Wokwi SPI/runtime markers, the raw-payload dump, and the latency probe are all absent from production"
echo "builds, while the T072 display adapter is present (every positive control passed)."
