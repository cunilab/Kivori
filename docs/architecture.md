# Kivori architecture

How Kivori is built today. Product goals are in [product.md](./product.md), open work in [roadmap.md](./roadmap.md), test and hardware evidence in [validation.md](./validation.md).

## 1. Overview

Kivori is a small desk companion. A keycap-shaped mascot lives on a 240x240 screen, reacts to what the desktop tells it, a rotary knob turns volume (or per-profile shortcuts), its push switch runs actions (Press, Hold, Double press) and the out-of-band recovery hold, and three contextual buttons run per-profile actions. The display can also show a clock, volume, media and CPU/RAM view.

| Component | What it is |
|---|---|
| Firmware | ESP32-C3, ST7789 240x240 panel, HW-040 rotary encoder, three buttons. Version 1.3.0, protocol 1.5. `no_std`, no heap, `riscv32imc`. |
| Desktop native core | Tauri 2.12 Rust process. Owns the serial port, the connection, OS integration, preview rendering, flashing. |
| Desktop webview | React UI. Only calls a fixed set of typed commands. No serial, filesystem or shell access. |
| Shared crates | One copy of the model, protocol, renderer and asset reader, used by both firmware and desktop. |

```text
                  USB Serial/JTAG (COBS + CRC-32 + postcard frames)
  +--------------------+  <------------------------------------>  +---------------------+
  | Desktop native core|                                           | Firmware (ESP32-C3) |
  |  device thread     |   Hello/HelloAck/Ready, SetState,         |  ports: Clock,      |
  |  orchestrator      |   InputEvent, Presentation, ...           |  Transport, Display,|
  |  platform backends |                                           |  Input              |
  +---------+----------+                                           |  renderer -> ST7789 |
            | typed Tauri IPC (commands, events, binary channel)   +----------+----------+
  +---------+----------+                                                      |
  | React webview      |                                        HW-040 A/B/SW + panel
  +--------------------+
```

The wire carries meaning (state, input, presentation), never pixels. Both sides render with the same crates, so the Device Studio preview matches the panel.

### Repo layout

Two Cargo workspaces. Shared crates are `no_std`, no-alloc, and the single source of truth.

| Path | Purpose |
|---|---|
| `crates/kivori-model` | State enums, device profile, timeline, version and capabilities, mascot animator, input and presentation types. |
| `crates/kivori-protocol` | Wire codec: COBS, header, CRC-32, postcard `Message`, handshake and negotiation, sequence policy. |
| `crates/kivori-framebuffer` | RGB565 tile bands and content hashing (change detection). |
| `crates/kivori-renderer` | Deterministic scene compositor and volume overlay (`embedded-graphics` `DrawTarget`). |
| `crates/kivori-assets` | Zero-copy reader for the compiled asset blob. |
| `apps/desktop/src-tauri` | `kivori-desktop`: `device/` (discovery, session, reconnect, heartbeat, nonce), `runtime/` (device task), `orchestrator/`, `input/`, `action/`, `platform/`, `presentation/`, `desk/` (profiles, bindings, catalog, actions), `config/` (store, schema, resolve, migrate), `companion.rs`, `activity/`, `catalog.rs` (product registry and catalog, ADR-0014), `firmware.rs`, `firmware_marker.rs`, `window_lifecycle.rs`, `provision.rs` (dev-only `just provision`), `ipc/`, `render/`. |
| `apps/desktop/src` | React frontend and typed IPC wrappers. |
| `products/` | `registry.toml` (machine ids and wire numbers for products and boards) and `<id>/product.toml` (customer name, short name, model code, boards, flash method, USB hints, bundled firmware crate and artifact, required capabilities); compiled into the app, see ADR-0014. |
| `firmware/pala` | The Kivori Pip firmware (crate `kivori-firmware`); separate workspace: `ports.rs`, `input/` (quadrature, gesture), `physical_st7789.rs`, `physical_rotary.rs`, `runtime.rs`, `proto.rs`, plus host simulation adapters in `sim/`. |
| `tools/asset-compiler` | SVG to deterministic RGB565 blob (host build tool). |
| `tests/e2e-host-sim`, `tests/golden-frames` | Host-sim end-to-end tests, cross-OS frame-hash harness. |
| `.github/workflows/` | CI, see below. |
| `scripts/` | Boundary guards (`check-crate-boundaries.sh`, `check-offline-deps.sh`, `check-frontend-offline.mjs`) and firmware/desktop build scripts. |

CI workflows (`.github/workflows/`):

| Workflow | What it runs |
|---|---|
| `host` | Rust on Ubuntu and Windows, the frontend, and the Windows Device Studio startup smoke test: format, clippy, tests, the boundary, offline and release-surface guards. |
| `firmware` | no_std isolation proof, firmware host-sim tests and RISC-V builds and clippy per feature set. |
| `determinism` | Golden-frame hashes across operating systems and the byte-reproducible asset blob. |
| `release` | The unsigned Windows installer and draft release on a `v*` tag, manually with an optional macOS build; also on pull requests that touch packaging files ([release.md](./release.md)). |
| `Release` (`release-start.yml`) | Manual only, from `main`: the owner's Release button. Bumps the app (and optionally firmware) version, refreshes the lockfiles, commits and tags, then calls `release` with the tag; a dry run only prints the plan and notes ([release.md](./release.md)). |

`scripts/check-crate-boundaries.sh` fails CI if a shared crate gains a std, host or OS dependency. Compiling the shared crates for RISC-V is the hard backstop.

### State axes

Three separate axes. Never merge them into one enum.

- Companion (6): `booting, idle, happy, busy, sleeping, offline`. What the device shows.
- Sendable (4): `idle, happy, busy, sleeping`. The subset the desktop may command. `booting` and `offline` come from the device.
- Connection (5): `disconnected, connecting, connected, incompatible, error`. The link lifecycle.

Desktop state is in memory only. `desired` defaults to `idle` and is re-sent after a reconnect. `reported` is what the device says and never overwrites `desired`.

### Chosen platform APIs

