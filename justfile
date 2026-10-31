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
# fw-check/test/flash/flash-latency are explicit board-specific development recipes.
# no_std isolation proof: compile ONLY the shared crates for the RISC-V target from the firmware
# workspace. Nothing esp-hal here, so this stays fast and proves the firewall.

# Compile the shared crates for RISC-V (no_std / no-alloc isolation proof).
fw-check:
    cd firmware/pip && cargo build -p kivori-model -p kivori-protocol -p kivori-framebuffer -p kivori-renderer -p kivori-assets

# The flashable binary needs `--features embedded` at minimum: the bin target declares it in
# `required-features`, so a plain `cargo build --release` silently builds only the library and
# produces NO binary. `embedded` alone only selects main.rs's bare fallback mode (clock + serial,
# no display, no input) — the shipped product firmware needs `physical-st7789` (which enables
# `embedded` transitively via Cargo.toml). The selected catalog entry owns the build features.

# Build the catalog firmware (sole pair, or KIVORI_PRODUCT + KIVORI_BOARD). Never flashes.
fw-build:
    bun scripts/build-firmware.mjs

# Host-side device-core tests (no hardware).
fw-test:
    cd firmware/pip && cargo test --features host-sim --target $(rustc -vV | sed -n 's/^host: //p')

# Flashes and monitors via the `espflash` runner in .cargo/config.toml.
# The physical ESP32-C3 + ST7789 profile is verified in docs/validation.md.
# `--features physical-st7789` is required to select that runtime; `embedded` alone builds only
# main.rs's bare fallback (no display, no input) and would silently flash non-product firmware.

# Flash + monitor the verified physical board profile (ST7789 + rotary product runtime).
fw-flash:
    cd firmware/pip && cargo run --release --features physical-st7789

# DEVELOPMENT ONLY: physical profile plus the on-panel detent -> flush latency readout (Slice 002
# checklist row 14). Never a product build.
fw-flash-latency:
    cd firmware/pip && cargo run --release --features physical-st7789,latency-probe

# Hardware-free resource check of the catalog firmware: image size, RAM, stack (per-function frames
# and the worst call chain) and the 64-bit division allowlist, against the crate's resource-budget.json.
# Needs llvm-objdump (rustup component add llvm-tools, or LLVM_OBJDUMP=...).
fw-budget:
    bun scripts/build-firmware.mjs > /dev/null
    bun scripts/firmware-budget.mjs
    bun scripts/virtual-resource-facts.mjs --check

# Refresh the resource facts the virtual device reports (simulator/fixtures/resource-facts.json)
# from the freshly built product ELF; commit the result. `just fw-budget` fails when they drift.
fw-virtual-facts-update:
    bun scripts/build-firmware.mjs > /dev/null
    bun scripts/virtual-resource-facts.mjs --write

# Predicted ESP32-C3 compose time per scenario and tile row, without hardware: the real renderer compiled
# for riscv32imc and run in an instruction-set simulator (the firmware crate's frame-cost). Fails if a
# scenario got more than 10% slower than the committed fixture. Needs the riscv32imc target.
fw-frame-cost:
    firmware/pip/frame-cost/run.sh check

# Rewrite frame-cost/fixtures/frame-cost.json after an intended renderer cost change.
fw-frame-cost-update:
    firmware/pip/frame-cost/run.sh update

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

# Product-contract identifiers stay out of shared app/tooling (ADR-0017).
check-identifiers:
    bun scripts/check-product-identifiers.mjs
    bun test scripts/check-product-identifiers.test.mjs

# ---- release bundles (M3 S5; docs/release.md) ----

# Build the unsigned Windows NSIS installer (run on Windows; needs PowerShell, bun and Rust).
bundle-windows:
    #!/usr/bin/env bash
    set -euo pipefail
    firmware_dir="$(mktemp -d)"
    trap 'rm -rf "$firmware_dir"' EXIT
    unset KIVORI_FIRMWARE_PATH
    manifest="$(bun scripts/build-firmware-set.mjs --out-dir "$firmware_dir/images" --manifest-out "$firmware_dir/manifest.json")"
    [ -s "$manifest" ] || { echo "Missing firmware manifest" >&2; exit 1; }
    powershell -NoProfile -File scripts/fetch-espflash.ps1
    bun install --frozen-lockfile
    bun --filter kivori-desktop-ui build
    cd apps/desktop
    args=(--config src-tauri/tauri.release.conf.json)
    if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
        args+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
    fi
    KIVORI_FIRMWARE_MANIFEST="$manifest" bunx tauri build "${args[@]}" -- --no-default-features

# Build the macOS .app + .dmg (run on a Mac). Signed with APPLE_SIGNING_IDENTITY, else the stable
# KIVORI_MACOS_SIGNING_IDENTITY / "Kivori Dev" identity (see macos-dev-identity), else ad-hoc.
bundle-macos:
    bash scripts/bundle-macos.sh

