#!/usr/bin/env bash
# Runs the Wokwi pre-hardware simulation scenarios against the real firmware ELFs.
#
# Four modes, four artifacts, four diagrams:
#   internal  kivori-selftest.elf + diagram.json             on-target self-test, markers over UART0
#   external  kivori-serial.elf   + diagram-usb-serial.json  real USB Serial/JTAG rx/tx, Wokwi injects frames
#   spi       kivori-spi.elf      + diagram-spi.json         real T072 SPI DisplaySink → generic display part
#   runtime   kivori-runtime.elf  + diagram-runtime.json     the REAL production run loop (T074), sim profile
#
# The SPI scenario additionally captures a VCD (logic-analyzer) file and runs the deterministic host-side
# checker in tools/wokwi-vcd over it. A failed VCD check fails the scenario.
#
# Requires `wokwi-cli` and a WOKWI_CLI_TOKEN (https://wokwi.com/dashboard/ci). The token is read from the
# environment ONLY — never a command-line argument, never logged, never committed.
#
# This gate is ADDITIONAL: host tests remain primary, and passing here is NOT evidence of physical
# hardware correctness (see sim/wokwi/README.md).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# shellcheck source=lib/wokwi-evidence.sh
. "$ROOT/scripts/lib/wokwi-evidence.sh"
# shellcheck source=lib/load-env.sh
. "$ROOT/scripts/lib/load-env.sh"
# A GUI-launched process does not inherit a terminal's exports, so fall back to the gitignored local .env.
# A real environment variable (e.g. a CI secret) always wins; the value is never printed.
wokwi_load_token
PROJECT="sim/wokwi"
STAGE="$ROOT/target/wokwi"
# Overridable so a control-flow/harness run can be pointed at a scratch directory instead of clobbering
# the evidence transcripts of a real run (which is exactly what happened once — the logs are the evidence,
# so the default location must never be written by anything but a genuine simulation).
LOGS="${WOKWI_LOGS_DIR:-$ROOT/target/wokwi-logs}"
VCD_DIR="${WOKWI_VCD_DIR:-$ROOT/target/wokwi-vcd}"
TIMEOUT_MS="${WOKWI_TIMEOUT_MS:-30000}"

# --- preflight: fail loudly rather than silently "passing" with no simulator ---------------------
if ! command -v wokwi-cli >/dev/null 2>&1; then
  cat >&2 <<'MSG'
error: `wokwi-cli` is not installed, so no simulation was run.
  install: curl -L https://wokwi.com/ci/install.sh | sh
  then add it to PATH: export PATH="$HOME/.wokwi/bin:$PATH"
MSG
  exit 2
fi

if [ -z "${WOKWI_CLI_TOKEN:-}" ]; then
  cat >&2 <<'MSG'
