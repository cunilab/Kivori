# Pip host runner

Development-only, product-owned executable. This is its own Cargo workspace: the desktop does
not link the firmware library. Build from the repository root with an explicit host target:

```sh
cargo build --locked --manifest-path firmware/pala/simulator/Cargo.toml \
  --target aarch64-apple-darwin --target-dir firmware/pala/simulator/target
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

Each request produces exactly one stdout JSON record with `wire`, optional 57600-pixel `frame`
(row-major RGB565 integers), `brightness`, and `reboot`. Frames are composed by the production
firmware renderer. Time advances only when requested; one request runs exactly one production
runtime tick, so pending protocol input may need subsequent ticks to drain.

Both directions have a 400000-byte record limit excluding newline. EOF exits cleanly; malformed,
truncated, unknown-field, oversized or invalid requests exit nonzero with diagnostics on stderr.
No successful response is emitted for a rejected request. After `reboot:true`, restart the process
before sending another tick. Restarting creates a fresh deterministic virtual identity/session.
The runner never performs OS actions, discovers USB ports, flashes hardware or accesses settings.
