# Third-party notices

Kivori's installers bundle the following third-party software in addition to the Rust and JavaScript
dependencies recorded in `Cargo.lock` and `bun.lock`.

## mediaremote-adapter (macOS builds only)

- Compiled from the pinned source in `apps/desktop/src-tauri/vendor/mediaremote-adapter` and bundled in
  `Contents/Resources/mediaremote-adapter`. It reads the system-wide "now playing" state (ADR-0009).
- Project: <https://github.com/ungive/mediaremote-adapter>
- License: BSD 3-Clause. The full text ships alongside the helper in
  `mediaremote-adapter/LICENSE` and in `apps/desktop/src-tauri/vendor/mediaremote-adapter/LICENSE`.
