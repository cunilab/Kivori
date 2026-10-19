# Provisioning and per-unit QA

`just provision` flashes the current firmware onto one new unit and checks that it works. It is a
developer tool (run from a repo checkout with the Rust and RISC-V toolchains). It is never part of
an installer: the release-surface guard fails if it leaks into a release build.

## QA target and build scope

The bundled image's identity selects the catalog product and board, its minimum required
capabilities, discovery hints and guided input groups. Missing/unknown image identity, unknown
capability names or incomplete input prerequisites stop before any port is opened. No first entry
or product-name default selects QA requirements.

`just provision` builds the catalog-selected release firmware. With today's sole product/board pair,
no selection is needed. For an explicit target, set `KIVORI_PRODUCT` and `KIVORI_BOARD`, for example
`KIVORI_PRODUCT=pala KIVORI_BOARD=c3-supermini just provision`. If the catalog has multiple pairs,
both variables are required; catalog order never selects a default.

`--product/--board/--rev` remain identity preview/burn assertions; they do **not** select the build.
They must match the bundled image even without `--burn-identity`. For a separately built image, supply
its absolute path through `KIVORI_FIRMWARE_PATH` to `cargo run -p kivori-desktop --example provision`.
The desktop validates the image against its catalog entry. Multi-image packaging remains separate
M2.8 work.

## Steps

1. Plug in **one** Kivori by USB. Unplug any other Kivori first; the tool requires exactly one candidate for the selected board.
2. Run `just provision`. It builds the firmware, flashes it, then waits for the unit to restart.
3. Watch the terminal. It identifies the selected catalog target, holds the connection for 5
   seconds, then asks for the input groups that product requires (20 seconds each). For Pip:
   - turn the knob right 3 clicks, then left 3 clicks
   - press the knob, then hold it for about a second
   - press the left, middle and right context buttons
4. It prints `PASS` or `FAIL` with the reason, the unit's short hash and the firmware versions, and
   adds one line to `provisioning-log.csv` in the folder you ran it from.

Before it flashes, the tool connects to the unit and applies the app's image-versus-device identity
guard: a unit that is a different product or board than the bundled firmware is refused (a unit with
no identity message counts as the Kivori Pip on the C3 SuperMini; a unit that does not answer at
all, such as a blank board, cannot be checked and is flashed).

## Burning the identity into the chip

`just provision --product pala --board c3-supermini --rev 1` resolves the ids through the
product catalog (an unknown id is an error that lists the valid ones; all three options go together,
`--rev` is 1 to 255). **Without `--burn-identity` this previews the identity record only** (fields, 12 bytes in hex and
CRC-32). **Firmware is still flashed and QA still runs. No eFuses are written.** It is not an
offline dry run. The preview's revision is not checked against the device; revision verification
applies only when `--burn-identity` is requested.

**Burning is permanent. The record goes into one-time-programmable eFuses (the C3 user data block):
it can never be erased or changed, and a wrong one cannot be fixed on that chip.**

Prerequisite (developer machine only, never part of an installer): Espressif's `espefuse` from
`esptool`, pinned to **`esptool==4.7.0`** (`pip install esptool==4.7.0`; it installs `espefuse.py`).
The tool runs `espefuse --help` and refuses any major version other than 4.

Identity burning is implemented only for `c3-supermini` and its ESP32-C3 `BLOCK_USR_DATA` backend.
Other boards are refused before device I/O when `--burn-identity` is requested, even if their flash
method is `espflash`. The explicit board binding is a hardware-backend exception, never a product
selection default. Another eFuse/OTP layout requires separate implementation and bench validation.

To burn, add `--burn-identity`: `just provision --product pala --board c3-supermini --rev 1
--burn-identity`. Checks, in order; any failure stops before anything is written:

1. The bundled firmware must be for that product and board (otherwise the unit would show the
   wrong-board screen), and the unit must pass the identity guard above.