# Create the self-signed "Kivori Dev" signing identity in your login keychain (once; safe to rerun),
# so local bundles keep their Accessibility approval across rebuilds (docs/macos.md).
macos-dev-identity:
    bash scripts/create-macos-dev-identity.sh

# Native Mac checks without changing audio or launching the media observer / permission prompts.
macos-check:
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(uname -s)" = Darwin ] || { echo "Run macos-check on a Mac." >&2; exit 1; }
    cargo test --locked -p kivori-desktop --lib platform::macos
    cargo test --locked -p kivori-desktop --test macos_platform -- --skip media_observer_reads_without_blocking_and_stops_promptly
    cargo test --locked -p kivori-desktop --test volume_backend_contract

# Opt-in: changes output volume/mute, verifies observation, then restores the original values.
macos-audio-test:
    cargo test --locked -p kivori-desktop --test macos_platform write_round_trips_and_classifies_echo_versus_external -- --ignored --exact --test-threads=1

# ---- dev ----

# Run the desktop UI dev server.
dev:
    bun --filter kivori-desktop-ui dev

# A plain `cargo run` bundles no firmware on purpose (no stale artifact can slip in), which logs
# `firmwareUnavailable` and disables Flash firmware. Start `just dev` first.

# Run Kivori Desktop in one command: UI dev server + native app with the bundled firmware.
desktop:
    #!/usr/bin/env bash
    set -euo pipefail
    if curl -sf -o /dev/null http://localhost:1420; then
        echo "Port 1420 is already in use (another UI dev server?). Stop it, then retry." >&2
        exit 1
    fi
    firmware_dir="$(mktemp -d)"
    trap 'rm -rf "$firmware_dir"' EXIT
    unset KIVORI_FIRMWARE_PATH
    manifest="$(bun scripts/build-firmware-set.mjs --out-dir "$firmware_dir/images" --manifest-out "$firmware_dir/manifest.json")"
    [ -s "$manifest" ] || { echo "Missing firmware manifest" >&2; exit 1; }
    set -m  # the UI server gets its own process group, so Ctrl-C / exit stops all of it
    bun --filter kivori-desktop-ui dev &
    ui=$!
    trap 'kill -- -"$ui" 2>/dev/null || true; rm -rf "$firmware_dir"' EXIT
    echo "Waiting for the UI dev server on :1420..."
    until curl -sf -o /dev/null http://localhost:1420; do
        kill -0 "$ui" 2>/dev/null || { echo "The UI dev server exited." >&2; exit 1; }
        sleep 0.3
    done
    KIVORI_FIRMWARE_MANIFEST="$manifest" cargo run -p kivori-desktop

# Run Kivori Desktop with freshly built firmware for every catalog pair embedded.
app:
    #!/usr/bin/env bash
    set -euo pipefail
    firmware_dir="$(mktemp -d)"
    trap 'rm -rf "$firmware_dir"' EXIT
    unset KIVORI_FIRMWARE_PATH
    manifest="$(bun scripts/build-firmware-set.mjs --out-dir "$firmware_dir/images" --manifest-out "$firmware_dir/manifest.json")"
    [ -s "$manifest" ] || { echo "Missing firmware manifest" >&2; exit 1; }
    KIVORI_FIRMWARE_MANIFEST="$manifest" cargo run -p kivori-desktop

# DEV ONLY (M3 S7; docs/provisioning.md): flash the freshly built firmware onto the ONE plugged-in
# unit, check the link and controls, print PASS/FAIL and append to provisioning-log.csv.

# Provision and QA one Kivori unit (builds the firmware first). Optional identity record:
# `just provision --product <id> --board <id> --rev 1 [--burn-identity]` (docs/provisioning.md).
provision *args:
    #!/usr/bin/env bash
    set -euo pipefail
    firmware="$(bun scripts/build-firmware.mjs)"
    unset KIVORI_FIRMWARE_MANIFEST
    KIVORI_FIRMWARE_PATH="$firmware" cargo run -p kivori-desktop --example provision -- {{args}}

# ---- website (apps/web; Next.js on Cloudflare Workers via OpenNext) ----

# Run the website dev server on :3000.
web-dev:
    bun --filter kivori-web dev

# Build and run the site in the local Workers runtime (workerd).
web-preview:
    bun --filter kivori-web preview

# Build and deploy the site to Cloudflare (needs `bunx wrangler login` or CLOUDFLARE_API_TOKEN).
web-deploy:
    bun --filter kivori-web deploy

# DEV ONLY: isolated product-owned simulator + desktop-core sandbox (no physical device).
virtual-device:
    bun firmware/pip/simulator/run.mjs

# Real Runtime/desktop-core smoke with fake OS and a captured firmware framebuffer.
virtual-device-smoke:
    node firmware/pip/simulator/run.mjs --smoke

# Opt-in, several minutes: one virtual session past both protocol sequence wraps (no soak credit).
virtual-device-endurance:
    node firmware/pip/simulator/run.mjs --endurance
