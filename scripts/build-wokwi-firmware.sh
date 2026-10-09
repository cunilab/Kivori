#!/usr/bin/env bash
# Builds the four Wokwi firmware artifacts from the real esp-hal workspace.
#
#   kivori-selftest.elf  --features wokwi         internal on-target self-test (markers over UART0)
#   kivori-serial.elf    --features wokwi-serial  external serial test (real USB Serial/JTAG rx/tx loop)
#   kivori-spi.elf       --features wokwi-spi     GENERIC SPI/RGB565 tile-transfer probe (T072 adapter)
#   kivori-runtime.elf   --features wokwi-runtime PRODUCTION runtime loop (T074) with the sim-only profile
#
# All four are genuine RISC-V ELFs built from the same device core; only the harness around it differs.
# `kivori-runtime.elf` is special: it runs the REAL `runtime::run` loop the physical firmware calls, with
# only the board profile injected differently — there is no second runtime implementation.
# `kivori-spi.elf` drives the real T072 `mipidsi` DisplaySink; it validates generic bus/tile properties
# only and never a real panel controller (see sim/wokwi/README.md).
# Nothing here flashes or touches hardware.
set -euo pipefail

FW_DIR="firmware/esp32-c3"
TARGET="riscv32imc-unknown-none-elf"
BUILT="$FW_DIR/target/$TARGET/release/kivori-firmware"
STAGE="target/wokwi"
mkdir -p "$STAGE"

verify() {
  local elf="$1" label="$2"
  [ -f "$elf" ] || { echo "error: expected $label at $elf" >&2; exit 2; }
  local magic
  magic=$(head -c 4 "$elf" | od -An -tx1 | tr -d ' \n')
  [ "$magic" = "7f454c46" ] || { echo "error: $label is not an ELF (magic=$magic)" >&2; exit 1; }
  # Confirm the machine really is 32-bit little-endian RISC-V (e_machine = 0xF3 at offset 18).
  local machine class
  class=$(od -An -tx1 -j 4 -N 1 "$elf" | tr -d ' \n')
  machine=$(od -An -tx1 -j 18 -N 2 "$elf" | tr -d ' \n')
  [ "$class" = "01" ] || { echo "error: $label is not 32-bit (EI_CLASS=$class)" >&2; exit 1; }
  [ "$machine" = "f300" ] || { echo "error: $label is not RISC-V (e_machine=$machine)" >&2; exit 1; }
  echo "  ✓ $label: $(wc -c < "$elf" | tr -d ' ') bytes, 32-bit RISC-V ELF"
}

echo "Building internal self-test firmware (--features wokwi)…"
(cd "$FW_DIR" && cargo build --release --features wokwi)
cp "$BUILT" "$STAGE/kivori-selftest.elf"
verify "$STAGE/kivori-selftest.elf" "kivori-selftest.elf"

echo "Building external serial firmware (--features wokwi-serial)…"
(cd "$FW_DIR" && cargo build --release --features wokwi-serial)
cp "$BUILT" "$STAGE/kivori-serial.elf"
verify "$STAGE/kivori-serial.elf" "kivori-serial.elf"

echo "Building generic SPI display-probe firmware (--features wokwi-spi)…"
(cd "$FW_DIR" && cargo build --release --features wokwi-spi)
cp "$BUILT" "$STAGE/kivori-spi.elf"
verify "$STAGE/kivori-spi.elf" "kivori-spi.elf"

echo "Building PRODUCTION runtime firmware (--features wokwi-runtime)…"
(cd "$FW_DIR" && cargo build --release --features wokwi-runtime)
cp "$BUILT" "$STAGE/kivori-runtime.elf"
verify "$STAGE/kivori-runtime.elf" "kivori-runtime.elf"

echo "Firmware staged in $STAGE — run: bash scripts/test-wokwi.sh"