2. `espefuse` is found and has major version 4.
3. The chip's whole user data block is read (`espefuse dump`, read only). If it is not all zero the
   tool refuses. If it already holds exactly this record it says "already provisioned" and goes on
   without writing. Any other content (another record, a damaged one, stray bits) is an error.
4. The record and the port are shown, and you must **type the product id** (for example
   `pala`) exactly. Anything else aborts. There is no flag or environment variable that
   confirms for you.
5. It runs `espefuse --chip esp32c3 --port <port> --do-not-confirm burn_block_data BLOCK_USR_DATA
   <file>`; `--do-not-confirm` is passed only now, after your typed confirmation. The file is the 12
   record bytes padded with zeros to the 32-byte block (the size `burn_block_data` requires).
6. The unit is flashed (which restarts it), reconnected, and its `Identity` must report
   `source = Chip` with the same product, board and revision. The result is printed and a mismatch
   is a `FAIL` (`identity` in the failed checks).

If the record turns out to be wrong: the unit now reports the burned identity (`source = Chip`)
whatever the firmware build says, and a firmware built for a different product or board shows the
wrong-board screen. The block cannot be rewritten (the tool refuses a non-blank block, and flipping
more bits would corrupt the record, which a unit then ignores as if it were absent). A damaged
record is treated as not present, so the unit still runs, but its identity is no longer protected.
Treat such a chip as having the wrong identity and do not ship it as another product.

A `FAIL` lists the failed checks (for example `capability:HOST_TAKEOVERS_V1`, `firmware_version`,
`health`, `product_identity`, `input:button_left`). Fix the cause and run it again; each run adds a row.

## What is checked

- Exactly one port nominated by the selected board’s USB hints, and the bundled firmware flashes.
  USB is only a discovery hint; the pre-flash identity guard still applies.
- The handshake completes and the selected product’s catalog-required capabilities are negotiated
  (including `HOST_TAKEOVERS_V1` for Pip).
- After flashing, identity must match the selected product/board and report `Build` or `Chip`;
  missing, unknown, wrong or legacy-fallback identity fails as `product_identity`. Pre-flash legacy
  identity remains accepted for the matching legacy product/board.
- The unit and bundled version marker must both equal the selected image identity’s firmware version.
- A state report and a health report arrive; the heartbeat holds for 5 seconds.
- All catalog-required input checks are supported and seen. Physical input requires both turn
  directions; push-switch adds press/hold, and context-button support adds each protocol-defined
  context button. Missing negotiated support skips the wait but fails both the required capability
  and its input checks. Negotiated omissions cannot shrink the QA policy. Optional negotiated
  controls beyond catalog minimums are outside this check set.
- Double-press and host-takeover capabilities are checked through negotiation; this tool does not
  guide functional checks for those behaviors. Context-button checks use the protocol's fixed
  button count.
- Heartbeat, state and health remain baseline requirements even for products with zero input
  checks. The example prints that zero-check count explicitly.

## The log

`provisioning-log.csv` is local and gitignored. Columns: `timestamp_utc`, `device_hash`,
`device_fw`, `bundled_fw`, `result`, `failed_checks`, then `identity_source` (`legacy`, `build` or
`chip`), `identity_product`, `identity_board`, `identity_rev` (`unknown` when not reported). An
existing log keeps its old header and rows; delete or move it to get the new columns. It holds only
the short, non-reversible device hash (the one the app shows), never the raw device id, and never a
port name. The terminal shows
the port name for the operator; it is not saved.

## Limits

- No `--erase`: the app's flashing path runs `espflash flash` only, which has no erase option, so a
  whole-chip erase for a first-time unit is not offered. Run `espflash erase-flash` by hand if needed.
- A board stuck in ROM download mode (BOOT held) may need the app's recovery flow first.
- The required input checks (seven for Pip) and the physical row in [validation.md](./validation.md) (Phase 12) still need a
  person at the bench; this tool only records what the unit reported.