error: WOKWI_CLI_TOKEN is not set, so no simulation was run.
  Create a CI token at https://wokwi.com/dashboard/ci (it looks like `wok_...`), then either export it:
    export WOKWI_CLI_TOKEN=...
  or put it in the gitignored local .env at the repository root (needed when this process was launched
  from a GUI app, which does not inherit a terminal's exports):
    WOKWI_CLI_TOKEN=...
  In CI, provide it as the repository secret WOKWI_CLI_TOKEN.
  NOTE: an interactive user/session token (~/.wokwi/user.tok) is NOT a CI token and the API
        rejects it with "Unauthorized".
MSG
  exit 2
fi

for elf in kivori-selftest.elf kivori-serial.elf kivori-spi.elf kivori-runtime.elf; do
  [ -f "$STAGE/$elf" ] || { echo "error: $STAGE/$elf missing — run: bash scripts/build-wokwi-firmware.sh" >&2; exit 2; }
done

mkdir -p "$LOGS" "$VCD_DIR"

# scenario | elf | diagram | required success marker | capture-vcd (1 = yes) | wall-clock timeout ms
#
# The timeout is a WALL-CLOCK budget, not a scenario assertion. Wokwi runs slower than real time and the
# scenarios do very different amounts of work: the production runtime re-hashes six 240x40 tiles every
# simulated 33 ms to decide whether anything changed, so it needs a larger budget than a marker-only
# self-test. Every marker/failure/VCD assertion is identical regardless of this value.
#
# The required marker is verified in the captured serial log AFTER the run, so a scenario can never pass
# on simulator exit alone. Scenario paths resolve relative to the project directory.
SCENARIOS=(
  "scenarios/boot.yaml|kivori-selftest.elf|diagram.json|KIVORI-SIM PASS lifecycle-booting-to-offline|0|"
  "scenarios/protocol.yaml|kivori-selftest.elf|diagram.json|KIVORI-SIM PASS malformed-recovery|0|"
  "scenarios/state-cycle.yaml|kivori-selftest.elf|diagram.json|KIVORI-SIM ALL PASS|0|"
  "generated/serial-smoke.yaml|kivori-serial.elf|diagram-usb-serial.json|KIVORI-EXT TX kind=Pong|0|"
  "generated/protocol-serial.yaml|kivori-serial.elf|diagram-usb-serial.json|KIVORI-EXT SEQ seq=0 class=ok|0|"
  "generated/state-cycle-serial.yaml|kivori-serial.elf|diagram-usb-serial.json|KIVORI-EXT ALL PASS state-cycle|0|"
  "scenarios/spi-display-probe.yaml|kivori-spi.elf|diagram-spi.json|KIVORI-SPI ALL PASS|1|60000"
  "generated/production-runtime.yaml|kivori-runtime.elf|diagram-runtime.json|KIVORI-RUN ALL PASS production-runtime|1|180000"
)

# Any of these in the serial log fails the scenario regardless of the CLI's exit status.
FAILURE_SIGNATURES=("panicked" "KIVORI-SIM FAIL" "KIVORI-EXT FAIL" "KIVORI-SIM ALL FAIL"
  "KIVORI-SPI FAIL" "KIVORI-SPI ALL FAIL" "KIVORI-RUN FAIL")

# Verifies the captured log: required marker present, no failure signature, no reset loop.
verify_log() {
  local log="$1" required="$2" ok=0
  if ! grep -aqF -- "$required" "$log" 2>/dev/null; then
    echo "    ✗ required marker never observed: '$required'"
    ok=1
  fi
  local sig
  for sig in "${FAILURE_SIGNATURES[@]}"; do
    if grep -aqF -- "$sig" "$log" 2>/dev/null; then
      echo "    ✗ failure signature in log: '$sig'"
      ok=1
    fi
  done
  # A second ESP boot banner means the device reset mid-run (reset loop / crash-restart).
  local boots
  boots=$(grep -ac "ESP-ROM:" "$log" 2>/dev/null || echo 0)
  if [ "${boots:-0}" -gt 1 ]; then
    echo "    ✗ reset loop: $boots boot banners in one run"
    ok=1
  fi
  return $ok
}

# Optional filter: `bash scripts/test-wokwi.sh boot protocol-serial`
if [ "$#" -gt 0 ]; then
  FILTERED=()
  for want in "$@"; do
    for entry in "${SCENARIOS[@]}"; do
      case "${entry%%|*}" in *"$want".yaml) FILTERED+=("$entry");; esac
    done
  done
  [ "${#FILTERED[@]}" -gt 0 ] || { echo "error: no scenario matched: $*" >&2; exit 2; }
  SCENARIOS=("${FILTERED[@]}")
fi

fail=0
# Durable evidence: exit codes and durations only exist in this loop, so they are captured here and
# written to target/wokwi-logs/summary.json for CI forensics (see scripts/lib/wokwi-evidence.sh).
SUMMARY="$LOGS/summary.json"
ROWS="$(mktemp)"
trap 'rm -f "$ROWS"' EXIT
CLI_VERSION="$(wokwi-cli --version 2>/dev/null | head -1)"
API_VERSION=""
first_row=1

