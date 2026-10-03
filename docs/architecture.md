# Kivori architecture

How Kivori is built today. Product goals are in [product.md](./product.md), open work in [roadmap.md](./roadmap.md), test and hardware evidence in [validation.md](./validation.md).

## 1. Overview

Kivori is a small desk companion. A keycap-shaped mascot lives on a 240x240 screen, reacts to what the desktop tells it, a rotary knob controls system volume, and its push switch runs actions (Press, Hold) and the out-of-band recovery hold. The display can also show a clock, volume, media and CPU/RAM view.

| Component | What it is |
|---|---|
| Firmware | ESP32-C3, ST7789 240x240 panel, HW-040 rotary encoder. `no_std`, no heap, `riscv32imc`. |
| Desktop native core | Tauri v2 Rust process. Owns the serial port, the connection, OS integration, preview rendering, flashing. |
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
| `apps/desktop/src-tauri` | `kivori-desktop`: `device/` (discovery, session, reconnect, heartbeat, nonce), `runtime/` (device task), `orchestrator/`, `input/`, `action/`, `platform/`, `presentation/`, `companion.rs`, `activity/`, `firmware.rs`, `ipc/`, `render/`. |
| `apps/desktop/src` | React frontend and typed IPC wrappers. |
| `firmware/esp32-c3` | Separate workspace: `ports.rs`, `input/` (quadrature, gesture), `physical_st7789.rs`, `physical_rotary.rs`, `runtime.rs`, `proto.rs`, plus host simulation adapters in `sim/`. |
| `tools/asset-compiler` | SVG to deterministic RGB565 blob (host build tool). |
| `tools/wokwi-*`, `tests/e2e-host-sim`, `tests/golden-frames` | Simulation vectors, host-sim end-to-end tests, cross-OS frame-hash harness. |
| `scripts/` | Boundary guards (`check-crate-boundaries.sh`, `check-offline-deps.sh`, `check-frontend-offline.mjs`) and firmware/desktop build scripts. |

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
- Flashing: the installed `espflash` utility, driven by the native core.
- Not built yet: a Linux backend, desktop self-update, a per-device stable id (the firmware uses a fixed 16-byte id).

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

Constants: `PROTOCOL_MAJOR` 1, `PROTOCOL_MINOR` 4. COBS and CRC are implemented in-crate.

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

- A device counts as Kivori only after a well-formed `HelloAck` with the matching nonce. VID:PID alone is not identity.
- The desktop gives up on a missing `HelloAck` after 5 s, closes the link, and retries with backoff.
- After connecting, the desktop pings every 1 s and treats 3 misses as a lost link. `Pong.t_ms_echo` matches a ping.
- The desktop never sends state commands to an `Incompatible` device.

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

### Messages (tag = postcard variant index)

| Tag | Message | Dir | Payload | Capability |
|---:|---|---|---|---|
| 0 | `Hello` | D to V | desktop_version, desktop_caps, nonce | |
| 1 | `HelloAck` | V to D | device_caps, device_id, firmware_version, nonce_echo | |
| 2 | `Ready` | D to V | negotiated_minor, negotiated_caps | |
| 3 | `Bye` | both | reason | |
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
| 16 | `Feedback` | D to V | session, action, kind | `ACTION_FEEDBACK_V1` |
| 17 | `MediaInfo` | D to V | session, `Option<MediaInfo>` (title, artist) | `MEDIA_INFO_V1` |

`InputEvent.control` is `Rotary` or `Button`. With `Rotary`, `kind` is `GestureStarted`, `Detent(Cw|Ccw)` or `GestureEnded`; with `Button` (needs `BUTTON_INPUT_V1` too), it is `Press`, `Hold` or `DoublePress` (needs `DOUBLE_PRESS_V1`), sent once per gesture. The desktop rejects a kind that does not belong to its control. `Presentation.primary` is `Idle, Active, Error, Unknown`. `value` is `{ kind: Volume, current_percent, confidence, at_boundary }`.

