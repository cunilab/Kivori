# Third-party notices

Kivori's installers bundle the following third-party software in addition to the Rust and JavaScript
dependencies recorded in `Cargo.lock` and `bun.lock`.

## espflash

- Bundled as a sidecar next to the app (`espflash` / `espflash.exe`) and used to flash the Kivori
  firmware. Pinned to version 4.5.0 (see `scripts/fetch-espflash.sh`).
- Project: <https://github.com/esp-rs/espflash>
- License: MIT OR Apache-2.0. Copyright the espflash contributors.
  License texts: <https://github.com/esp-rs/espflash/blob/main/LICENSE-MIT> and
  <https://github.com/esp-rs/espflash/blob/main/LICENSE-APACHE>.

## mediaremote-adapter (macOS builds only)

- Compiled from the pinned source in `apps/desktop/src-tauri/vendor/mediaremote-adapter` and bundled in
  `Contents/Resources/mediaremote-adapter`. It reads the system-wide "now playing" state (ADR-0009).
- Project: <https://github.com/ungive/mediaremote-adapter>
- License: BSD 3-Clause. The full text ships alongside the helper in
  `mediaremote-adapter/LICENSE` and in `apps/desktop/src-tauri/vendor/mediaremote-adapter/LICENSE`.
