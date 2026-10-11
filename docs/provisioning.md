# Provisioning and per-unit QA

`just provision` flashes the current firmware onto one new unit and checks that it works. It is a
developer tool (run from a repo checkout with the Rust and RISC-V toolchains). It is never part of
an installer: the release-surface guard fails if it leaks into a release build.

## Steps

1. Plug in **one** Kivori by USB. Unplug any other Kivori first; the tool refuses to run with two.
2. Run `just provision`. It builds the firmware, flashes it, then waits for the unit to restart.
3. Watch the terminal. It holds the connection for 5 seconds, then asks for each control in turn
   (20 seconds each):
   - turn the knob right 3 clicks, then left 3 clicks
   - press the knob, then hold it for about a second
   - press the left, middle and right buttons under the screen
4. It prints `PASS` or `FAIL` with the reason, the unit's short hash and the firmware versions, and
   adds one line to `provisioning-log.csv` in the folder you ran it from.

Before it flashes, the tool connects to the unit and applies the app's image-versus-device identity
guard: a unit that is a different product or board than the bundled firmware is refused (a unit with
no identity message counts as the Desk Buddy on the C3 SuperMini; a unit that does not answer at
all, such as a blank board, cannot be checked and is flashed).

## Burning the identity into the chip

`just provision --product desk-buddy --board c3-supermini --rev 1` resolves the ids through the
product catalog (an unknown id is an error that lists the valid ones; all three options go together,
`--rev` is 1 to 255). **Without `--burn-identity` this is a dry run**: it prints the exact record
(fields, the 12 bytes in hex, the CRC-32) and carries on with the normal provisioning. Nothing is
written.

**Burning is permanent. The record goes into one-time-programmable eFuses (the C3 user data block):
it can never be erased or changed, and a wrong one cannot be fixed on that chip.**

Prerequisite (developer machine only, never part of an installer): Espressif's `espefuse` from
`esptool`, pinned to **`esptool==4.7.0`** (`pip install esptool==4.7.0`; it installs `espefuse.py`).
The tool runs `espefuse --help` and refuses any major version other than 4.

To burn, add `--burn-identity`: `just provision --product desk-buddy --board c3-supermini --rev 1
--burn-identity`. Checks, in order; any failure stops before anything is written:

1. The bundled firmware must be for that product and board (otherwise the unit would show the
   wrong-board screen), and the unit must pass the identity guard above.
2. `espefuse` is found and has major version 4.
3. The chip's whole user data block is read (`espefuse dump`, read only). If it is not all zero the
   tool refuses. If it already holds exactly this record it says "already provisioned" and goes on
   without writing. Any other content (another record, a damaged one, stray bits) is an error.
4. The record and the port are shown, and you must **type the product id** (for example
   `desk-buddy`) exactly. Anything else aborts. There is no flag or environment variable that
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
`health`, `input:button_left`). Fix the cause and run it again; each run adds a row.

## What is checked

- Exactly one allowlisted port, and the bundled firmware flashes.
- The handshake completes and the required capabilities are negotiated (including
  `HOST_TAKEOVERS_V1`).
- The unit's firmware version equals the bundled version (read from the image's version marker).
- A state report and a health report arrive; the heartbeat holds for 5 seconds.
- All seven guided inputs are seen.

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
- The 7 input checks and the physical row in [validation.md](./validation.md) (Phase 12) still need a
  person at the bench; this tool only records what the unit reported.
