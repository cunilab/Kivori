#!/usr/bin/env bash
# Hardware-free ESP32-C3 frame cost: builds the bench (the real renderer for riscv32imc) and the
# simulator, then runs, checks or refreshes the committed fixture. Used by `just fw-frame-cost*` and CI.
#   run.sh run [args]   print the predicted compose time of every scenario (args go to `frame-cost run`)
#   run.sh check        simulator tests + native-parity test + regression check against the fixture
#   run.sh update       rewrite fixtures/frame-cost.json from the current renderer
set -euo pipefail
cd "$(dirname "$0")"
# Both builds name their own target; inherited overrides would send the bench to the wrong one.
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_TARGET CARGO_TARGET_DIR
HOST="$(rustc -vV | sed -n 's/^host: //p')"
ELF="$PWD/bench/target/riscv32imc-unknown-none-elf/release/kivori-frame-cost-bench"
SIM="target/$HOST/release/kivori-pip-frame-cost"
FIXTURE="fixtures/frame-cost.json"

build() {
  (cd bench && cargo build --release --locked)
  cargo build --release --locked --target "$HOST"
}

case "${1:-}" in
  run)
    build
    shift
    "$SIM" run --elf "$ELF" "$@"
    ;;
  check)
    build
    KIVORI_FRAME_COST_ELF="$ELF" cargo test --release --locked --target "$HOST"
    "$SIM" check --elf "$ELF" --fixture "$FIXTURE"
    ;;
  update)
    build
    "$SIM" run --elf "$ELF" --out "$FIXTURE"
    echo "wrote $FIXTURE; commit it with the reason the predicted cost changed"
    ;;
  *)
    echo "usage: run.sh run|check|update" >&2
    exit 2
    ;;
esac
