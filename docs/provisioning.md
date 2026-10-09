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
`device_fw`, `bundled_fw`, `result`, `failed_checks`. It holds only the short, non-reversible device
hash (the one the app shows), never the raw device id, and never a port name. The terminal shows
the port name for the operator; it is not saved.

## Limits

- No `--erase`: the app's flashing path runs `espflash flash` only, which has no erase option, so a
  whole-chip erase for a first-time unit is not offered. Run `espflash erase-flash` by hand if needed.
- A board stuck in ROM download mode (BOOT held) may need the app's recovery flow first.
- The 7 input checks and the physical row in [validation.md](./validation.md) (Phase 12) still need a
  person at the bench; this tool only records what the unit reported.