- Transport: ESP32-C3 native USB Serial/JTAG (VID:PID `0x303A:0x1001`). It has a 64-byte FIFO and TX can stall if the host does not drain, so frames are small and writes are bounded.
- Firmware HAL: `esp-hal`, bare metal. Display: `mipidsi` with windowed writes.
- Desktop serial: blocking `serialport` on a dedicated device thread.
- Windows volume and mute: Core Audio `IAudioEndpointVolume` with callbacks, on its own COM thread.
- macOS volume and mute: CoreAudio `kAudioHardwareServiceDeviceProperty_VirtualMainVolume` and `kAudioDevicePropertyMute` on the default output device, with HAL property listeners, on its own `kivori-audio` thread (FFI declared by hand in `platform/macos/audio.rs`).
- Media keys and shortcuts: `enigo` (Windows `SendInput`, macOS Quartz events; macOS needs Accessibility permission). On macOS every synthesized input runs on the app's main thread via Tauri's `run_on_main_thread`: since macOS 15 the HIToolbox layout lookups it uses assert the main queue and kill the process otherwise.
- Media playback observation (state, title, artist): Windows Global System Media Transport Controls, polled on a `kivori-media` thread. macOS has no public API, so it is layered (ADR-0009): the vendored MediaRemote adapter, then AppleScript for Spotify and Music, then unknown.
- CPU/RAM: Windows `GetSystemTimes` / `GlobalMemoryStatusEx`; macOS per-CPU `host_processor_info` (`host_statistics` is rate-limited for third-party apps) and Activity Monitor's "Memory Used". Local time: `GetLocalTime` / `localtime_r`.
- Sleep, wake, lock and console changes: Windows `PowerRegisterSuspendResumeNotification` (callback) plus `WTSRegisterSessionNotification` on a message-only window thread; macOS IOKit `IORegisterForSystemPower` on a CFRunLoop thread (hand-declared FFI) plus a 4 Hz poll of the session lock flag. See Host presence.
- Flashing: a pinned `espflash` sidecar, driven by the native core (see Firmware update and recovery).
- Not built yet: a Linux backend, desktop self-update, a factory-provisioned device id.
- Device id (#24): the firmware derives its 16-byte `device_id` from the chip's factory eFuse unique ID, falling back to the base MAC, through a domain-separated SHA-256 (`kivori-device-id-v1`); raw eFuse bytes never go on the wire. The raw id stays in the transport layer; UI and logs only show the short hash (ADR-0005).

## 2. Wire protocol

Contract crate: `kivori-protocol`. Both peers compile the same `Message` enum.

### Framing

One packet is `COBS(frame) || 0x00`. COBS makes the stream self-synchronizing, so the decoder resyncs after corruption. Decoded frame, little-endian:

| Field | Type | Notes |
|---|---|---|
| `magic` | u16 | `0x4B56` ("KV") |
| `ver_major`, `ver_minor` | u16, u16 | Sender protocol version. Outside the payload so a major mismatch is caught before decoding. |
| `seq` | u16 | Wrapping sequence number |
| `payload_len` | u16 | At most `MAX_PAYLOAD` = 512 |
| `payload` | bytes | postcard-encoded `Message` |
| `crc32` | u32 | CRC-32/IEEE over header and payload |

Constants: `PROTOCOL_MAJOR` 1, `PROTOCOL_MINOR` 6. COBS and CRC are implemented in-crate.

Every decode failure is a typed `ProtoError` (`BufferOverflow, Cobs, TooShort, BadMagic, UnsupportedVersion, PayloadTooLarge, LengthMismatch, BadCrc, Postcard`). The decoder never panics, always makes forward progress, and never dispatches a frame that failed CRC.

### Handshake

```text
Desktop                                   Device
  | Hello { desktop_version, desktop_caps, nonce } -> |
  | <- HelloAck { device_caps, device_id, firmware_version, nonce_echo } |
  |  nonce mismatch or unsupported major: Bye, Incompatible/Error       |
  | Ready { negotiated_minor, negotiated_caps } ->    |   Connected
  | SetState { desired } ->  <- StateReport { reported, elapsed_ms }    |
```

- After `Ready`, a device that negotiated `PRODUCT_IDENTITY_V1` sends one `Identity` (product, board, revision, source); one that did not is the legacy Kivori Pip on the C3 SuperMini (see Product identity).
- A device counts as Kivori only after a well-formed `HelloAck` with the matching nonce. VID:PID alone is not identity.
- The desktop gives up on a missing `HelloAck` after 5 s, closes the link, and retries with backoff.
- After connecting, the desktop pings every 1 s and treats 3 misses as a lost link. `Pong.t_ms_echo` matches a ping.
- The desktop never sends state commands to an `Incompatible` device.
- Discovery rotates through every VID:PID match (`CandidateRotator`), because a stock ESP32-C3 shares the ids. Each port has its own backoff; a port that fails to open or answer is passed over for the others, and after 3 handshake timeouts in a row it is only retried when no other candidate exists. A single never-failed candidate is tried immediately.
- On an `Incompatible` handshake the UI shows the state, then the desktop closes that link and skips the port until it is unplugged, so another candidate can connect (`Incompatible` -> `Connecting`). With only the incompatible board plugged in, the state stays and no attempt is made. Port names are never logged or recorded in the activity log.

### Nonce is the session identity

The handshake nonce also identifies the connection. It is minted fresh from OS randomness (`getrandom`, one `u32` per attempt; `device/nonce.rs`), so two desktop processes cannot collide.

- Firmware stamps every `InputEvent` with the nonce of the `Hello` it accepted. The desktop drops any `InputEvent` with another session.
- The desktop stamps every `Presentation`. Firmware drops any with another session, or with a revision that is not newer.
- Firmware clears input state and its revision high-water mark on a new `Hello`, on `Bye`, and on link loss. The desktop clears open gestures whenever it leaves `Connected`.
- `Presentation.revision` increases within a session and restarts at 1 in a new session.
- The nonce is a freshness token. It is not secret and authenticates nothing.

### Negotiation and compatibility

- Compatible if the device major is one the desktop supports. Otherwise `Incompatible`.
- `negotiated_minor = min(desktop, device)`. `negotiated_caps = desktop_caps & device_caps`.
- A capability set on one side only stays off. An unnegotiated peer never sends or acts on the gated message.
- Evolution is append-only: the postcard variant index is the wire tag, and this also holds for every type nested inside a message. Add variants at the end, gate new behavior with a capability. Reordering or removing anything needs a new major.
- Unknown input: a bad frame or an undecodable payload (`Postcard`, including an unknown variant) is rejected and counted, never dispatched. A message that is invalid for the current session phase is rejected too. Unknown capability bits are preserved and ignored.
- Sequence policy: a duplicate `seq` does not re-apply side effects, a gap is counted and the newer frame accepted, wrap `0xFFFF` to `0` is normal. USB CDC is ordered, so there are no retransmits. This is not enough for a firmware-image transfer protocol, which would need its own offsets and acks.
- Frame and blob format changes (asset format v2) require updating desktop and firmware together.

### Firmware update and recovery

- The webview only ever asks for "flash the bundled image" (`flash_firmware`) or "restore" (`restore_firmware`). Ports, paths and `espflash` output never leave the native process. `FirmwareStatus` carries a phase, a safe message, the image size, a `failure` token, the bundled version and an update `advice` token; the UI owns all wording.
- Sidecar lookup (`find_espflash`): `espflash[.exe]` beside the application executable first (where the installer puts it). `PATH` and `~/.cargo/bin` are searched only when built with the `device-studio` feature (the dev default). Release builds use `--no-default-features`, so they never run whatever `espflash` sits on `PATH`. A missing tool is `ToolMissing`. The installer bundles the pinned binary (see Release pipeline).
- Flash: on `FlashFirmware` the device task sends `Bye(FirmwareUpdate)` (only when `supports_host_takeovers()`, so the panel shows Updating), drops the serial handle, runs `espflash` and then waits up to 20 s for the same port to handshake. The device identity must match (`FlashTarget::Verified { port, hash }`), so a swapped board is not reported as success.
- Failures: stderr is captured in-process through a reader thread, bounded to the last 8 KiB, classified by the pure `classify_espflash(exit, stderr)` and dropped; it is never logged or emitted. Classes: `ToolMissing`, `PortBusy`, `NoDownloadMode`, `Timeout`, `Cancelled`, `ImageUnavailable`, `ReconnectTimedOut`, `WrongProduct`, `ChooseProduct`, `Unknown`. `WrongProduct` and `ChooseProduct` are decided before `espflash` runs, so nothing is written (see Product identity). Matching is conservative (case-insensitive fragments of known `espflash` 4.x and OS messages); anything else is `Unknown`. Refresh the fixtures from real output in the hardware session.
- Recovery: a unit that cannot handshake (wrong-major firmware, or held in ROM download mode with BOOT) is restored with `FlashTarget::Recovery { port }`. `request_recovery` refuses unless exactly one allowlisted (`DEFAULT_ALLOWLIST`) port is present, so the flasher cannot be pointed at the wrong board. After the flash, any compatible device on that port is accepted and its new short hash recorded. A flash requested while the connection is `incompatible` takes this path too. A verified flash still rejects a different hash.
- Version advice: `build.rs` scans the embedded firmware image (`KIVORI_FIRMWARE_PATH`) for `KIVORI-FW-VERSION:<x.y.z>\0` with the pure `scan_firmware_version` and sets `KIVORI_BUNDLED_FIRMWARE_VERSION` (empty with no firmware). `update_advice(device, bundled)` returns `UpToDate`, `UpdateAvailable`, `DeviceNewer` or `Unknown`. It is advice only: nothing flashes without an explicit request.

### Product identity (protocol 1.6, `PRODUCT_IDENTITY_V1`, ADR-0014)

- One appended message: tag 23 `Identity { session, product, board, hw_rev, source }` (device to desktop). The device sends it once, right after the `Ready` that negotiated the capability, from the accepted session. `HelloAck` is unchanged (a fixed struct, ADR-0002) and `PROTOCOL_MINOR` stays 6: this capability rides the same minor as the rest of the 1.6 additions, which is what negotiation by capability is for. `product` and `board` are the `u16` numbers of `products/registry.toml` (never 0); `hw_rev` is `None` until provisioning burned one; `source` is `Build` (compiled into the firmware) or `Chip` (read from the chip identity record).
- The desktop (`Session::identity`) keeps it for the connection only and clears it at every session boundary. A device that did not negotiate the capability is the legacy identity: product `pala`, board `c3-supermini`, revision unknown, source "legacy" (a desktop-side value, never on the wire). Numbers resolve through `catalog::by_wire`; one the catalog does not know is shown as "Unknown product (n)" or "Unknown board (n)" and never guessed: the session negotiates nothing else for that device and the flashing guard refuses it.
- Firmware build identity: `firmware/<product>/build.rs` reads the product and board numbers from `products/registry.toml` (and the image's minimum protocol from `product.toml`) into compile-time `env!` values, so no number is typed twice. The physical build advertises the capability.
- Chip identity record (`kivori_protocol::identity`, `no_std`): 12 bytes, little-endian: format version (1), product (`u16`), board (`u16`), hardware revision (`u8`, 0 = unspecified), 2 reserved zero bytes, CRC-32 (the frame CRC) over the first 8. A region of all zeros is "not provisioned" (fresh eFuses read zero). Firmware reads it from the C3 user eFuse block (BLOCK3, `BLOCK_USR_DATA`) with `esp_hal::efuse::read_field_le`; this step never writes eFuses. An unprovisioned or invalid record means source `Build` with no revision.
- Wrong-board boot check (tier A, ADR-0016): a valid record whose product or board differs from the build identity makes the firmware run no product. It shows the static "Wrong firmware for this Kivori" screen (`render_wrong_board`; only the recovery hold is drawn above it), advertises only `PRODUCT_IDENTITY_V1`, and reports the chip's identity with source `Chip`, so the app can see what the unit is and reflash it. The recovery hold still reboots it (invariant 24).
- Image identity record: the firmware image carries `KIVORI-FW-IDENTITY:p=<product>;b=<board>;v=<x.y.z>;m=<major.minor>\0` next to its version marker (`firmware_marker::scan_image_identity`). The desktop `build.rs` fails the build if the embedded image's record differs from the catalog (product, board, firmware crate version, minimum protocol).
- Flashing guard (tier A): before `flash_firmware` reserves the port, the image record is compared with the connected device's identity (legacy counts as `pala` on `c3-supermini`); a mismatch, or a device whose numbers the catalog does not know, is refused with `FlashFailure::WrongProduct` and both names in `FirmwareStatus.wrong_product`, and nothing is written. Recovery has no handshake, so `decide_recovery` picks from the catalog entries listing the port's USB ids: one product is used; several end in `FlashFailure::ChooseProduct` with `restore_choices`, and `restore_firmware(product)` is sent again with the user's pick, which must be the bundled image's product or it is refused as `WrongProduct`.

### Quiet Unverified (protocol 1.6, `QUIET_ACK_V1`)

- `FeedbackKind` gains `Triggered` (5), appended after the original five. A trigger-only outcome (media key, shortcut, or a macro whose least-confirmed step is one) is `Triggered`; a state action that could not read its result back stays `Unverified`. In a macro `Unverified` ranks below `Triggered`, so the amber badge wins.
- The desktop sends `Triggered` only when `QUIET_ACK_V1` was negotiated (`Session::send_feedback`); otherwise it sends `Unverified`, exactly as before.
- The device shows `Triggered` for 400 ms: the keycap press and a brightened legend, no badge, never a success look, and `buddy_face` never celebrates for it. Activity and Test report it as Unverified.

### Idle life and brightness (protocol 1.6, `IDLE_LIFE_V1`, ADR-0012)

- One appended message: tag 21 `BuddySettings { session, personality, self_play, brightness }` (desktop to device). The device applies it only from the accepted session with the capability negotiated, and keeps the last values until it restarts. Desktop sends it on every session start and whenever the personality, reactions or brightness change (reactions are sent off while the screen is locked).
- The firmware runs the idle moments itself (`kivori_model::IdleLife` beside `MascotAnimator`; the runtime polls it every tick). Moments: blinks and the slow gaze drift (pure functions of time in `MascotAnimator::pose_at`), glances left or right (`MascotAnimator::glance`, Idle and Happy only; every 3-6 s playful, 5-10 s cozy, 9-15 s calm; 1.4 s eased, 2 to 4 px by personality), the buddy's own reactions (`SelfPlay`, the same scheduler the desktop director wraps: calm 18-30 s, cozy 8-16 s, playful 4-8 s, only in Idle and Happy) and a look towards a touched control (`MascotAnimator::look_at`, from the knob turn or switch going down and each button going down, so it precedes the action and its feedback; eased in over 150 ms, held 700 ms after the last touch, eased out over 300 ms). Late evening (21:00 to 04:59 on the local clock from `Status`; no clock, no evening) stretches the blink from 160 ms to 400 ms. The eye layer only moves (`eye_offset_q8`, the existing eye frames), and outside Sleeping the eye scale never drops below 48/256.
- Deterministic: the integer timebase plus a seed. A unit seeds from its device id (`device_id::idle_seed`, the 16 bytes folded to 32 bits), so units differ and a unit repeats; Device Studio and golden frames use `DEFAULT_IDLE_SEED`. A state change cancels a glance, look or reaction (the pose eases from where it was). Reactions off stops the buddy's own reactions only; glances and looks are not reactions. Until the first `BuddySettings` the device plays no reactions of its own, so a host without the capability never gets two schedulers.
- Without the capability (older firmware) the desktop `CompanionDirector` keeps scheduling self-play and sending `PlayMascotAction`, as before. With it the director is not polled.
- Brightness: five steps (`BRIGHTNESS_STEPS`); step 1 is 25 % duty and is still clearly on, step 5 is the old always-on. GPIO8 is a strapping pin sampled at reset, so the physical firmware keeps it a plain output while the panel initialises and hands it to LEDC (24 kHz, 8-bit, low-speed channel 0) afterwards. There is no time-based dimming. The step lives in the config (`display.brightness`, default 5, version 1 with `#[serde(default)]`), is set by `set_brightness` and applied at once.

### Microphone state (protocol 1.6, `MIC_STATE_V1`, ADR-0011)

- One appended message: tag 22 `MicState { session, mic }` (desktop to device), `mic` being `MicState { muted: Option<bool>, in_use: Option<bool> }` (`kivori-model::desk`). `muted` is `Some(true)` only when every active capture device reads muted, `Some(false)` when any reads live or has no mute control, and `None` when that cannot be told (a device could not be read, or there is none). `in_use` is `None` where the OS does not say. `None` is never rendered as muted or as "not live". `Status` stays a fixed struct (ADR-0008), so this is its own message.
- The desktop sends it when the state changes, and with each status refresh while it is known, only with the capability negotiated; the device keeps the latest of the accepted session and forgets it at every session boundary.
- The device draws the mic indicator in the top slot (priority over master mute and media): a slashed microphone when muted, a red microphone only when `in_use` AND unmuted ("Live"), nothing otherwise. The resting buddy wears the covered-mouth `MascotExpression::MicMuted` (eye frame 1, mouth frame 11, art `mouth-micmuted`) in Idle only, after Error and celebrate and before strained, master mute, listening and attentive.
- `MicBackend` (`platform`): `devices`, `set_mute` (write every device, read each back), `in_use`, `try_recv_change`. The write returns the per-device read-back, so StateConfirmed stays structurally honest. The `Mic Mute` action (`Action::ToggleMicMute`, token `micMute`, wire kind `Mute`) mutes all unless all are already muted; Confirmed only if every device reads muted, Error if one stays live or has no mute control, Unverified if one cannot be read back. Device names are user data: they appear in the app (`mic.devices` in the desk status) and never in the activity log, which records only the action token.
- Windows: one `kivori-mic` thread owns COM, `IAudioEndpointVolume` per active capture endpoint with an `IAudioEndpointVolumeCallback` (own writes carry a Kivori event context, so a user's change is External) and an `IMMNotificationClient` that rescans when a device arrives (it is muted too while mic mute is on). In use reads the consent store; any failure is unknown. macOS: `kAudioDevicePropertyMute` on the input scope per non-aggregate, non-hidden device with input streams (`kAudioHardwarePropertyDevices`), HAL listeners for mute and for the device list, `kAudioDevicePropertyDeviceIsRunningSomewhere` for in use (it can read true on a duplex device that is only playing). A device without a settable mute property is `Unsettable`: the action fails for it, with no fallback to input volume 0. Other targets report `NotImplementedYet`.
- Default binding: the left button in the Zoom and Teams profiles (replacing the call app's own mute shortcut), so the mic mute is the OS microphone, not the app's button.

### Device internals (protocol 1.6, `DEVICE_INTERNALS_V1`, ADR-0013)

- Two appended messages: tag 19 `InternalsRequest { session }` (desktop to device) and tag 20 `Internals { session, DeviceInternals }` (device to desktop). The device answers a request from the accepted session with one reply, only when the capability was negotiated; anything else is dropped unseen. `Health` is unchanged.
- `DeviceInternals` (`kivori-model::internals`) is fixed-size and every field is an `Option`, so a value the device cannot measure is `None` on the wire and "Unavailable" in the card, never 0. Fields: flash (`app_image_bytes`, `app_partition_bytes`, `asset_blob_bytes`, `asset_blob_cap_bytes`, `flash_total_bytes`), RAM (`ram_total_bytes`, `ram_static_bytes`, `ram_free_bytes`, `stack_free_min_bytes`), load and display (`busy_percent`, `compose_ms`, `flush_ms`, `fps`), chip (`chip_model`, `chip_revision`, `temperature_decicelsius`, `uptime_ms`, `reset_reason`) and input and link (`edge_queue_high_water`, `dropped_edges`, `invalid_transitions`, `frames_in`, `frames_out`, `frames_rejected`). Counters are `u32`, wrap, and reset at boot. `ChipModel` and `ResetReason` are append-only enums.
- Firmware (`internals.rs` holds the pure parts, `chip_probe.rs` the physical-only reads). The run loop owns busy %, fps, compose and flush time (`LoadMeter`, one-second windows) and the counters; the board fills the rest through a `ChipProbe` that `run_buffered` installs.
- The desktop asks with `Session::request_internals` (nothing is sent without the capability), caches the latest reply in `Session` (connection-scoped: cleared by `open` and by any session boundary), and the device thread publishes it with the diagnostics snapshot (at once when it changes). `get_device_internals` asks for a fresh snapshot and returns the cached `DeviceInternalsDto`; the card polls it every 1 s while mounted (developer mode on, window visible), so the device is asked only then.

### Host takeovers (protocol 1.5, `HOST_TAKEOVERS_V1`)

- `ByeReason` gains `HostSleeping` (3) and `FirmwareUpdate` (4), appended after the original three. The desktop sends them only when `Session::supports_host_takeovers()`; an older device never sees them. `Session::close(link, reason)` writes the `Bye` and ends the session. The desktop sends `Bye(HostSleeping)` on suspend (see Host presence) and `Bye(Shutdown)` on Quit; the flash sends `Bye(FirmwareUpdate)` first (see Firmware update and recovery).
- `Bye(HostSleeping)`: the device shows Sleeping and keeps showing it across the link loss that follows (`DeviceEvent::HostSleep` latches it). A new `Hello` clears the latch. Without the capability negotiated, the reason acts like any other `Bye` and the device goes Offline.
- `Bye(FirmwareUpdate)`: the device drops to Offline and the runtime renders the Updating screen (`render_updating`; below recovery, above the pose). It lapses after 120 s (`UPDATING_MAX_MS`) so it cannot stick if the host dies, and a new session ends it at once.
- Host-silence timeout: with an accepted session and no valid inbound frame for 4 s (`HOST_SILENCE_MS`), the firmware calls `link_lost` and goes Offline. The desktop pings every second (`HEARTBEAT_INTERVAL`), so a killed or crashed desktop never leaves a frozen "connected" frame. This applies to every session, whatever was negotiated.
- The firmware version comes from the firmware crate's own `Cargo.toml` (1.3.0) and is also embedded as `KIVORI-FW-VERSION:<ver>` text in the image.

### Messages (tag = postcard variant index)

| Tag | Message | Dir | Payload | Capability |
|---:|---|---|---|---|
| 0 | `Hello` | D to V | desktop_version, desktop_caps, nonce | |
| 1 | `HelloAck` | V to D | device_caps, device_id, firmware_version, nonce_echo | |
| 2 | `Ready` | D to V | negotiated_minor, negotiated_caps | |
| 3 | `Bye` | both | reason (`IncompatibleVersion`, `Shutdown`, `ProtocolError`, `HostSleeping`, `FirmwareUpdate`) | `HOST_TAKEOVERS_V1` for the last two |
| 4 | `SetState` | D to V | desired (`SendableState`), at_ms (optional) | |
| 5 | `StateReport` | V to D | reported (`CompanionState`), elapsed_ms | |
| 6 | `Ping` | D to V | t_ms | |
| 7 | `Pong` | V to D | t_ms_echo, uptime_ms | |
| 8 | `Health` | V to D | free_bytes | |
| 9 | `Diagnostic` | V to D | category, code | |
| 10 | `Error` | both | category, code | |
| 11 | `PlayMascotAction` | D to V | action, personality, seed | `MASCOT_INTERACTION` |
| 12 | `MascotActionApplied` | V to D | action, personality, seed, applied_at_ms | `MASCOT_INTERACTION` |
| 13 | `InputEvent` | V to D | session, gesture_id, control (`Rotary`), kind, device_ms | `PHYSICAL_INPUT_V1` |
| 14 | `Presentation` | D to V | session, revision, primary, value (optional), transient_ms | `PRESENTATION_V1` |
| 15 | `Status` | D to V | session, `DeskStatus` | `DESK_STATUS_V1` |
| 16 | `Feedback` | D to V | session, action, kind (`Processing, StateConfirmed, ExecutionConfirmed, Unverified, Error, Triggered`) | `ACTION_FEEDBACK_V1`, `QUIET_ACK_V1` for `Triggered` |
| 17 | `MediaInfo` | D to V | session, `Option<MediaInfo>` (title, artist) | `MEDIA_INFO_V1` |
| 18 | `ControlLabels` | D to V | session, `ControlLabels` (rotate, press, hold) | `CONTROL_LABELS_V1` |
| 19 | `InternalsRequest` | D to V | session | `DEVICE_INTERNALS_V1` |
| 20 | `Internals` | V to D | session, `DeviceInternals` (every field optional) | `DEVICE_INTERNALS_V1` |
| 21 | `BuddySettings` | D to V | session, personality, self-play on/off, brightness step (1 to 5) | `IDLE_LIFE_V1` |
| 22 | `MicState` | D to V | session, `MicState` (muted and in use, each optional) | `MIC_STATE_V1` |
| 23 | `Identity` | V to D | session, product (`u16`), board (`u16`), hw_rev (optional `u8`), source (`Build`, `Chip`) | `PRODUCT_IDENTITY_V1` |

`InputEvent.control` is `Rotary`, `Button` or `ContextButton(0..3)` (the three contextual buttons, left to right; needs `CONTEXT_BUTTONS_V1`; `Press` and `Hold` only, an index of 3 or more is rejected). With `Rotary`, `kind` is `GestureStarted`, `Detent(Cw|Ccw)` or `GestureEnded`; with `Button` (needs `BUTTON_INPUT_V1` too), it is `Press`, `Hold` or `DoublePress` (needs `DOUBLE_PRESS_V1`), sent once per gesture. The desktop rejects a kind that does not belong to its control. `Presentation.primary` is `Idle, Active, Error, Unknown`. `value` is `{ kind: Volume, current_percent, confidence, at_boundary }`.

`DeskStatus` (`kivori-model::desk`) is `{ mode, clock, volume_percent, muted, media, cpu_percent, ram_percent, high_load }`; every value is an `Option` and `None` is rendered as unknown. `mode` is `Buddy, Clock, Volume, Media, System`; `media` is `Playing, Paused, Stopped`. `Feedback.kind` is `Processing, StateConfirmed, ExecutionConfirmed, Unverified, Error` and `action` is `Volume, PlayPause, Mute, Shortcut, Launch`. `MediaInfo` text is `MediaText`: at most 32 ISO-8859-1 characters (what the device's Latin-1 font can draw), sanitized on the desktop (other characters become `?`, control characters dropped, a cut ends in `~`). `ControlLabels` holds one `MediaText` per control (rotate, press, hold, and the three contextual buttons, empty until that hardware lands), derived on the desktop from its live bindings (`Bindings::labels`) and sent once per session and again when a binding changes; an empty label means "draw nothing", never a guess. Status, feedback, media info and control labels are session-scoped like `Presentation`: the device drops any from another session and forgets them all on every session boundary.

| Bit | Capability |
|---:|---|
| 0 | `MASCOT_INTERACTION` |
| 1 | `PHYSICAL_INPUT_V1` |
| 2 | `PRESENTATION_V1` |
| 3 | `BUTTON_INPUT_V1` |
| 4 | `DESK_STATUS_V1` |
| 5 | `ACTION_FEEDBACK_V1` |
| 6 | `DOUBLE_PRESS_V1` |
| 7 | `MEDIA_INFO_V1` |
| 8 | `CONTROL_LABELS_V1` |
| 9 | `CONTEXT_BUTTONS_V1` |
| 10 | `HOST_TAKEOVERS_V1` |
| 11 | `QUIET_ACK_V1` |
| 12 | `DEVICE_INTERNALS_V1` |
| 13 | `IDLE_LIFE_V1` |
| 14 | `MIC_STATE_V1` |
| 15 | `PRODUCT_IDENTITY_V1` |

`Capabilities` is a `u32` set in `kivori-model`. Bits are allocated centrally and never reused.

## 3. Desktop and webview IPC

The webview may call only these commands and listen to these events. No command exposes raw serial bytes, port choice, filesystem, shell, environment, raw `device_id`, or the network. DTOs carry no internals (device id is a short hash only). TS wrappers live in `apps/desktop/src/lib/ipc/`.

| Command | Purpose | Build |
|---|---|---|
| `get_app_info` | App version, protocol version, supported majors, Device Studio flag | all |
| `get_connection_status` | Connection, desired, reported, device info, retry count, connection generation, negotiated mascot flag, last mascot action, `host` (`active, locked, sleeping`) | all |
| `list_states` | Companion states | all |
| `set_desired_state` | Set `desired`; sends `SetState` when connected | all |
| `get_onboarding`, `complete_onboarding`, `restart_onboarding` | First-run state (`completed`), finish it, reopen it ("Run setup again"); see First-run onboarding | all |
| `get_accessibility`, `request_accessibility`, `open_accessibility_settings` | macOS Accessibility status (`granted, missing, notApplicable`), the system prompt, and the settings pane; see First-run onboarding | all |
| `get_startup_settings`, `set_launch_at_login` | Launch-at-login state and switch (`startup://changed`); see Launch at login and tray | all |
| `get_update_status`, `check_for_updates`, `install_update`, `snooze_update`, `skip_update_version`, `set_auto_update_check` | App updater status (`UpdateStatusDto`, `updater://status`), "Check now", "Install & restart" / "Try again", "Later", "Skip this version", and the automatic-check switch (`enabled`); see App updates. No URL crosses IPC | all |
| `get_config` | Saved settings: `version`, `revision`, `notice`, `profiles` (every profile as resolved, each slot with `deviceLabel` and `overridden`), `display`, `buddy` | all |
| `set_display_settings` | `defaultView`, `secondaryView` (a view, or `cycle`); returns the config | all |
| `set_buddy_settings` | `reactions`, `intensity` (`low, normal, high`); returns the config | all |
| `set_binding` | `profile`, `control` (`press, hold, button1Press, button1Hold, button2Press, button3Press, button3Hold`), `slot` (a replacement slot, or `null` to reset it); `button2Hold` is refused (reserved for the profile pin); returns the config | all |
| `set_rotate` | `profile`, `rotate` (`systemVolume` or `shortcuts`, or `null` to reset); returns the config | all |
| `reset_profile` | `profile`; a built-in drops every override, including its app changes; a custom profile gets a fresh copy of General's bindings (name and apps stay); returns the config | all |
| `add_custom_profile` | `appId`, `name` (the app's display name, may be empty); creates a custom profile (at most 16) as a one-time copy of General's bindings and moves the app into it; returns the config | all |
| `assign_app` | `profile` (`general` = out of the profile that lists it), `appId`; one app belongs to one profile, so this moves it; returns the config | all |
| `rename_profile` / `delete_profile` / `set_profile_pin_cycle` | custom profiles only (`profile`, plus `name` or `enabled`); a name is 1 to 32 characters; returns the config | all |
| `list_running_apps` | none; apps with a window open now (`id`, `name`), run off the UI thread; empty where the OS gives none | all |
| `get_app_icon` | `appId`; a PNG data URL or `null` (the UI shows a letter); off the UI thread, cached per id. macOS: `NSWorkspace iconForFile` re-encoded by `NSBitmapImageRep`; Windows: `ExtractIconExW` of the running executable | all |
| `reset_config` | Every setting back to its default; returns the config | all |
| `save_macro`, `delete_macro` | Create or replace a macro by id; delete one (refused while a binding names it); returns the config | all |
| `play_mascot_action` | `greet, pet, tickle, surprise, comfort` | all |
| `get_activity_log` | Newest N activity records | all |
| `get_diagnostics` | None; returns `DiagnosticsDto` (see Diagnostics) | all |
| `get_device_internals` | None; asks the device for a snapshot and returns `DeviceInternalsDto` (see Device internals) | all |
| `get_firmware_status`, `flash_firmware`, `restore_firmware` | Firmware update status, update request and recovery request (no port or path; `restore_firmware` takes an optional product id, only after the status asked the user to choose) | all |
| `get_firmware_offer`, `accept_firmware_offer`, `dismiss_firmware_offer` | The firmware update offer (`FirmwareOfferDto`, `firmware://offer`), "Update now" (queues the same flash as `flash_firmware`) and "Later" (optional offers only); see Firmware flashing from the app | all |
| `render_preview_frame`, `open_preview_stream`, `update_preview_stream`, `ack_preview_frame`, `close_preview_stream` | Native-rendered RGBA frames for Device Studio | `device-studio` only |
| `mirror_state` | Dev-labelled `set_desired_state` | `device-studio` only |
| `get_desk_status` | Display mode, monitored values (`null` = unknown), Press/Hold bindings, last action outcome | all |
| `set_display_mode` | `buddy, clock, volume, media, system` | all |
| `list_action_catalog` | Every bindable action with `slot`, `scope`, `verification`, `params`, `availability`, `reason`, `runsWhenProtected`. Availability is the device thread's latest backend snapshot (system volume and per-app volume); until its first publish, and for key input, it is what the OS is expected to provide | all |
| `test_action` | `action` (an `ActionSpec`, validated natively like a saved binding); runs it now through the device thread, exactly as if its control fired. Returns nothing: the outcome arrives in `desk://status` `lastAction`. A protected foreground refuses everything but system actions | all |
| `test_rotate` | `rotate` (a `RotateSpec`, validated natively). Tests the knob once: a volume knob (system or one app) steps one detent up and back to exactly where it was (down first when already at the top), so the net change is zero; a shortcut pair sends its clockwise shortcut once. Same outcome path and Protected rule as `test_action` (the shortcut is refused, a volume nudge injects no keys and runs). The UI counts down 3 seconds first for a shortcut pair | all |

| Event | Payload | When |
|---|---|---|
| `connection://status` | `ConnectionStatusDto` | Any change to connection, desired or reported |
| `activity-log://event` | `ActivityEventDto` | A new activity record |
| `desk://status` | `DeskStatusDto` | Any change to the desk projection |
| `config://changed` | `ConfigDto` | After every successful save or reset |
| `startup://changed` | `StartupSettingsDto` | After launch at login changes, from the tray item or `set_launch_at_login` (the OS entry's state) |
| `updater://status` | `UpdateStatusDto` | Any updater state change (throttled to about 7 per second while downloading) |
| `updater://show` | none | The tray's "Update to vX.Y.Z…" was clicked: show the update dialog |
| `firmware://offer` | `FirmwareOfferDto` | The firmware update offer changed |
| `firmware://show-offer` | none | The tray's "Update Kivori firmware…" was clicked: show the firmware offer dialog |

- Events are small JSON. Frame bytes travel as raw RGBA8888 (240x240, no JSON or base64) through a `tauri::ipc::Response` or a `Channel<ArrayBuffer>`. The canvas only blits. RGB565 to RGBA happens in Rust.
- Dev-only commands are compiled out of release builds, and so is the Device Studio route.
- Preview animation input: `{ initialState, events: [{ atMs, state }], actionEvents: [{ atMs, action, personality, seed }] }`, at most 256 events, integer milliseconds. Supplying it selects externally clocked playback, which supports pause, step and backward seek. Streaming keeps at most two in-flight frames and drops frames that were superseded while rendering.
- `connectionGeneration` changes whenever device uptime may have reset. Device Studio maps applied-action acknowledgments onto its own monotonic clock per generation.

## 4. Timing, rendering and assets

### Timing (ADR-0003)

- Canonical time is integer `elapsed_ms: u32`. The renderer never sees wall-clock time or floats.
- Device Studio keeps an integer step index `n` and derives `ms = (n * 1000 + 15) / 30`. It never accumulates a float, so it has no drift.
- Per-scene frame selection: `floor(elapsed_ms * num / (1000 * den)) mod frame_count`. The 30 FPS step is only an inspection cadence.
- Implementation: `crates/kivori-model/src/timeline.rs`. Negative indices clamp to 0 and results saturate at `u32::MAX`.

### Rendering path

- `render_scene(blob, scene, elapsed_ms, tile)` composites into an RGB565 tile with integer math only. Stitched tiles must equal the full-frame output.
- Without a frame buffer (the host-sim `Runtime::new` path) the firmware renders through a small tile buffer. On the physical ST7789 it stages one full RGB565 pose (115,200 bytes, 112.5 KiB) in static SRAM, then sends only changed 40x40 tiles by SPI DMA. Composing and hashing never overlap the transfer. Tile hashes (`kivori-framebuffer`) skip unchanged tiles.
- The unwired TE pin means no refresh sync, and this is not panel double buffering.
- Golden frames (`tests/golden-frames`) pin frame hashes across OSes. An intentional pixel change updates the goldens in the same change.

### Assets (ADR-0004)

- SVG is a build input only. `tools/asset-compiler` (pinned `resvg`/`tiny-skia`, stable ordering, no timestamps) writes a byte-reproducible blob. CI recompiles and diffs its hash.
- Blob: fixed header (magic, `format_version` = 2, offsets, hash), a postcard structured section (scenes, layers, keyframes, tables) loaded into bounded `heapless` structures, and a data pool read zero-copy from flash.
- Format v2: sprites are cropped RGB565 with an optional packed alpha4 pool. Layer roles: `Static, Body, Eyes, Mouth, Cap`. Readers reject v1 blobs. The mascot pack is capped at 128 KiB at build time.
- Bitmap fonts are compiled into the blob, so text does not depend on host fonts.
- Layers use integer keyframe transforms and sprite-frame selection. Generic layers are step-held. The mascot uses Q8 fixed-point transforms and cubic smoothstep.

### Mascot: the keycap buddy

- Layers, in draw order: `Body` (the fixed base: side walls and front lip), `Cap` (top face, upper wall wedges, legend), both eyes, then the mouth. Base and cap are shared by all six states. One eye texture serves both eyes.
- `face::buddy_face(state, view)` (`kivori-model`, #13) maps observed desk truth to a face override (Error, celebrate, MicMuted, Strained, Muted, Listening, Attentive; rules in product.md). The firmware applies it to the pose on the Buddy view only. Faces are eye/mouth sprite frames compiled from `assets/mascot.svg`; the blob is ~127.3 KB (127,253 bytes) of its 131,072-byte cap; the covered-mouth face added one 24x13 mouth frame (780 bytes) with no trimming, but a further face needs art or crop trimming first.
- A pose's `press_q8` sinks the cap and the face printed on it while the base stays put, so the walls visibly shorten. Happy holds a 16 px press.
- `MascotAnimator` (in `kivori-model`) resolves blinking, per-state motion, and eased transitions (350 ms, 600 ms into sleep). Expression changes are hidden inside a blink. Firmware keeps the animator between state changes and renders through the same compositor as Device Studio.
- Volume overlay room: `MascotPose::with_overlay_room` scales the body by 0.72 (184/256) and lifts it 34 px so the keycap sits above the bar. The switch is instant, not eased.

### Companion and personality rules

- Personalities: `cozy` (default), `playful`, `calm`. They scale motion amplitude and the self-play interval (calm 18-30 s, cozy 8-16 s, playful 4-8 s). They never change the reported state.
- Reactions: `greet, pet, tickle, surprise, comfort`. Self-play runs only in `idle` and `happy`: on the device for firmware with `IDLE_LIFE_V1` (`IdleLife`), in the desktop companion director for older firmware. Device Studio reactions are forwarded to a connected device that negotiated `MASCOT_INTERACTION`.
- Personality never looks like a desktop state. No reaction may use the same `(eye_frame, mouth_frame)` as any state face. A test enforces this.
- A state change interrupts a running reaction.
- Reactions over `busy`, `booting` and `offline` are refused (`CompanionState::allows_reaction`). Firmware sends no `MascotActionApplied` for a refused one, so the desktop never assumes it played. A poke over `sleeping` gives a sleepy `Affectionate` response, then returns to sleeping.
- Higher layers win. When v1 adds Success and Error primary states, rerun the face-uniqueness check.

## 5. Rotary volume loop

```text
HW-040 A/B/SW -> InputSource port -> QuadratureDecoder -> RotaryGesture -> InputEvent (tag 13)
  -> wire -> Session decode -> InputIngress -> resolve_binding -> apply_step
  -> GestureValue -> VolumeBackend -> PresentationResolver -> Presentation (tag 14)
  -> wire -> firmware expires value after transient_ms -> render_volume_overlay -> panel / preview
```

Everything between the two hardware adapters (`PhysicalRotary` in firmware, the Windows backend on the desktop) is a pure function or state machine and is host-testable.

- Input port: `InputSource::sample() -> { a, b, sw }`. `PhysicalRotary` only reads and inverts the pins. All three lines are active-low with internal pull-ups. Pins: CLK GPIO4, DT GPIO0, SW GPIO1 (the push switch, read since M1). GPIO9 is avoided because it is the BOOT strap. Power the HW-040 from 3V3, not 5V. This pin map is a specification until a dated physical check closes it (see validation.md).
- Decoder: a Gray-code state machine on phase `(a << 1) | b`, resting at `00`. It emits a `Direction` only when the knob returns to rest after four quarter-steps in one sense. The accumulator resets at every rest arrival. Bounce and partial motion emit nothing. An impossible transition (both bits change) is counted as `invalid_transitions`, resets the accumulator, and is a diagnostic only.
- Gesture: the first detent opens a gesture (`gesture_id`, unique within the session, never 0). 250 ms without a detent (`GESTURE_END_MS`) emits exactly one `GestureEnded`. A direction reversal does not split a gesture.
- Ingress: `InputIngress` rejects an event with a stale session (`StaleSession`). It also requires an observed `GestureStarted` before a `Detent` or `GestureEnded` (`UnknownGesture`), as defence in depth.
- Freshness (#26, `input/freshness.rs`): `Session` turns each `Pong` into a device-to-host clock offset (`uptime_ms` plus half the round trip, against the host time the `Pong` was read; the latest sample with a round trip of at most 500 ms wins, wrapping `u32` arithmetic throughout) and clears it on every session boundary. `RotaryPipeline::accept_inputs_fresh` runs after the session and gesture checks: a discrete `LogicalInput` (`Press`, `Hold`, `DoublePress`, `ButtonPress`, `ButtonHold`) whose age (host receive time converted to device time, minus `InputEvent.device_ms`) exceeds `MAX_ACTION_AGE` (750 ms) is dropped and logged as `InputStale` (no payload), with no device feedback, like other rejected input. Detents and gesture boundaries bypass the check. With no offset yet there is no age and nothing is dropped. The age left over (`remaining`) becomes the job's deadline in `ActionWorker`, whose single queue a slow launch can stall: a job dequeued past its deadline is skipped, logged as `InputStale` and its pending feedback cancelled. A stale action is dropped before `DeskRuntime::on_input`, so a focus or Protected change during a backlog cannot send it to the new app. Knob-shortcut detents carry no deadline.
- Binding and step: one global binding, rotate to master volume (`action::resolve_binding`). `apply_step` moves 2 points per detent (`BASE_STEP_PERCENT`), clamps to 0-100 and flags a boundary.
- `GestureValue` (preview to confirmed):
  - While a gesture is open it owns the displayed value. Each detent applies locally and is shown as `Preview`, even if the backend set succeeded.
  - At `GestureEnded` the desktop reads the value back and reconciles the display to it as `Confirmed`. Desktop truth wins.
  - An external change during a gesture is recorded but not drawn over the preview. An endpoint switch marks the gesture abandoned so it is not retargeted.
  - `Unverified` means the set was dispatched but the read-back failed.
- `VolumeBackend` trait: `availability`, `read`, `set` (returns the OS read-back value, so `StateConfirmed` is honest).
  - Windows: Core Audio `IAudioEndpointVolume` on the default render endpoint (`eRender` + `eConsole`). A dedicated `kivori-audio` thread owns COM and both callbacks (`IAudioEndpointVolumeCallback`, `IMMNotificationClient`). Callbacks only post to a channel. Kivori writes carry a Kivori GUID, so its own echo is ignored while external changes and endpoint switches are applied.
  - macOS: CoreAudio (see Chosen platform APIs). It has no event-context GUID, so a notification is tagged `Kivori` when it matches Kivori's own write within 250 ms.
  - Linux: `NotImplementedYet { target }`. A deterministic `FakeVolumeBackend` runs all host tests.
- `AppVolumeBackend` trait (per-app volume and mute): `availability`, `read(app)`, `set(app, pct)` (returns the read-back), `read_mute(app)`, `set_mute(app, muted)`, and `AppVolumeError::{NoSession, ReadBackUnavailable, Os}`. `app` is the lowercase foreground-style id (Windows: the executable file name, `spotify.exe`).
  - Windows: the same `kivori-audio` thread, through `Command::App*` arms (`platform/windows/app_volume.rs`). It takes the default render endpoint, `IAudioSessionManager2::GetSessionEnumerator`, then per session `IAudioSessionControl2::GetProcessId` and `QueryFullProcessImageNameW` for the lowercase exe file name (a session it cannot inspect, such as an elevated process, is skipped). Every matching session is written with `ISimpleAudioVolume::SetMasterVolume(level, &KIVORI_EVENT_CONTEXT)` and read back; the result is confirmed only if all read-backs agree, otherwise `ReadBackUnavailable` (Unverified). No session, or no endpoint, is `NoSession`.
  - macOS: `Unsupported { reason: "Per-app volume isn't available on macOS" }`; Linux: `NotImplementedYet`. `FakeAppVolumeBackend` runs all host tests.
  - Keep three ideas apart: `BackendAvailability` (implemented or not), `ConfirmationClass` (`StateConfirmed`, `ExecutionConfirmed`, `TriggeredUnverified`), and the derived `ActionAvailability`. An OS-level `PlatformCapability` is not a type: "this OS cannot do it" is `ActionAvailability::Unsupported { reason }`, decided by the action catalog.
- Presentation: `PresentationResolver` is a pure function from `ProductSnapshot` to `Presentation`. `value` is a transient overlay of 800 ms (`VALUE_TRANSIENT_MS`). Firmware expires it locally, so the overlay clears without a host timer. Firmware stores `primary` but does not render it; a failed volume write is shown through `Feedback { Volume, Error }` instead (section 5b).
- Overlay: `render_volume_overlay` (`kivori-renderer/src/overlay.rs`) draws a solid-rect bar with no glyphs. `Confirmed` is a solid fill, `Preview` is hollow (top and bottom rows only), and the track outline turns white at a boundary. It is composited over the mascot in the same tile pass.
- Firmware owns raw input truth (conditioning, detents, gestures). The desktop owns action meaning.
- Edge capture: every edge on CLK, DT or SW raises the GPIO interrupt, whose handler only stores a timestamped level snapshot in a 128-entry queue (`physical_rotary.rs`). The run loop drains it each tick (`InputSource::drain`), so quarter-steps and switch edges during a frame compose or flush are not lost. On overflow new edges are dropped: the decoder counts one invalid transition, which can lose a detent but never invents one.
- Latency: the firmware renders on the tick a `Presentation` or `Feedback` arrives instead of waiting for the 33 ms frame cadence, and the desktop device thread blocks on serial bytes (bounded by its 50 ms tick) instead of sleeping. Row 3.14 measures the result.
- Every `SetState` is answered with a `StateReport` of the current state, changed or not, so a reconnecting desktop always learns it.

## 5b. Push switch, recovery hold and desk

```text
SW edge -> InputSource::drain -> ButtonGesture (debounce 20 ms, edge-timed)
  Press (< 500 ms) / Hold (500 ms - 2 s) -> InputEvent{Button} -> InputIngress -> Bindings
  -> ActionWorker (kivori-actions thread) -> Outcome -> FeedbackLadder -> Feedback (tag 16)
  2 s: recovery takeover (time-based progress) ... 10 s from key-down: Bye, software_reset
desktop 1 Hz: CPU/RAM, volume/mute, media, local time -> StatusPublisher -> Status (tag 15)
```

- `ButtonGesture` (`firmware/.../input/button.rs`) is pure and host-tested. Times are measured between debounced edges at the moment the switch first changed, and a release still inside its debounce window stops the hold clock. A press that starts while a rotary gesture is open never becomes Press or Hold (one gesture owns input) but can still recover. While the switch is down, detents are decoded but swallowed (invariant 33).
- The recovery hold needs no host, session or capability (invariant 24). A session boundary drops a half-done press but never a running recovery. On the reboot tick the device renders "Restarting", queues `Bye(Shutdown)` for an accepted session, flushes it for at most 20 ms and calls `esp_hal::system::software_reset`.
- Local acknowledgement: while the switch is down the keycap sinks 8 px (Buddy view) or the screen gets a white frame. This is the device's own < 50 ms acknowledgement, not a confirmation.
- Double press: with `DOUBLE_PRESS_V1` negotiated, a short press waits 250 ms (`DOUBLE_WINDOW_MS`) for a second key-down; two short presses are one `DoublePress`, which the desktop turns into the next display view. A long second press fires the first `Press`, then behaves normally (Hold, recovery). Without the capability a press fires on release with no wait.
- Contextual buttons (GPIO20, GPIO10, GPIO5 to GND, internal pull-ups) are captured by the same edge interrupt and each runs its own `ButtonGesture` with double press off. They never own recovery: a `RecoveryStarted` or `Reboot` from one is ignored, so a long hold fires nothing. One gesture owns all input: a switch going down while any other control is busy (raw or debounced) is ineligible, and knob detents are swallowed while any switch is down.
- Profiles (`desk/profile.rs`, #12): built-in `Profile`s matched by lowercase foreground id. `Context` polls `ForegroundObserver` (`platform/foreground.rs` pure rules; `windows/foreground.rs` Win32 foreground window, input-desktop name and token elevation; `macos/foreground.rs` NSWorkspace via raw Objective-C FFI plus the session lock flag) every 100 ms, commits after 400 ms stable, commits Protected at once, and resolves every input and every `ControlLabels` from one context so labels and bindings never disagree. A knob gesture records its binding (`KnobTarget::{Shortcuts, AppVolume}`) and the focused app at `GestureStarted`; `RotaryPipeline::accept_inputs` hands only system Volume gestures to the volume loop. An App Volume gesture stays in `DeskRuntime`: each detent is an `Action::AppVolumeStep` on the action worker (read, `action::volume::apply_step`, write with read-back), so a fast spin never reaches the batched system-volume path. The detents show nothing one by one: when the gesture has ended and every detent has finished, the device gets one `Feedback { Volume }` at the least-confirmed outcome, or the gesture's single Error (`knob_failure`) if any detent failed. It runs under Protected (it injects no input) and does not follow focus.
- Bindings (`desk/actions.rs`): a `Slot { action: Option<Action>, label: Option<String> }` per gesture; `Bindings` holds Press, Hold and three `ButtonSlots { press, hold }`. `Bindings::default()` is the General profile: Press = Play/Pause, Hold = master mute, Double press = next view (fixed); contextual buttons Press = Previous track, Play/Pause, Next track (media keys, always Unverified), Hold unbound. A button's Hold is bindable, except the middle button's, which `Context::resolve` checks first and keeps for cycling the pin. The device legend shows Press labels only; Hold labels are not on the wire (a later protocol change).
- Layering (`config/resolve.rs`): `resolve(builtins_for(os), &ConfigFile)` takes each built-in `Profile` (`Profile.id` keys the override; names, ids and labels are owned strings) and replaces only the slots an override sets, so untouched controls keep built-in fixes from app updates. A `SlotSpec` replaces the whole slot; `action: null` is an explicit unbind. Validation: labels are 1 to 32 Latin-1 characters (`MediaText`), shortcuts parse with `Shortcut::from_str` and are stored canonical, launch targets pass `launch::validate_target`, `button2Hold` is refused. `ConfigStore` resolves on open (a file that does not resolve is corrupt) and on every save, and caches the `ResolvedConfig`. The override entry is removed when it becomes empty, so a reset leaves no trace.
- Live pickup: after a save the device thread calls `DeskRuntime::apply_config`: `Context::set_profiles` swaps the profiles (the pin and the committed app's profile stay), the display settings are applied, and `sent_controls` is cleared so the next tick re-sends the labels even when they did not change. A knob gesture in progress keeps the keys it recorded at `GestureStarted`.
- Classification (`desk/actions.rs`): mute is State Confirmed only when the OS read-back shows the new state; play/pause, previous, next and a shortcut are always Triggered (the quiet Unverified: the Activity log and Test still say "Unverified"; a matching media state can be stale or caused by something else, so it is never proof; the media indicator and view show what the OS reports); a launch is Execution Confirmed once the OS accepted it (`open -a` exit 0 on macOS, process created on Windows; no shell). A missing permission is Error with `permission_required`, never another mechanism.
- Action catalog (`desk/catalog.rs`): `CATALOG` lists every bindable action with its slot (`rotate` or `discrete`), scope (`system, app, media, keyboard, launch, macro`), verification, params and `runsWhenProtected`; `availability(os, &services)` derives `ActionAvailability` per entry (`list_action_catalog` shows it). Verification maps from `ConfirmationClass`: `StateConfirmed` is `confirmed`, `ExecutionConfirmed` is `started`, `TriggeredUnverified` is `unverified` (a macro is `leastOfSteps`). A table test runs each entry's action against the fakes and holds the entry to what `execute` classifies (macros are checked in their own test). Macros are `Available` on every OS (each step is checked when it runs). App Volume and App Mute are `Available` (confirmed) on Windows and `Unsupported` on macOS; pressing a binding that cannot run is an Error and nothing ever falls back to system volume or system mute. `runsWhenProtected` is `Action::is_system()`: system volume, media keys and mute run in a protected context, shortcuts, launches and knob shortcuts do not.
- Macros (`desk/actions.rs`, `config/{schema,resolve}.rs`): `MacroSpec { id, name, steps }` holds at most 8 `StepSpec`s (`Action { action }`, never a Macro, or `Delay { ms: 50..=2000 }`) and a config holds at most 32. `resolve` turns every macro into an `Arc<Macro>` by id and bindings (`ActionSpec::Macro { id }`, discrete slots only) hold the same `Arc`; an unknown id fails `save` and a stored file with one is corrupt on load (backed up, defaults). `execute_macro` runs on the action worker: steps in order, the least-confirmed outcome wins (Error < Unverified < Started < Confirmed), the first Error stops it and nothing is rolled back. Between steps it stops (Error) when the worker epoch moved (`cancel_pending`: the session ended) and before a shortcut or launch step when the foreground became Protected or is no longer the app the macro started in (invariants 9 and 12); a Delay sleeps in 10 ms slices so a cancel is seen at once. A macro is not `is_system`, so it is suspended under Protected like a shortcut. Its wire `ActionKind` is its first action step's kind (`FeedbackLadder::start_as`) and its UI token is `macro`. The #26 dequeue deadline applies unchanged: a macro longer than the device's 1.5 s Unverified window shows Unverified on the device and its late outcome is only logged. `save_macro` replaces by id (bindings keep working and see the new steps); `delete_macro` is refused while any binding names the macro, so nothing becomes unbound silently. `ConfigDto.macros` carries the specs; the UI edits them in `features/controls/MacroEditor.tsx`.
- Action tokens (`ActionToken`): the desktop-only identity used by `lastAction`, the activity log (`DeskAction { action }`) and the DTO tokens (`volume, playPause, mute, shortcut, launch, previousTrack, nextTrack, appVolume, appMute, macro`). `Action::wire_kind()` maps to the wire `ActionKind` for `Feedback`; the wire gains no variants (firmware's postcard decoding rejects an unknown one), so App Volume (`Volume`), App Mute (`Mute`) and macros reuse an existing kind. Metadata never carries keys, a launch target or an app id.
- Test action: `test_action` becomes `DeviceCommand::TestAction`, which calls `DeskRuntime::test`. It classifies Protected from the live foreground first and refuses a non-system action there (Error, no key sent, never a bypass), otherwise it goes through the same worker, feedback ladder and activity log as a device press.
- `FeedbackLadder` (`desk/mod.rs`): Processing at 500 ms, Unverified at 1.5 s, after which a late outcome is logged but not shown. Timeout is never Error. A newer action replaces the pending feedback; a session end clears it, and requests still queued for the worker are skipped (an epoch counter), so nothing from an old session runs later. A new streak of failed volume writes is shown once as a volume Error.
- `StatusPublisher`: sends `Status` when anything shown changes, when the minute rolls over, or every 1 s (bounded recovery from a dropped initial status); the device advances the clock locally in between. High load is CPU at or above 85 % for 3 samples, cleared below 70 % or when CPU is unknown.
- Device rendering order (`firmware/.../render.rs`, `kivori-renderer::desk`): recovery takeover, else the Buddy pose or the selected desk view, then the volume overlay (except in the Volume view, which shows the value itself), then the chrome: mute and media indicators, the high-load cue (Buddy only), the control legend (Buddy only, never shrinking the keycap: with a contextual button bound, the three button labels over ticks pointing at the buttons under the keycap and the knob label top left; otherwise `TURN` / `PRESS` / `HOLD` captions over the labels; three fixed 80 px columns, a label too wide for its column fades at the edge), the press frame and the feedback badge. Unverified is an amber ring with `?`, never a check mark.
- View switch (`desk::render_views`): for `VIEW_TRANSITION_MS` (320 ms) the outgoing view slides up and the new one pushes in from below (integer ease-out cubic). Each view is drawn into a row slice of the tile (`TileBand::split_at_row` + `scrolled`), so the mascot slides like any view, no extra buffer is needed and tiles still stitch to the full frame. The volume overlay and chrome stay fixed above it.

## 6. Activity log, flashing, offline rule

### Activity log (ADR-0005)

The typed session activity log is the only runtime log.

- Every record is an `ActivityEventDto`: native-issued `id`, ISO-8601 `at`, `summary`, and closed `type`, `severity` (`info, warning, error`), `source` (`connection, action, device, protocol, firmware, config, update`), `outcome` tokens.
- Optional `metadata` is a fixed allowlist: connection state, retry and elapsed counters, diagnostic category and code, versions, short device hash, capability mask, state, personality, action, seed, protocol counters, reported state. There is no free-form map.
- Never logged: raw payload bytes, raw device id, port or file paths, usernames, tokens, secrets, flasher output, raw error text. The UI reads only the fixed fields.
- Delivery: the webview subscribes to `activity-log://event` first, then calls `get_activity_log(limit)`. It dedupes by id (live copy wins), sorts by id, and keeps the newest 256. Nothing is persisted or uploaded.
- The raw device id stays inside the transport layer. Only a short non-reversible hash may appear.
- Raw payload output exists only behind the `debug-payloads` Cargo feature. It is off by default and must not ship.
- Tests: native redaction tests, frontend runtime-cast privacy tests, ordering and 256-entry retention tests.

### Host presence (sleep, lock, console; M3 S2)

- `platform/host_events.rs` is pure. A per-OS source turns notifications into `HostEvent`s (`Suspending, Resumed, Locked, Unlocked, ConsoleDisconnected, ConsoleConnected`) on a channel the device thread drains next to its commands (`HostSignal`, with an ack channel on `Suspending`). `HostPresenceTracker::on_event` returns the actions to take; an event that changes nothing returns none, so Windows' two resume notices and repeated lock events are harmless.
- Sleep: the OS callback sends `Suspending` and waits at most 500 ms (`SUSPEND_ACK_BUDGET`). The device thread writes `Bye(HostSleeping)` (`Bye(Shutdown)` if the device did not negotiate `HOST_TAKEOVERS_V1`) with `Session::close`, drops the link, clears the connection timers, leaves the manager `Disconnected` with no retry count, then acks. macOS answers `IOAllowPowerChange` after the ack or the timeout; `kIOMessageCanSystemSleep` is allowed at once. While asleep, or while another user session has the console, discovery does not run, so the port is never reopened before the machine is really suspended.
- Wake (`Resumed`): the pending retry is cleared and the candidate rotator is reset, so discovery runs on the next tick with no backoff. A resume with no suspend seen is ignored; the heartbeat covers it.
- Lock: not a link event. The orchestrator gets a host override (`Sleeping`) and the buddy's reactions pause. `Orchestrator::desired()` is the effective state (what is sent and resent on reconnect); `user_desired()` is the user's, which the UI shows and which returns on unlock. Changes made while locked wait behind the override. The knob and buttons keep working; the Protected rules in `foreground.rs` already suspend shortcuts.
- Console loss (Windows fast user switching): the link is released without a goodbye so another user's Kivori can claim the port; `ConsoleConnected` rediscovers.
- Input never runs across a sleep: closing the session discards queued input, and leaving `Connected` ends the session boundary in `synchronize_session` (gesture state, queued actions), so anything made before the sleep is rejected as `NoSession`.
- `ConnectionStatusDto.host` is `locked` while the override is set and `sleeping` while the link is released for a suspend. Activity records: `hostSuspending`, `hostResumed`, `hostLocked`, `hostUnlocked` (source `connection`, info).
- If the OS refuses the registration the app logs one warning and runs without it; the device then falls back to its host-silence timeout and shows Offline.

### Launch at login and tray (M3 S4)

- `tauri-plugin-autostart` is used from Rust only: `init(MacosLauncher::LaunchAgent, Some(["--autostart"]))` and `app.autolaunch()`. There is no JS package and no permission in `capabilities/default.json`, so the webview reaches it only through `get_startup_settings` and `set_launch_at_login(enabled)`. The OS entry (Windows `Run` key, macOS LaunchAgent) is the source of truth: nothing is stored in `config.json`, and each command returns what the OS reports after the change. Its own dependencies (`auto-launch`, `dirs`, `os_info`, registry and `smappservice` bindings) have no network crate.
- Hidden start: `tauri.conf.json` creates the window with `visible: false`. `setup` shows it unless `std::env::args()` contains `--autostart` (`window_lifecycle::launched_at_login`); a login launch starts `WindowLifecycle::hidden()` (tray only, a close request is a no-op, `Reactivated` shows the window). The device task runs either way.
- Tray (`runtime/lifecycle.rs`, id `kivori`): a disabled status line (`Connected`, `Not connected`, `Paused while locked`), `Open Kivori`, a `Launch at login` check item and `Quit`. While an app update is offered, `Update to vX.Y.Z…` sits below the status line (it opens the window and emits `updater://show`) and the tooltip ends `· Kivori X.Y.Z is ready`. The tooltip reads `Kivori · Desktop x.y.z · Firmware a.b.c` (firmware only while a device is connected, plus `Update available` when the advice says so). `tray_model(&ConnectionStatusDto, UpdateAdvice, Option<&str>)` is pure; `refresh_tray` applies it from the device thread next to `emit_status`, so the tray follows the same snapshot as the UI. The check item is re-read from the OS after each toggle.
- A second launch while Kivori runs still goes to single-instance and surfaces the window.

### First-run onboarding (M3 S6)

- State: `config.json` gains `onboarding: { completed }` (`#[serde(default)]`, still schema v1). `get_onboarding`, `complete_onboarding` and `restart_onboarding` (the last backs the Device page's "Run setup again") read and save it; the two writers go through the normal config path, so they publish `config://changed` like any save.
- Existing users: a file that parses and has no `onboarding` key was written before the field existed, so `ConfigStore::open` loads it as `completed: true` and the stored settings are never questioned. A missing file (fresh install) or a file that is recovered as corrupt or newer starts at `completed: false`, so first-run setup shows once and can always be skipped. A file that does name the field is taken at its word. `reset` keeps `completed` as it was, so "Reset all settings" does not send anyone back through setup. Downgrading to a build older than this field makes the strict parser set the file aside as corrupt (the known M2 consequence of `deny_unknown_fields`, only before the beta).
- Permissions (`platform/permissions.rs`): `accessibility()` (`AXIsProcessTrusted`), `request_accessibility()` (`AXIsProcessTrustedWithOptions` with the prompt option, so macOS shows its own dialog) and `open_accessibility_settings()` (`/usr/bin/open` with one fixed `x-apple.systempreferences:` URL, no caller input) return `Granted`, `Missing` or `NotApplicable` (every other OS). The FFI is declared by hand like the other macOS code. The webview reaches them through `get_accessibility`, `request_accessibility` and `open_accessibility_settings`, which return the tokens `granted`, `missing`, `notApplicable`.
- Diagnostics: the "input permission" row reads `accessibility()` (cached for a second in the device loop): `granted` or `required` on macOS, `notNeeded` elsewhere, and `required` also while the last desk action was refused by the OS.
- UI (`features/onboarding/`): while `completed` is `false`, `App.tsx` renders `Onboarding` instead of the sidebar layout (until the native answer arrives, which is a few milliseconds, the normal layout shows). Steps: Welcome, Plug in, Permissions, Tour, Make it yours, Done. A step moves on only from real status (connected, Accessibility granted, a control seen on the device) or its Skip button, and "Skip setup" is always in the header. Plug in follows `ConnectionStatusDto`; an incompatible device, or three or more failed reconnects (`needsRecovery`), offers the S3 `RecoveryDialog`, and the update hint appears when the advice says `updateAvailable`. Permissions is dropped from the step list when `get_accessibility` says `notApplicable` (Windows). The Tour needs no new input event: `useTour` watches `desk://status` and ticks Turn the knob on a change of `volumePercent`, then Press the knob and Press a button on a new settled (not `processing`) `lastAction`, in that order. A repeated identical action does not change the status, so the button hint asks for the side buttons. Make it yours saves the default view and buddy intensity immediately (`setDisplaySettings`, `setBuddySettings`); Launch at login is on by default but is only applied (`set_launch_at_login`) when the person presses Finish, and not at all on Skip setup.

### Splash screen (M2.6)

On a normal launch the main window stays hidden and `setup` starts a watcher thread. If the frontend has not called `app_ready` ~300 ms after launch, the watcher opens a small undecorated `splash` window (a second Vite page, `splash.html`, bundled locally: no remote assets, no React) and keeps its one status line in step with the real core state ("Starting", then "Looking for your Kivori" once the device task has begun connecting); there is no percentage. When `app_ready` arrives (the frontend sends it after its first native answer) the splash fades and is destroyed and the main window shows; a fast start never opens the splash. A login launch (`--autostart`, from the autostart plugin) starts no watcher: no splash, window hidden, `app_ready` is a no-op. If the frontend never reports ready, the window is shown anyway after 10 s so a broken page cannot leave only a splash. The splash window has a capability for events only, no app commands. The decisions live in the pure `startup` module.

### Config

- One JSON file, `config.json`, in the per-user app data directory (`app_local_data_dir`: `%LOCALAPPDATA%\id.immer.kivori` on Windows, `~/Library/Application Support/id.immer.kivori` on macOS). It is Local, not Roaming, so settings stay per user and per machine. Installers never touch it. A missing file means defaults, and nothing is written until the first change.
- Atomic write: write `config.json.tmp` in the same directory, flush it to disk, then rename it over `config.json` (retried 3 times, 50 ms apart, for antivirus locks). There is no non-atomic fallback. A stale `.tmp` is deleted at load.
- Save first, then apply: a command validates and saves, and only then sends the new config to the device thread. A failed save changes nothing and returns a fixed, path-free error.
- Recovery (`ConfigStore::open` never fails): a file over 256 KiB, not JSON, without a numeric `version`, with an unknown field or an invalid value is renamed to `config.corrupt-<unix_secs>.json` and defaults are used (`notice: recoveredCorrupt`). A `version` above this build's is renamed to `config.v<N>-<unix_secs>.json` with its bytes intact (`recoveredNewerVersion`). The newest 5 of these backups are kept. Each recovery is a `configRecovered` activity record and a `notice` on `ConfigDto`. Paths never cross IPC.
- Versioning: `version` is read first. An older file is copied to `config.v<old>.bak`, migrated step by step (`config/migrate.rs`, one function per version) and written back (`notice: migrated`). The schema rejects unknown fields, so during M2 new fields stay in version 1 with `#[serde(default)]`; after the beta ships, any field change bumps `CONFIG_VERSION`.
- Reset copies the current file to `config.before-reset.json` (one slot) and writes defaults.
- The file holds `customProfiles` (at most 16: `id` `custom-<n>`, `name`, `apps`, `inPinCycle`, and `bindings`, a full copy of General's slots made when it was created or reset), `profiles` (sparse overrides by profile id, which also carry a built-in's `addedApps` and `removedApps`: `rotate`, `press`, `hold` and three `buttons` of `press` and `hold`) and `macros` (at most 32, see Macros), `display` (`defaultView`, `secondaryView`, `brightness` 1 to 5) and `buddy` (`reactions`, `intensity`). Intensity is the mascot personality: low, normal, high are calm, cozy, playful. Reactions turn self-play on or off (on the device, or in the companion director for older firmware). A double press toggles between the default and the chosen view (from any other view it returns to the default), or steps through every view when `secondaryView` is `cycle`.

### Diagnostics

- `get_diagnostics` returns `DiagnosticsDto`, shown as a card on the Device page (polled every 1 s while visible) and copied as JSON by "Copy diagnostics". Groups: `versions` (app, firmware, protocol, negotiated minor, capability names, short device hash, and `identity`: product and board display names, hardware revision or `null`, identity source `legacy`, `build` or `chip`, and whether the catalog knows it; product and board names are not personal data), `connection` (state, connected-for, reconnects, retry count, last pong age, round trip), `health` (device uptime, free SRAM, malformed frames, sequence gaps, device errors), `host` (system volume, app volume, media observation, focus detection, input permission) and `config` (status, schema version, custom bindings, macros). Unknown values are `null`.
- Privacy: no ports, paths or raw device ids. The device appears only as its short hash. A DTO allowlist test pins the serialised keys.
- With developer mode on, the Device page adds a Device internals card (see Device internals), and "Copy diagnostics" adds an `internals` group from its latest reply; with developer mode off neither exists.
- Sources: `Session` keeps `Health.free_bytes`, `Pong.uptime_ms` with its receive instant, and counters for malformed frames, sequence gaps and device `Error`s. All of it is connection-scoped and reset by `Session::open` (the free-memory and uptime figures also clear when the link drops). The round trip comes from `HeartbeatMonitor` (`on_ping_sent(t_ms)`, `on_pong(t_ms_echo, now_ms)`), taken only from the echo of the latest ping.
- The device thread writes a `DiagnosticsSnapshot` (raw figures and `Instant`s) at most once a second; `get_diagnostics` turns it into ages and adds the config summary from the config store at read time. Host-service tokens come from the live backends (`available`, `unsupported`, `unavailable`; media `observable`; focus `detecting`; input permission `required`, `granted`, `notNeeded` or `unknown`). Input permission is `granted` or `required` on macOS (from the Accessibility check, see First-run onboarding), `required` also after the OS refused synthesized input, and `notNeeded` elsewhere.
- Firmware update advice (bundled version, update available) belongs in `FirmwareStatus` (M3); diagnostics will read it from there rather than duplicate it.

### Firmware flashing from the app

- Overview has a Flash firmware button, enabled when connected and no update is running. `flash_firmware` takes no arguments. It installs only the firmware embedded in the desktop build.
- Native flow: reject a disconnected or concurrent request, release the serial port, run `espflash` on that port with verification on, wait for a compatible handshake from the same device, then report `succeeded`. Failure returns `failed` and connection handling resumes. Accepting a request is not completion.
- `FirmwareStatusDto`: `available`, `phase` (`idle, preparing, flashing, reconnecting, succeeded, failed`), `message`, `imageSize`. The UI polls it every 750 ms. Timeouts: flash 90 s, reconnect 20 s.
- Embedding: `scripts/build-desktop-firmware.ps1` builds the firmware release ELF with `physical-st7789`, then builds the desktop with it embedded. For other pipelines set `KIVORI_FIRMWARE_PATH`. The build rejects a non-RISC-V or non-ELF32 image. A host build without the variable disables the action, so a stale artifact cannot slip in.
- `espflash` is the pinned sidecar shipped in the installer (dev builds may use one on `PATH`). Mirror to device sends a state command only. It does not install anything.
- The offer (M2.7, `src/firmware_offer.rs`, pure): the device thread evaluates it every tick from the connection, the session identity, the bundled image's identity record, the updater state and whether a flash or knob gesture runs, and publishes `FirmwareOfferDto` (`kind`: `none | offerOptional | required | updateApp | deferredForAppUpdate`; product name, `from` and `to` versions, required `reason`: `protocolTooOld | missingCapabilities`, and an opaque `key`) on `firmware://offer`. Rules:
  - Only the bundled image whose record names the device's product and board counts, compared with `update_advice`. An unknown or foreign product, or an identity not yet reported, gets `none`; the flash guard (`wrongProduct`) still decides what may be written.
  - The question (`offerOptional`, `required`) appears only after the connection has been idle (no gesture, no flash) for 5 s; once shown, a gesture does not take it away. A flash always makes it `none`.
  - App first: while the updater holds, offers, downloads or is about to install an app update (`Machine::app_update_due`), a question becomes `deferredForAppUpdate`; the new app may bundle newer firmware. The tray then shows only the app item, and the dialog never opens by itself while an app update is offered.
  - Keyed by short device hash, bundled version and connection generation. "Later" (`dismiss_firmware_offer`, optional offers only) hides that key; the next connect is a new key and asks again. The memory is per run, not stored: a restart is a new connect, so "Later" never skips for good.
  - `required`: an incompatible handshake with an older (or unknown) protocol major, or a connected device older than the bundled firmware that lacks a capability the catalog requires of its product. It has "Update now" and "Not now", which only closes the dialog. An incompatible device's "Update now" takes the restore path, like Flash firmware does. A device newer than the app (newer firmware, or a newer protocol major) is `updateApp`: the Device page says to update the app and, when an app update is offered, links to its dialog.
  - "Update now" (`accept_firmware_offer`) queues the ordinary `FlashFirmware` (takeover screen, phases, verification); `features/updates/FirmwareOfferDialog.tsx` follows `get_firmware_status` until `succeeded` or `failed`. The tray shows "Update Kivori firmware…" while a question is open and no app update is offered; it opens the window on the dialog (`firmware://show-offer`). Activity: `firmwareOfferShown` (once per key), `firmwareOfferAccepted`, `firmwareOfferLater`, all without metadata.

### Release pipeline (M3 S5)

- Versions: the Cargo workspace version is the only app version. `tauri.conf.json` has no `version`, so Tauri takes it from `kivori-desktop`; the installer name, `CFBundleShortVersionString` and the tag check all follow it.
- Config split: the base `tauri.conf.json` stays dev-safe (`bundle.active: false`, no `externalBin` or `resources`), because Tauri's build script checks those paths at compile time and would break every dev and CI `cargo build`. Packaging lives in `tauri.release.conf.json` (NSIS, per-user install, embedded WebView2 bootstrapper, the `espflash` sidecar, `THIRD_PARTY_NOTICES.md`) and `tauri.release.macos.conf.json` (app and dmg, the adapter resource, `Info.plist`, `Entitlements.plist`, hardened runtime), merged with `tauri build --config`. Release builds pass `-- --no-default-features`, so Device Studio and the PATH fallback are compiled out.
- Sidecar: `scripts/fetch-espflash.{sh,ps1}` download espflash 4.6.0 from the official GitHub release, check a pinned SHA-256 and write `src-tauri/binaries/espflash-<triple>[.exe]` (gitignored). Tauri installs it as `espflash[.exe]` next to the app, which is where `find_espflash` looks first. Bump the version and hashes together, in both scripts and `release.yml`.
- Windows: `.github/workflows/release.yml` builds the firmware ELF on Ubuntu, then on Windows builds the NSIS installer with `KIVORI_FIRMWARE_PATH`, runs `check-release-surface.sh --desktop-artifact` (dev symbols absent, firmware version marker present), silently installs, checks the exe and `espflash --version`, runs the startup smoke test with `--autostart`, and silently uninstalls. `nsis/hooks.nsh` removes the HKCU `Run\Kivori` value that `tauri-plugin-autostart` writes; the workflow plants that value before uninstalling to prove it. A release tag (which must equal the workspace version), from the Release button's call or a `v*` tag pushed by hand, uploads the installer and `SHA256SUMS` to a draft GitHub release. Nothing is published automatically.
- Signing is not implemented: the installer is unsigned and a placeholder step (repository variable `WINDOWS_SIGNING`) fails closed until owner certificates exist (#36). Unsigned binaries trigger SmartScreen, and Defender may flag `espflash.exe`.
- macOS (ADR-0009): `scripts/bundle-macos.sh` is the main path; CI only builds it on a manual dispatch. The adapter is staged by `build.rs` in `target/release/mediaremote-adapter/` and bundled to `Contents/Resources/mediaremote-adapter`, which is what `adapter_dir()` resolves. With `APPLE_SIGNING_IDENTITY` set the script signs the two Mach-O helpers, then the app, with hardened runtime. Notarization is not done.

### App updates (M2.7, ADR-0015)

- Native only: `tauri-plugin-updater` (no `zip` feature) is used from `src/updater/` alone. There is no JS package and no permission in `capabilities/`; the webview gets the typed commands and events above.
- Availability (`updater::policy::availability`, read from the merged Tauri config at startup): Windows, `plugins.updater` with exactly the endpoint `https://github.com/cunilab/Kivori/releases/latest/download/latest.json` and a non-empty `pubkey`. Only then is the plugin registered and the `kivori-updater` thread started. Otherwise the status is `disabled` (`devBuild`: the base `tauri.conf.json` has no updater section; `noKey`: the release config's key is still empty; `badEndpoint`) or `unsupported` (macOS), and no request is ever made. The plugin needs its config at init, so not registering it is also what keeps a dev build from failing to start.
- Schedule (pure `Schedule`): first check 30 s after start, then every 24 h while the app runs, if "Check for updates automatically" is on. "Check now" checks whatever the setting. A check that fails (offline, server error, unreadable answer) changes nothing on screen, records `appUpdateCheckFailed` (source `update`, no URL or version) and waits for the next interval.
- Decisions (pure `decide`, `Machine`): `disabled | unsupported | idle | checking | available | downloading | readyToRestart | failed`. Only a version newer than the running one (semver) is offered; a skipped version never is; a snoozed one ("Later") is not offered for 3 days, unless the user presses "Check now". A check runs only from `idle`, so an open offer or an install is never replaced. An update found while a firmware flash or a knob gesture runs is held and offered when both are over (polled every second); the install downloads and verifies, then waits the same way before the installer starts.
- Install: the plugin downloads the installer named by `latest.json`, verifies its minisign signature against the compiled-in `pubkey` (`requireSignedVersion` also binds it to the announced version), then runs the NSIS installer with `/P /R` (passive, restart) and exits. Before it exits, `AppState::shutdown` stops the device thread so the serial port is released. A failure is `failed` with "Try again" and "Later".
- Config: `config.json` gains `updates: { autoCheck, skippedVersion, snoozed: { version, at } }` (`#[serde(default)]`, still schema v1, `autoCheck` defaults to true). "Later", "Skip this version" and the switch save through the normal config path. The last check time is per session only.
- UI: `features/updates/UpdateDialog.tsx` at the app root opens by itself once per version when an offer appears (the window is often hidden at login, so it is there when the window opens), and on `updater://show` or the Device page's "View update". Release notes are plain text (`whitespace-pre-wrap`), never HTML. `AppUpdates.tsx` on the Device page shows the last check, the switch and "Check now".

### Offline-first rule

Core function needs no internet, cloud, CDN, telemetry or licence check. Discovery, handshake, reconnect, rendering, Device Studio, state control, the activity log and app launch all work offline.

- No first-party crate may depend on a network client. Firmware, shared crates and the webview never get network access.
- The webview loads no remote scripts, styles, fonts, images, fetches or sockets (localhost in dev is the exception).
- No telemetry, and no startup wait on the network.
- Update discovery is the one exception (ADR-0015): `apps/desktop/src-tauri/src/updater/` alone may reach the network, only to the release endpoint, only in a Windows release build with a public key, and the guards allow that module and nothing else (see App updates).
- Guards: `scripts/check-offline-deps.sh` (direct deps of both workspaces: no network client, and no network-capable Tauri plugin except `kivori-desktop` → `tauri-plugin-updater`; transitive deps on every desktop target with that plugin pruned: no network client left; sources: no network API outside `src/updater/`), `scripts/check-frontend-offline.mjs` (also: no network plugin JS package, import or capability permission), `apps/desktop/src-tauri/tests/offline_smoke.rs` (also: the same source rule, and a development build never starts the updater).

## 7. Decisions

Each architecture decision is its own record in [`docs/adr/`](./adr/). The [ADR index](./adr/README.md)
lists them all with their status and says how to add one.

## 8. Engineering principles

1. Observable truth first. An acknowledgement is not confirmation, unknown stays unknown, stale input is never replayed.
2. One visual model. Preview and device use the same scenes, assets, timing and renderer. The frontend never re-implements device pixels.
3. Deterministic rendering. Same inputs give byte-identical RGB565. No wall clock, unseeded randomness or platform floats. Intentional pixel changes update the goldens in the same change.
4. Hardware limits are design inputs (RAM, flash, SPI, USB, boot straps, recovery). Shared crates stay `no_std` and no-alloc. Hardware claims need hardware evidence, and host-sim results are labelled as host-sim.
5. Desktop owns desktop meaning (OS integration, bindings, actions, config, updates). Firmware owns what must work without a healthy desktop (input conditioning, gestures, local rendering, device health, recovery).
6. Send meaning across boundaries (state, input, presentation), not drawing commands or streamed frames.
7. Offline-first core. Network features are isolated, explicit, and never block startup or device operation.
8. Least privilege. The webview gets typed native capabilities only. Ask for OS permission only when a feature needs it. Missing permission is a capability state, not a reason to elevate. Diagnostics use allowlists.
9. Test across boundaries without hardware where possible, and validate for real where a mock cannot prove the claim. A test must not claim evidence for a layer it did not exercise.
10. Build the smallest complete vertical path. Extract a crate or service only when isolation, reuse, security or runtime ownership needs it.
11. Record hard-to-reverse decisions as a new entry in [Decisions](#7-decisions): what, why, and what was rejected.
12. Evidence beats earlier recommendations. If measurements or tests show a better approach, change the code and fix the docs.