for entry in "${SCENARIOS[@]}"; do
  IFS='|' read -r scenario elf diagram required want_vcd budget_ms <<< "$entry"
  timeout_ms="${budget_ms:-$TIMEOUT_MS}"
  name=$(basename "$scenario" .yaml)
  log="$LOGS/$name.log"
  vcd=""
  # Expanded below as ${vcd_args[@]+…}: bash 3.2 (still the macOS default) treats a plain "${arr[@]}" on
  # an EMPTY array as an unbound variable under `set -u`, which would abort the six non-VCD scenarios.
  vcd_args=()
  if [ "$want_vcd" = "1" ]; then
    vcd="$VCD_DIR/$name.vcd"
    vcd_args=(--vcd-file "$vcd")
  fi
  echo "── Wokwi scenario: $name (elf=$elf, diagram=$diagram, budget=${timeout_ms}ms) ──────────────────"
  # Pin the exact bytes about to be simulated, so a later rebuild cannot blur which binary made this log.
  elf_id="$(evidence_artifact_id "$STAGE/$elf")"
  start_ms="$(evidence_now_ms)"
  start=$(date +%s)
  # `--fail-text` aborts on a self-test failure or a panic; scenario `wait-serial` steps time out if an
  # expected line never appears (missing handshake/Pong, render stall, reset loop).
  set +e
  wokwi-cli "$PROJECT" \
    --elf "$STAGE/$elf" \
    --diagram-file "$ROOT/$PROJECT/$diagram" \
    --scenario "$scenario" \
    --timeout "$timeout_ms" \
    --timeout-exit-code 42 \
    --serial-log-file "$log" \
    ${vcd_args[@]+"${vcd_args[@]}"} \
    --fail-text "panicked" \
    2>&1 | tee "$LOGS/$name.stdout"
  status=${PIPESTATUS[0]}
  set -e
  duration_ms=$(( $(evidence_now_ms) - start_ms ))
  duration=$(( $(date +%s) - start ))
  # Record the real exit code now, while it still exists.
  [ -z "$API_VERSION" ] && API_VERSION="$(grep -aoE 'Simulation API [0-9][^ ]*' "$LOGS/$name.stdout" 2>/dev/null | head -1 | sed 's/Simulation API //')"
  # A VCD capture is evidence in its own right: parse it and assert the digital bus properties. An
  # unusable or unconvincing capture fails the scenario — it is never accepted just for existing.
  vcd_result="none"
  if [ -n "$vcd" ]; then
    if [ ! -s "$vcd" ]; then
      echo "    ✗ no VCD capture written to $vcd"
      vcd_result="fail"
    elif cargo run -q -p kivori-wokwi-vcd -- "$vcd"; then
      vcd_result="pass"
    else
      vcd_result="fail"
    fi
  fi
  [ "$first_row" -eq 1 ] || printf ',\n' >> "$ROWS"
  first_row=0
  evidence_json_row "$name" "$STAGE/$elf" "$elf_id" "$status" "$duration_ms" "$log" "$required" \
    "$vcd" "$vcd_result" >> "$ROWS"
  if [ "$status" -eq 0 ]; then
    # Exit 0 is necessary but not sufficient: the log must also prove the outcome.
    if verify_log "$log" "$required" && [ "$vcd_result" != "fail" ]; then
      echo "  ✓ $name passed in ${duration}s (marker: '$required', log: $log)"
    else
      echo "  ✗ $name FAILED verification despite exit 0 (${duration}s, log: $log, vcd: $vcd_result)"
      fail=1
    fi
  else
    echo "  ✗ $name FAILED (exit=$status, ${duration}s, log: $log)"
    [ "$status" -eq 42 ] && echo "    exit 42 = scenario timeout: an expected serial line never arrived."
    fail=1
  fi
done

evidence_write_summary "$SUMMARY" "$CLI_VERSION" "$API_VERSION" "$ROWS"
echo "Evidence summary: $SUMMARY"

if [ "$fail" -ne 0 ]; then
  echo "Wokwi simulation gate FAILED." >&2
  exit 1
fi

echo "Wokwi simulation gate OK — internal self-test, external serial path, and the GENERIC SPI/RGB565"
echo "tile-transfer probe all verified in simulation."
echo "NOTE: simulation is not physical validation. USB enumeration, the real panel controller's init"
echo "      sequence, panel offsets/orientation/colour order, analog SPI integrity, sustained frame rate,"
echo "      and unplug/reconnect remain hardware tasks (docs/validation.md)."
