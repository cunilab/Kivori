# Kivori dev command recipes — run `just <recipe>`. (Dev prerequisite: https://github.com/casey/just)
set shell := ["bash", "-uc"]

# Default: list recipes.
default:
    @just --list

# ---- host workspace (root Cargo workspace + Bun) ----

# Format, clippy, eslint, typecheck, prettier.
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    bun run lint
    bun run typecheck
    bun run format

# Host workspace + frontend tests.
test:
    cargo test --workspace
    bun run test

# Build the host workspace + frontend.
build:
    cargo build --workspace
    bun run build

# ---- firmware (isolated workspace; ADR-0001) ----
# no_std isolation proof: compile ONLY the shared crates for the RISC-V target from the firmware
# workspace. Nothing esp-hal here, so this stays fast and proves the firewall.

# Compile the shared crates for RISC-V (no_std / no-alloc isolation proof).
fw-check:
    cd firmware/esp32-c3 && cargo build -p kivori-model -p kivori-protocol -p kivori-framebuffer -p kivori-renderer -p kivori-assets

# The flashable binary needs `--features embedded` at minimum: the bin target declares it in
# `required-features`, so a plain `cargo build --release` silently builds only the library and
# produces NO binary. `embedded` alone only selects main.rs's bare fallback mode (clock + serial,
# no display, no input) — the shipped product firmware needs `physical-st7789` (which enables
# `embedded` transitively via Cargo.toml), so that is the default here.

# Build the flashable RISC-V firmware binary (the physical ST7789 + rotary product runtime).
fw-build:
    cd firmware/esp32-c3 && cargo build --release --features physical-st7789

# Host-side device-core tests (no hardware, no simulator).
fw-test:
    cd firmware/esp32-c3 && cargo test --features host-sim --target $(rustc -vV | sed -n 's/^host: //p')

# Flashes and monitors via the `espflash` runner in .cargo/config.toml.
# The physical ESP32-C3 + ST7789 profile is verified in docs/validation-checklist.md; simulator profiles
# remain separate evidence and are not substitutes for physical wiring/controller validation.
# `--features physical-st7789` is required to select that runtime; `embedded` alone builds only
# main.rs's bare fallback (no display, no input) and would silently flash non-product firmware.

# Flash + monitor the verified physical board profile (ST7789 + rotary product runtime).
fw-flash:
    cd firmware/esp32-c3 && cargo run --release --features physical-st7789

# DEVELOPMENT ONLY: physical profile plus the on-panel detent -> flush latency readout (Slice 002
# checklist row 14). Never a product build.
fw-flash-latency:
    cd firmware/esp32-c3 && cargo run --release --features physical-st7789,latency-probe

# ---- Wokwi pre-hardware simulation gate (sim/wokwi/README.md) ----

# Build all four simulation artifacts (self-test, external serial, SPI probe, production runtime).
sim-build:
    bash scripts/build-wokwi-firmware.sh

# Run every Wokwi scenario (needs wokwi-cli + a CI WOKWI_CLI_TOKEN; never false-passes).
sim-test: sim-build
    bash scripts/test-wokwi.sh

# Offline checks for the simulator's own tooling (no token, no simulator).
sim-check:
    cargo test -p kivori-wokwi-vectors -p kivori-wokwi-vcd
    bash scripts/test-wokwi-evidence.sh

# ---- assets / golden frames ----

# Recompile the canonical asset blob.
assets:
    cargo run -p kivori-asset-compiler

# Verify rendered frames against the committed golden hashes.
golden:
    cargo test -p kivori-golden-frames

# No automated bless exists: golden frames are committed deliberately, so a change is reviewed by hand.
golden-bless:
    @echo "No bless mode. Golden hashes live in tests/golden-frames/; a mismatch means the renderer moved."
    @echo "Investigate with 'just golden' first. Only update the committed frames if the change is intended."

# ---- architecture firewall (ADR-0001) ----

# Direct-dependency firewall over the shared no_std crates.
check-boundaries:
    bash scripts/check-crate-boundaries.sh

# ---- dev ----

# Run the desktop UI dev server.
dev:
    bun --filter kivori-desktop-ui dev
