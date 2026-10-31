# Pip host runner

Development-only, product-owned executable. This is its own Cargo workspace: the desktop does
not link the firmware library. Build from the repository root with an explicit host target:

```sh
cargo build --locked --manifest-path firmware/pip/simulator/Cargo.toml \
  --target aarch64-apple-darwin --target-dir firmware/pip/simulator/target
```

Use the host triple reported by `rustc -vV` on other platforms. Tests use the same arguments with
`cargo test`. Do not enable embedded features or inherit embedded `RUSTFLAGS`.

The process accepts one newline-terminated JSON request per runtime tick on stdin:

```json
{"now_ms":33,"wire":[],"edges":[],"capture":true}
```

`wire` contains existing Kivori protocol bytes (up to 8192); `edges` contains up to 128 snapshots
`{"at_ms":32,"levels":3}`. Mask bits 0/1 are adapter-normalized encoder A/B (physical low reads as true); bit 2 is the knob switch;
bits 3/4/5 are left/middle/right contextual switches. Switch bits mean pressed. Initial levels
are zero, the production decoder rest position. One CW detent is masks `[2,3,1,0]`;
one CCW detent is `[1,3,2,0]`. Do not inject active-high electrical rest `3`: the
physical adapter has already inverted the pull-up pin readings at this port. Edge timestamps
must be ordered between the previous tick time and this tick time, inclusive. Clock rollover
is intentionally rejected in this first vertical slice.

An optional `fault` field selects one bounded development fault per request (default `normal`):

- `normal`: actual firmware traffic and build identity; restores both after a fault.
- `silent`: execute the firmware tick and drain its traffic, but return an empty `wire` array.
  Input, rendering and optional capture continue; dropped replies are never replayed.
- `wrong-major`: re-encode only actual outbound `HelloAck` headers with an unsupported major,
  preserving firmware-produced payload and sequence. Other packets are unchanged.
- `unknown-product` / `unknown-board`: report `65535` for that identity field through the actual
  firmware boot-identity seam and `Ready` response; retain other build identity fields.
- `exit`: close the process cleanly without a response, exercising real pipe EOF.

Use a fresh process for identity and protocol fault scenarios, and send the selected mode on every
relevant tick. A live session may switch to `silent` or `exit`. These are process-control values,
not protocol messages, physical faults or production wire-schema additions.