`DeskStatus` (`kivori-model::desk`) is `{ mode, clock, volume_percent, muted, media, cpu_percent, ram_percent, high_load }`; every value is an `Option` and `None` is rendered as unknown. `mode` is `Buddy, Clock, Volume, Media, System`; `media` is `Playing, Paused, Stopped`. `Feedback.kind` is `Processing, StateConfirmed, ExecutionConfirmed, Unverified, Error` and `action` is `Volume, PlayPause, Mute, Shortcut, Launch`. `MediaInfo` text is `MediaText`: at most 32 ISO-8859-1 characters (what the device's Latin-1 font can draw), sanitized on the desktop (other characters become `?`, control characters dropped, a cut ends in `~`). Status, feedback and media info are session-scoped like `Presentation`: the device drops either from another session and forgets both on every session boundary.

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

`Capabilities` is a `u32` set in `kivori-model`. Bits are allocated centrally and never reused.

## 3. Desktop and webview IPC

The webview may call only these commands and listen to these events. No command exposes raw serial bytes, port choice, filesystem, shell, environment, raw `device_id`, or the network. DTOs carry no internals (device id is a short hash only). TS wrappers live in `apps/desktop/src/lib/ipc/`.

| Command | Purpose | Build |
|---|---|---|
| `get_app_info` | App version, protocol version, supported majors, Device Studio flag | all |
| `get_connection_status` | Connection, desired, reported, device info, retry count, connection generation, negotiated mascot flag, last mascot action | all |
| `list_states` | Companion states | all |
| `set_desired_state` | Set `desired`; sends `SetState` when connected | all |
| `configure_companion` | Personality and self-play | all |
| `play_mascot_action` | `greet, pet, tickle, surprise, comfort` | all |
| `get_activity_log` | Newest N activity records | all |
| `get_firmware_status`, `flash_firmware` | Firmware update status and request (no arguments) | all |
| `render_preview_frame`, `open_preview_stream`, `update_preview_stream`, `ack_preview_frame`, `close_preview_stream` | Native-rendered RGBA frames for Device Studio | `device-studio` only |
| `mirror_state` | Dev-labelled `set_desired_state` | `device-studio` only |
| `get_desk_status` | Display mode, monitored values (`null` = unknown), Press/Hold bindings, last action outcome | all |
| `set_display_mode` | `buddy, clock, volume, media, system` | all |
| `run_test_action` | Run `playPause`, `mute`, `shortcut` (text, parsed natively) or `launch` (target, validated natively) now | `device-studio` only |

| Event | Payload | When |
|---|---|---|
| `connection://status` | `ConnectionStatusDto` | Any change to connection, desired or reported |
| `activity-log://event` | `ActivityEventDto` | A new activity record |
| `desk://status` | `DeskStatusDto` | Any change to the desk projection |

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
- In simulation firmware can render through small tile buffers. On the physical ST7789 it stages one full RGB565 pose (115,200 bytes, 112.5 KiB) in static SRAM, then sends only changed 40x40 tiles by SPI DMA. Composing and hashing never overlap the transfer. Tile hashes (`kivori-framebuffer`) skip unchanged tiles.
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
- A pose's `press_q8` sinks the cap and the face printed on it while the base stays put, so the walls visibly shorten. Happy holds a 16 px press.
- `MascotAnimator` (in `kivori-model`) resolves blinking, per-state motion, and eased transitions (350 ms, 600 ms into sleep). Expression changes are hidden inside a blink. Firmware keeps the animator between state changes and renders through the same compositor as Device Studio.
- Volume overlay room: `MascotPose::with_overlay_room` scales the body by 0.72 (184/256) and lifts it 34 px so the keycap sits above the bar. The switch is instant, not eased.

### Companion and personality rules

- Personalities: `cozy` (default), `playful`, `calm`. They scale motion amplitude and the self-play interval (calm 18-30 s, cozy 8-16 s, playful 4-8 s). They never change the reported state.
- Reactions: `greet, pet, tickle, surprise, comfort`. The companion director (desktop) runs self-play only in `idle` and `happy`. Device Studio reactions are forwarded to a connected device that negotiated `MASCOT_INTERACTION`.
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

- Input port: `InputSource::sample() -> { a, b, sw }`. `PhysicalRotary` only reads and inverts the pins. All three lines are active-low with internal pull-ups. Pins: CLK GPIO4, DT GPIO5, SW GPIO10 (unread today). GPIO9 is avoided because it is the BOOT strap. Power the HW-040 from 3V3, not 5V. This pin map is a specification until a dated physical check closes it (see validation.md).
- Decoder: a Gray-code state machine on phase `(a << 1) | b`, resting at `00`. It emits a `Direction` only when the knob returns to rest after four quarter-steps in one sense. The accumulator resets at every rest arrival. Bounce and partial motion emit nothing. An impossible transition (both bits change) is counted as `invalid_transitions`, resets the accumulator, and is a diagnostic only.
- Gesture: the first detent opens a gesture (`gesture_id`, unique within the session, never 0). 250 ms without a detent (`GESTURE_END_MS`) emits exactly one `GestureEnded`. A direction reversal does not split a gesture.
- Ingress: `InputIngress` rejects an event with a stale session (`StaleSession`). It also requires an observed `GestureStarted` before a `Detent` or `GestureEnded` (`UnknownGesture`), as defence in depth.
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
  - Keep three ideas apart: `BackendAvailability` (implemented or not), `ConfirmationClass` (`StateConfirmed`, `ExecutionConfirmed`, `TriggeredUnverified`), and the derived `ActionAvailability`. An OS-level `PlatformCapability` is deferred until an OS-restricted action exists.
- Presentation: `PresentationResolver` is a pure function from `ProductSnapshot` to `Presentation`. `value` is a transient overlay of 800 ms (`VALUE_TRANSIENT_MS`). Firmware expires it locally, so the overlay clears without a host timer. Firmware stores `primary` but does not render it; a failed volume write is shown through `Feedback { Volume, Error }` instead (section 5b).
- Overlay: `render_volume_overlay` (`kivori-renderer/src/overlay.rs`) draws a solid-rect bar with no glyphs. `Confirmed` is a solid fill, `Preview` is hollow (top and bottom rows only), and the track outline turns white at a boundary. It is composited over the mascot in the same tile pass.
- Firmware owns raw input truth (conditioning, detents, gestures). The desktop owns action meaning.
- Edge capture: every edge on CLK, DT or SW raises the GPIO interrupt, whose handler only stores a timestamped level snapshot in a 64-entry queue (`physical_rotary.rs`). The run loop drains it each tick (`InputSource::drain`), so quarter-steps and switch edges during a frame compose or flush are not lost. On overflow new edges are dropped: the decoder counts one invalid transition, which can lose a detent but never invents one.
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
- Bindings (M1, fixed until the M2 config UI): Press = Play/Pause, Hold = master mute, Double press = next view. Shortcut and launch actions exist and run from Device Studio's test action.
- Classification (`desk/actions.rs`): mute is State Confirmed only when the OS read-back shows the new state; play/pause and a shortcut are always Unverified (a matching media state can be stale or caused by something else, so it is never proof; the media indicator and view show what the OS reports); a launch is Execution Confirmed once the OS accepted it (`open -a` exit 0 on macOS, process created on Windows; no shell). A missing permission is Error with `permission_required`, never another mechanism.
- `FeedbackLadder` (`desk/mod.rs`): Processing at 500 ms, Unverified at 1.5 s, after which a late outcome is logged but not shown. Timeout is never Error. A newer action replaces the pending feedback; a session end clears it, and requests still queued for the worker are skipped (an epoch counter), so nothing from an old session runs later. A new streak of failed volume writes is shown once as a volume Error.
- `StatusPublisher`: sends `Status` when anything shown changes, when the minute rolls over, or every 30 s; the device advances the clock locally in between. High load is CPU at or above 85 % for 3 samples, cleared below 70 % or when CPU is unknown.
- Device rendering order (`firmware/.../render.rs`, `kivori-renderer::desk`): recovery takeover, else the Buddy pose or the selected desk view, then the volume overlay (except in the Volume view, which shows the value itself), then the chrome: mute and media indicators, the high-load cue (Buddy only), the press frame and the feedback badge. Unverified is an amber ring with `?`, never a check mark.
- View switch (`desk::render_views`): for `VIEW_TRANSITION_MS` (320 ms) the outgoing view slides up and the new one pushes in from below (integer ease-out cubic). Each view is drawn into a row slice of the tile (`TileBand::split_at_row` + `scrolled`), so the mascot slides like any view, no extra buffer is needed and tiles still stitch to the full frame. The volume overlay and chrome stay fixed above it.

## 6. Activity log, flashing, offline rule

### Activity log (ADR-0005)

The typed session activity log is the only runtime log.

- Every record is an `ActivityEventDto`: native-issued `id`, ISO-8601 `at`, `summary`, and closed `type`, `severity` (`info, warning, error`), `source` (`connection, action, device, protocol, firmware`), `outcome` tokens.
- Optional `metadata` is a fixed allowlist: connection state, retry and elapsed counters, diagnostic category and code, versions, short device hash, capability mask, state, personality, action, seed, protocol counters, reported state. There is no free-form map.
- Never logged: raw payload bytes, raw device id, port or file paths, usernames, tokens, secrets, flasher output, raw error text. The UI reads only the fixed fields.
- Delivery: the webview subscribes to `activity-log://event` first, then calls `get_activity_log(limit)`. It dedupes by id (live copy wins), sorts by id, and keeps the newest 256. Nothing is persisted or uploaded.
- The raw device id stays inside the transport layer. Only a short non-reversible hash may appear.
- Raw payload output exists only behind the `debug-payloads` Cargo feature. It is off by default and must not ship.
- Tests: native redaction tests, frontend runtime-cast privacy tests, ordering and 256-entry retention tests.

### Firmware flashing from the app

- Overview has a Flash firmware button, enabled when connected and no update is running. `flash_firmware` takes no arguments. It installs only the firmware embedded in the desktop build.
- Native flow: reject a disconnected or concurrent request, release the serial port, run `espflash` on that port with verification on, wait for a compatible handshake from the same device, then report `succeeded`. Failure returns `failed` and connection handling resumes. Accepting a request is not completion.
- `FirmwareStatusDto`: `available`, `phase` (`idle, preparing, flashing, reconnecting, succeeded, failed`), `message`, `imageSize`. The UI polls it every 750 ms. Timeouts: flash 90 s, reconnect 20 s.
- Embedding: `scripts/build-desktop-firmware.ps1` builds the firmware release ELF with `physical-st7789`, then builds the desktop with it embedded. For other pipelines set `KIVORI_FIRMWARE_PATH`. The build rejects a non-RISC-V or non-ELF32 image. A host build without the variable disables the action, so a stale artifact cannot slip in.
- `espflash` is the installed tool, not bundled. Mirror to device sends a state command only. It does not install anything.

### Offline-first rule

Core function needs no internet, cloud, CDN, telemetry or licence check. Discovery, handshake, reconnect, rendering, Device Studio, state control, the activity log and app launch all work offline.

- No first-party crate may depend on a network client. Firmware, shared crates and the webview never get network access.
- The webview loads no remote scripts, styles, fonts, images, fetches or sockets (localhost in dev is the exception).
- No telemetry, and no startup wait on the network.
- If update discovery is added, it goes in one isolated native module, and the guards are narrowed for that module only.
- Guards: `scripts/check-offline-deps.sh`, `scripts/check-frontend-offline.mjs`, `apps/desktop/src-tauri/tests/offline_smoke.rs`.

## 7. Decisions

- **ADR-0001 Two cargo workspaces.** The root workspace holds the host crates, the desktop, tools and tests. `firmware/esp32-c3` has its own `[workspace]` and reaches shared crates by path. Because one workspace unifies features, and a std feature enabled by the desktop would silently break the firmware `no_std` build. The firmware also needs its own target, panic strategy and runner.
- **ADR-0002 One wire protocol crate.** COBS framing, CRC-32, postcard payloads, an append-only `Message` enum, major-gated compatibility, minor and capability negotiation. Because both peers share one schema, COBS resyncs after corruption where a length prefix cannot, and postcard is `no_std` with a stable format. Reordering or removing anything is a major change.
- **ADR-0003 Integer-millisecond timebase.** Time is `u32` ms, and Device Studio derives it from a step index with `(n*1000+15)/30`. Because accumulating 33.33 ms as a float or 33 ms as an integer drifts and differs across platforms. Any frame is directly addressable.
- **ADR-0004 Layered scenes and a compiled asset blob.** Scenes are layers with integer keyframes, drawn with `embedded-graphics`, and compiled from SVG into a deterministic blob (format v2 adds alpha4 and the mascot roles, and allows Q8 interpolation for the mascot). Because baked frames cost too much flash and fight tile rendering, and byte-reproducible assets keep host and device pixels equal.
- **ADR-0005 Typed activity log with an allowlist.** Closed types, fixed metadata, hashed device id, raw payloads compiled out, session-only history. Because payloads, identity, paths and tool output are sensitive. Support loses raw-byte detail in release builds on purpose.
- **ADR-0006 The handshake nonce is the session identity.** One OS-random `u32` per connection attempt stamps `InputEvent` and `Presentation`, and `revision` is session-scoped. Because a complete stale gesture pair buffered across a disconnect would otherwise pass the observed-`GestureStarted` check and change the volume. A dedicated `SessionEpoch` message was rejected because the nonce already has the right lifecycle. `gesture_id` only needs to be unique inside a session.

- **ADR-0007 Interrupt-captured input edges.** The GPIO interrupt records a timestamped level snapshot on every edge of CLK, DT and SW into a bounded queue; decoding stays above the port. Because the loop only returns to input between render passes, and composing plus flushing a frame takes longer than the quarter-steps of a detent, so polling once per tick loses detents and mistimes presses. Rejected: sampling between tile writes (still blind during composition) and a timer-sampled ISR (fixed cost whether or not anything moves).
- **ADR-0008 Desk state as two new messages.** Display mode plus monitored values travel as `Status` (tag 15, the whole snapshot on every change) and action outcomes as `Feedback` (tag 16, transient, newest wins), each behind its own capability. Because `Presentation` is a fixed wire struct (adding fields is a major change) whose job is the rotary overlay, and persistent truth and transient outcomes expire differently. Rejected: a per-field delta protocol (more states to get wrong for a payload under 64 bytes).

- **ADR-0009 macOS now-playing observation is layered: MediaRemote adapter, then AppleScript, then unknown.** macOS has no public system-wide now-playing API: `MPNowPlayingInfoCenter` only publishes, and the private MediaRemote framework has refused unentitled callers since 15.4. Kivori therefore (1) runs the BSD-3 [ungive/mediaremote-adapter](https://github.com/ungive/mediaremote-adapter), copied into `apps/desktop/src-tauri/vendor/` at a pinned commit and compiled from source by `build.rs` with plain clang (no prebuilt binaries, no extra build tool, no network; a missing toolchain is a warning, not a failed build). Apple's own entitled `/usr/bin/perl` loads the helper and streams JSON lines, read by a `kivori-media` thread that owns the long-running child. Each start first runs the adapter's `test` command, so an empty payload, once it has lasted 1 s, is an honest system-wide Stopped. (2) While the adapter is unavailable (test fails, process dies, unexpected output) Kivori retries it with backoff (2 s doubling to 60 s) and meanwhile polls Spotify and Music through public Apple Events (`osascript` with a 3 s timeout, never launching either app, one-time Automation prompt); because AppleScript cannot see browsers, finding nothing there is unknown, never Stopped. (3) Otherwise status and title are `None`. Rejected: the Rust wrapper crates (they ship prebuilt binaries, need a Swift toolchain, or are barely maintained) and AppleScript alone (no browsers). Accepted risk: a private framework reached through an Apple binary that Apple may remove or lock down; then Kivori degrades to layer 2 or unknown and never reports a guessed state. Titles and artists are never logged (ADR-0005). Release builds must bundle and sign the helper (roadmap M3).

## 8. Engineering principles

1. Observable truth first. An acknowledgement is not confirmation, unknown stays unknown, stale input is never replayed.
2. One visual model. Preview and device use the same scenes, assets, timing and renderer. The frontend never re-implements device pixels.
3. Deterministic rendering. Same inputs give byte-identical RGB565. No wall clock, unseeded randomness or platform floats. Intentional pixel changes update the goldens in the same change.
4. Hardware limits are design inputs (RAM, flash, SPI, USB, boot straps, recovery). Shared crates stay `no_std` and no-alloc. Hardware claims need hardware evidence, and simulation is labelled as simulation.
5. Desktop owns desktop meaning (OS integration, bindings, actions, config, updates). Firmware owns what must work without a healthy desktop (input conditioning, gestures, local rendering, device health, recovery).
6. Send meaning across boundaries (state, input, presentation), not drawing commands or streamed frames.
7. Offline-first core. Network features are isolated, explicit, and never block startup or device operation.
8. Least privilege. The webview gets typed native capabilities only. Ask for OS permission only when a feature needs it. Missing permission is a capability state, not a reason to elevate. Diagnostics use allowlists.
9. Test across boundaries without hardware where possible, and validate for real where a mock cannot prove the claim. A test must not claim evidence for a layer it did not exercise.
10. Build the smallest complete vertical path. Extract a crate or service only when isolation, reuse, security or runtime ownership needs it.
11. Record hard-to-reverse decisions as a new entry in [Decisions](#7-decisions): what, why, and what was rejected.
12. Evidence beats earlier recommendations. If measurements or tests show a better approach, change the code and fix the docs.