Each request except `exit` produces exactly one stdout JSON record with `wire`, optional 57600-pixel `frame`
(row-major RGB565 integers), `brightness`, `reboot` and the firmware companion `state`
(`booting`, `idle`, `happy`, `busy`, `sleeping` or `offline`). Frames are composed by the production
firmware renderer. The runner uses the production buffered renderer and `TxBuffered` over its pipe.
Time advances only when requested. The process has two timings. By default it is modelled: a request
means "run the device until `now_ms`", on a device clock that charges the ESP32-C3's compose time per
tile row and SPI time per panel window, so frame time, fps, busy % and the delay before input is seen
mid-frame resemble the unit, and Device internals are filled in from a committed resource fixture
(see [Modelled timing and internals](../../../docs/virtual-pip.md#modelled-timing-and-internals)).
The device powers up at the first request's `now_ms` (its clock starts there, so a child spawned late
in a long host session does not simulate the time before it existed).
With `--ideal-time` one request runs exactly one production runtime tick with the clock frozen at
`now_ms`, so frames cost 0 ms and pending protocol input may need subsequent ticks to drain. The
in-process scenarios (`--input-smoke`, `--io-smoke`, `--replay`) always use ideal time.

Both directions have a 400000-byte record limit excluding newline. EOF exits cleanly; malformed,
truncated, unknown-field, oversized or invalid requests exit nonzero with diagnostics on stderr.
No successful response is emitted for a rejected request. After `reboot:true`, restart the process
before sending another tick. Restarting creates a fresh deterministic virtual identity/session.
The runner never performs OS actions, discovers USB ports, flashes hardware or accesses settings.

## Raw input fidelity smoke (S5.3)

`just virtual-device-smoke` also invokes this runner with `--input-smoke`. To run only these
scenarios, build for the Rust host target as above, then execute the resulting
`kivori-pip-simulator --input-smoke`. This explicit mode does not read JSON stdin; normal invocation
without arguments retains the pipe contract above. Unknown CLI arguments fail nonzero.

The suite uses the same `Simulator::step`, actual firmware `Runtime`, timestamped `InputSource`
edges, negotiated protocol and RGB565 capture. It compares complete decoded input-event traces
(control, kind and firmware emission timestamp), checks the current nonce, then settles each case
to reject extra actions. Settled trace cases print the actual events, capture FNV-1a 64-bit hash and
virtual firmware time; a mismatch fails the process. The hashes identify actual frames, rather than
claiming physical panel output or acting as fixed renderer golden images.

Coverage includes CW/CCW cycles; illegal transitions and partial-cycle bounce followed by a valid
cycle; 19/20 ms debounce, 499/500 ms Press/Hold and 1999/2000 ms recovery boundaries; switch bounce;
negotiated double-press gaps of 249/250/251 ms; delayed buffered edges on all four switches;
knob/contextual/rotary ownership; 128-edge capacity and fail-closed 129-edge request rejection;
session cancellation, recovery release and the exact virtual reboot boundary. Recovery pixels are
compared with a same-time no-input control to establish a causal display change. Reboot refusal
and a fresh runtime's next press are also checked.

The defensive Runtime sample-overflow case deliberately bypasses only the runner's JSON edge
bound and feeds an oversized `InputSource` drain into the same Runtime: the final release and next
valid press survive without extra actions. Runtime holds 129 samples, matching the normal 128-edge
request plus final levels, so this branch cannot be reached by a valid JSON request. This is input-port
fault injection, not a physical ISR queue overflow test. No desktop OS action mapping, wall-clock
deadline, electrical bounce, hardware reset circuit or physical timing is established by this suite.

## Desktop behavior checks (S5.4)

The launcher also runs the native development example's `--desktop-smoke` mode; this is not a runner
JSON fault or runner CLI flag. It uses existing fake OS services and a disposable configuration
directory owned by the native CLI. Labelled cases distinguish raw firmware/session execution from
direct desktop action tests and configuration persistence. The normal development window remains
memory-only. See [S5.4 evidence](../../../docs/validation.md#s54-simulated-desktop-behavior-evidence)
for setup UI coverage and the limits of simulated state. No real OS actions or installed settings
are used by this mode.

## Recovery rehearsal (S5.5)

The launcher's `--recovery-smoke` belongs to the native development example, not this runner.
Counting fake executors rehearse production flash/update policy and admission. The existing runner
can reconnect as a compatible or wrong-major peer; no new firmware is written to it. BOOT and
update UI checks use separate browser mocks. See [S5.5 evidence](../../../docs/validation.md#s55-recovery-and-compatibility-rehearsal-evidence).
Real flashing, downloads, installation, signature validation and physical recovery remain outside
this mode.

## Fixed replay and reports (S5.6)

The runner accepts `--replay 17` or `--replay 29` and emits one JSON result. Four raw recipes per seed
include complete input/time data, independently expected logical events, actual events and committed
RGB565 frame hashes from `fixtures/replay-goldens.json`. A mismatch produces a failed result and
nonzero exit; unsupported seeds also fail. Normal JSON pipe and `--input-smoke` behavior remain.

`just virtual-device-smoke` uses Node and runs both seeds twice, requiring byte-identical successful
JSON. It prints the new ignored `target/reports/run-*/report.json` path; that directory also contains
per-stage logs and the captured PPM. Build/source fingerprints, clock/layer labels and exclusions
are recorded. CI artifacts are named `virtual-device-report-<os>` and retained for 14 days.
A report left `running` after interruption is incomplete. Production/offline/no_std checks are
separate CI requirements, explicitly listed in the report.

Golden updates require checking the intended renderer/action behavior and reviewing changed hashes;
no verification mode rewrites expected values. This is fixed-recipe regression coverage, not a
fuzzer or evidence of physical appearance/timing. See [S5.6 evidence](../../../docs/validation.md#s56-repeatable-reports-ci-and-boundary-evidence).

## Hardware I/O stress checks

`--io-smoke` runs the buffered production runtime and transmit queue against bounded digital
I/O models. The full launcher includes this as a separate report stage. The JSON pipe contract
is unchanged; `silent` still discards replies and is distinct from an unread FIFO.

The I/O scenarios model a 64-byte transmit FIFO, host servicing, and SPI payload wire time at
the board profile's clock rate. Scheduled raw edges become available during display windows.
Assertions cover event delivery during a frame, a stalled host followed by a fresh handshake,
and failed display transfer recovery. These are modeled digital timing checks; composition CPU
cost, command overhead, interrupt latency, analog bounce, physical USB and actual OS effects
remain unmeasured. No paid simulator is required.
