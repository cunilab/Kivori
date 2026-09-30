# Feature Specification: Device Connection Foundation

**Feature Branch**: `001-device-connection-foundation`

**Created**: 2026-07-17

**Status**: Draft

**Input**: User description: "Build the first Kivori vertical slice: a local desktop companion foundation connecting a Windows desktop application to one ESP32-C3 device over USB serial. A user can launch Kivori, auto-discover a compatible device, establish a versioned handshake, display connection status, send one of the semantic companion states, and recover automatically after unplug/reconnect. Includes a development-only Device Studio page with a 240x240 preview driven by the same shared renderer as the device."

## Clarifications

### Session 2026-07-17

- Q: What canonical animation frame rate / time-base should the shared renderer use? → A: A millisecond-based deterministic timeline — rendering is a pure function of elapsed milliseconds. Each animation defines its own frame rate, and the device refreshes only the pixels that change. Device Studio's default step is 33.333 ms (a 30 FPS-equivalent step).
- Q: How should the desktop decide protocol compatibility at handshake? → A: Match major version — same major version is compatible (minor differences allowed); an unsupported major version is surfaced as `incompatible`.
- Q: Which companion states can the desktop actively send to the device? → A: Four sendable states (`idle`, `happy`, `busy`, `sleeping`). `booting` and `offline` are device-originated (shown at power-on and when powered-but-not-driven) and are not sent by the desktop.
- Q: Should the last desired state persist across a full desktop-app restart? → A: No — a cold launch starts at `idle` (no cross-restart persistence); a within-session reconnect still restores the most recent desired state.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Automatic Device Discovery & Connection (Priority: P1)

An owner plugs their Kivori device into a Windows PC and opens the Kivori application (or it is already
running in the background). Without choosing a port or configuring anything, the application finds the
device, confirms it is a genuine Kivori through a versioned handshake, and shows a clear connection
status.

**Why this priority**: A verified device connection is the foundation of the entire product. Nothing
else — states, animation, recovery, mirroring — is possible or demonstrable until the desktop can find
and trust a device. This is the smallest slice that proves the core promise: "plug it in and it just
connects."

**Independent Test**: Plug in a compatible device, launch the app, and confirm it transitions
`connecting` → `connected` and shows device/protocol information, all without the user selecting a COM
port.

**Acceptance Scenarios**:

1. **Given** a compatible device is plugged in, **When** the application starts, **Then** it
   automatically discovers the device and shows `connecting` followed by `connected`, without asking the
   user to choose a port.
2. **Given** no device is plugged in, **When** the application is running, **Then** it shows
   `disconnected` and keeps scanning so a later plug-in connects automatically.
3. **Given** a device reporting an unsupported protocol major version, **When** it is discovered,
   **Then** the application shows `incompatible` with a clear explanation and does not treat it as
   connected.
4. **Given** an unrelated (non-Kivori) serial device is attached, **When** the application scans ports,
   **Then** it does not report that device as connected because identity is verified via the handshake.
5. **Given** a device that never completes the handshake, **When** connection is attempted, **Then** the
   application shows `error` with a diagnostic reason and continues retrying.

---

### User Story 2 - Companion States Displayed on the Device (Priority: P2)

With a device connected, the owner (or the system on their behalf) sets a semantic companion state, and
the device shows the matching scene with the canonical animation. This is the companion visibly coming
alive on the desk.

**Why this priority**: Once a trusted connection exists, showing the right companion state on the
physical device is the core end-user experience. It is independently valuable and demonstrable, but it
depends on P1.

**Independent Test**: With a device connected, set each sendable state in turn and confirm the device
renders the correct animated scene for each.

**Acceptance Scenarios**:

1. **Given** a connected device, **When** the desktop sets the state to `happy`, **Then** the device
   displays the happy scene animated with its canonical timing.
2. **Given** a connected device, **When** the desktop sets each of `idle`, `happy`, `busy`, and
   `sleeping`, **Then** the device displays the matching scene for each state.
3. **Given** a connected device that is no longer being driven by an active desktop session, **When**
   the session ends, **Then** the device falls back to the device-originated `offline` scene.
4. **Given** a state change is issued, **When** the device receives it, **Then** the new state is
   reflected on the display near-instantly (within the latency defined in Success Criteria).
5. **Given** the desktop sets a state, **When** it is transmitted, **Then** only a semantic state is
   sent — never pixels, coordinates, or low-level drawing commands.

---

### User Story 3 - Automatic Recovery & State Restoration (Priority: P3)

The device is bumped and unplugged, or the cable is briefly disconnected. The application notices,
returns to searching, reconnects automatically when the device reappears, and restores the most recent
desired companion state without the owner doing anything.

**Why this priority**: Real desks are messy; cables get knocked. Reliable recovery makes Kivori feel
trustworthy. The product is demonstrable without it (P1+P2), so it follows them.

**Independent Test**: While connected and showing `busy`, unplug the device, then plug it back in, and
confirm the app reconnects on its own and the device returns to `busy` with no user action.

**Acceptance Scenarios**:

1. **Given** a connected device showing `busy`, **When** it is unplugged, **Then** the application shows
   `disconnected`; **When** it is plugged back in, **Then** the application reconnects automatically and
   restores `busy`.
2. **Given** repeated rapid unplug/replug cycles, **When** they occur, **Then** the application settles
   into a stable connected state without manual intervention and without rapidly flapping between
   statuses.
3. **Given** a desired state was set before a disconnect, **When** the device reconnects within the same
   session, **Then** the last desired state is re-sent and displayed.

---

### User Story 4 - Device Studio Developer Preview (Priority: P4)

A developer opens the development-only Device Studio page, which shows a 240x240 preview of the device.
They pick a companion state, scrub elapsed animation time, play/pause, step frame-by-frame, and mirror
the selected state to a connected device. The preview shows exactly the pixels the shared renderer
produces; it never draws device content on its own.

**Why this priority**: Device Studio accelerates building and validating the canonical visual model and
enables deterministic host-side testing without hardware. It is essential internal tooling but is not
part of the shipped end-user MVP, so it comes last in this slice.

**Independent Test**: With no hardware attached, open Device Studio, select each state, and use
scrub/play/pause/step to confirm deterministic animation; then, with a device connected, mirror a state
and confirm the device matches the preview.

**Acceptance Scenarios**:

1. **Given** Device Studio is open, **When** a developer selects a state, **Then** the 240x240 preview
   shows that state's scene.
2. **Given** a selected state, **When** the developer sets the elapsed time (in milliseconds) to a
   specific value, **Then** the preview shows the exact frame for that time, and the same value always
   yields the same frame.
3. **Given** the preview is playing, **When** the developer pauses, **Then** animation halts; **When**
   they step forward, **Then** the timeline advances by exactly one step (default 33.333 ms).
4. **Given** a device is connected, **When** the developer mirrors the selected sendable state, **Then**
   the device displays the same state shown in the preview.
5. **Given** the same state and elapsed-millisecond value, **When** rendered in the preview and on the
   device, **Then** both display an identical image.
6. **Given** Device Studio is rendering, **When** the preview updates, **Then** every displayed pixel
   originates from the shared renderer's output rather than from independent canvas drawing.

---

### Edge Cases

- **No device at launch**: The app starts, shows `disconnected`, and connects automatically once a
  compatible device is later plugged in.
- **Multiple compatible devices attached**: The app connects to the first compatible device discovered;
  managing more than one device concurrently is out of scope.
- **Port held by another process**: If the serial port cannot be opened (in use / access denied), the
  app shows `error` with an actionable diagnostic and keeps retrying.
- **Corrupted or malformed messages from the device**: The app rejects them safely, records a
  diagnostic, and does not crash or hang.
- **Handshake timeout / partial handshake**: Treated as `error`; the app retries rather than presenting
  a half-open connection.
- **Protocol major version newer than supported**: Surfaced as `incompatible`, not silently coerced.
- **Rapid unplug/replug (bounce)**: Debounced so the UI and connection logic do not thrash.
- **Window hidden/closed to background**: The connection and state handling continue running.
- **Elapsed-time extremes in Device Studio**: A time value of zero, very large values, and values beyond
  an animation's loop length resolve to a well-defined, deterministic frame.
- **Mirroring while disconnected**: Attempting to mirror with no connected device is reported clearly
  and does nothing to a (nonexistent) device.
- **Device powers on but no desktop is present**: The device shows the device-originated `booting` scene
  then falls back to `offline` until a desktop session drives it.

## Requirements *(mandatory)*

### Functional Requirements

**Discovery & Connection**

- **FR-001**: System MUST automatically discover a connected Kivori device by scanning available serial
  ports, without requiring the user to select or configure a specific COM port.
- **FR-002**: System MUST verify device identity through a versioned protocol handshake before treating
  a device as connected.
- **FR-003**: System MUST treat a device as compatible when its reported protocol **major** version
  matches a major version supported by the desktop (minor differences allowed), and MUST present an
  `incompatible` status that clearly explains the mismatch when the major version is unsupported.
- **FR-004**: System MUST NOT report non-Kivori serial devices as connected.
- **FR-005**: System MUST expose these connection statuses: `connecting`, `connected`, `incompatible`,
  `disconnected`, and `error`.
- **FR-006**: System MUST display the current connection status and basic connected-device information
  (such as protocol/firmware version) to the user.

**Resilience & Recovery**

- **FR-007**: System MUST detect device disconnection (physical unplug or link loss) and transition to
  `disconnected`.
- **FR-008**: System MUST automatically attempt to reconnect after a disconnection, with no user action
  required.
- **FR-009**: System MUST restore the most recent desired companion state after a successful
  reconnection within the running session. Across a full application restart the default state is
  `idle` (desired state is not persisted across restarts).
- **FR-010**: System MUST remain recoverable through repeated or rapid disconnect/reconnect cycles
  (no unrecoverable or flapping state).

**Companion States**

- **FR-011**: System MUST support six companion states: `booting`, `idle`, `happy`, `busy`, `sleeping`,
  and `offline`.
- **FR-012**: System MUST allow the desktop to command the device into any of the four sendable states
  (`idle`, `happy`, `busy`, `sleeping`).
- **FR-013**: The device MUST render the scene corresponding to its current companion state, updating
  only the pixels that change between rendered frames (change-driven / partial refresh) rather than
  redrawing the full 240x240 frame every tick.
- **FR-014**: The `booting` and `offline` states MUST be device-originated and MUST NOT be sent by the
  desktop: the device MUST display `booting` at power-on and `offline` whenever it is powered but not
  being actively driven by a desktop session (for example after connection loss or before the first
  state command).
- **FR-015**: System MUST represent companion states semantically only; the desktop MUST NOT send pixel
  data, coordinates, or low-level drawing commands to the device.

**Canonical Rendering**

- **FR-016**: The Device Studio preview and the physical device MUST use the same canonical scene
  definitions, animation timing, compiled RGB565 assets, bitmap fonts, and coordinate system.
- **FR-017**: The Device Studio preview and the physical device MUST produce identical logical images
  given the same state, elapsed milliseconds, and assets.
- **FR-018**: The preview surface MUST only display pixels produced by the shared renderer and MUST NOT
  independently draw device content.
- **FR-019**: Rendering MUST be deterministic and driven by a millisecond-based timeline: rendered
  output MUST be a pure function of (companion state, elapsed milliseconds, compiled assets, and any
  random seed) and MUST always produce identical output for identical inputs. Each scene defines its own
  animation frame rate/timing along this shared timeline; there is no single global frame rate.
- **FR-020**: The device display and the preview MUST target a 240x240 resolution.
- **FR-021**: Runtime rendering MUST consume compiled assets; source art files (such as SVG or PNG) MUST
  NOT be loaded at runtime by the device or the preview.

**Device Studio (development-only)**

- **FR-022**: System MUST provide a development-only Device Studio page containing a 240x240 device
  preview.
- **FR-023**: Device Studio MUST let a developer select any of the six companion states for preview.
- **FR-024**: Device Studio MUST let a developer set and scrub the elapsed animation time along the
  millisecond-based timeline.
- **FR-025**: Device Studio MUST let a developer play and pause the preview animation.
- **FR-026**: Device Studio MUST let a developer step the timeline forward one step at a time, where the
  default step is 33.333 ms (a 30 FPS-equivalent step).
- **FR-027**: Device Studio MUST let a developer mirror the selected sendable state to a connected
  device.
- **FR-028**: Device Studio MUST NOT be present in production end-user builds.

**Operation & Diagnostics**

- **FR-029**: System MUST operate fully without internet access; no core behavior may depend on network
  connectivity.
- **FR-030**: System MUST continue running and maintaining the device connection when its settings/main
  window is hidden or closed to the background.
- **FR-031**: System MUST expose useful local diagnostics (connection lifecycle, handshake results, and
  errors) to aid troubleshooting.
- **FR-032**: Diagnostics and logs MUST NOT contain sensitive data.

**Verification (deliverable-level)**

- **FR-033**: The feature MUST include deterministic host-side rendering tests (golden-frame /
  frame-hash) covering the shared renderer.
- **FR-034**: The feature MUST include protocol codec tests, including handling of malformed and
  corrupted messages without crashing.
- **FR-035**: The feature MUST include a firmware simulation or host-side test path exercising device
  logic that does not require physical hardware.

**Scope**

- **FR-036**: The feature MUST target only the Windows platform and only USB serial transport.

### Key Entities

- **Kivori Device**: A physical ESP32-C3 unit. Key attributes: verified identity, reported protocol
  version (major/minor), current connection status, and health/heartbeat signal.
- **Companion State**: One of six semantic states (`booting`, `idle`, `happy`, `busy`, `sleeping`,
  `offline`) describing the companion's mood/activity — the visual meaning, not any pixels. Four are
  desktop-sendable (`idle`, `happy`, `busy`, `sleeping`); `booting` and `offline` are device-originated.
- **Connection Session**: The active link between the desktop and one device. Tracks connection status,
  the desired companion state, the last-known state, and relevant timestamps.
- **Scene**: The canonical visual definition for a companion state — layers, layout coordinates, and its
  own animation frame rate/timing along a shared millisecond timeline — used by both preview and device.
- **Compiled Asset**: Runtime representation (RGB565 bitmaps and bitmap fonts) produced from source art;
  the only asset form loaded at runtime.
- **Protocol Message**: A unit exchanged over the serial link — handshake, state command, health/
  heartbeat, or diagnostic.
- **Device Profile**: Description of the target display and capabilities (240x240, RGB565 color).
- **Diagnostic Event**: A recorded, non-sensitive connection or rendering event used for
  troubleshooting.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: When a compatible device is plugged in, the application discovers and connects to it within
  5 seconds, with no manual port selection.
- **SC-002**: After an unplug and replug, the application reconnects and restores the previous companion
  state within 10 seconds, with no user action.
- **SC-003**: An incompatible device is detected and clearly surfaced within 5 seconds of being attached.
- **SC-004**: A sent companion state is reflected on the device within 1 second of being issued.
- **SC-005**: For any given state and elapsed-millisecond value, the preview and the physical device
  display an identical image (zero differing pixels).
- **SC-006**: All core features function while the host is completely offline (zero network
  dependencies).
- **SC-007**: The application maintains the device connection while its window is hidden (zero
  disconnects attributable to hiding or backgrounding the window).
- **SC-008**: 100% of malformed or corrupted device messages are handled without crashing or hanging the
  application.
- **SC-009**: Host-side rendering tests reproduce identical frames across repeated runs and across
  different machines (zero nondeterministic failures).
- **SC-010**: No diagnostic or log entry contains sensitive data (verified by automated check and
  review).
- **SC-011**: In Device Studio, the same state and elapsed-millisecond value always produces the exact
  same frame, and each step advances the timeline by exactly the configured step (default 33.333 ms).

## Assumptions

- **State model** (clarified): There are four desktop-sendable states (`idle`, `happy`, `busy`,
  `sleeping`). `booting` and `offline` are device-originated (shown at power-on and when
  powered-but-not-driven, respectively) and are not sent by the desktop. All six states are previewable
  in Device Studio.
- **Rendering time-base** (clarified): The canonical time input is a millisecond timeline; rendering is a
  pure function of elapsed milliseconds. Each animation defines its own frame rate, and the device
  refreshes only changed pixels. Device Studio's default step is 33.333 ms (a 30 FPS-equivalent step).
- **Protocol compatibility** (clarified): Compatibility is decided by protocol major version — a matching
  major version is compatible (minor differences allowed); an unsupported major version is
  `incompatible`.
- **Default & persistence** (clarified): On power-on the device shows the device-originated `booting`
  scene; once connected, the desktop sets `idle` as the default. The most recent desired state is
  retained for the running session and re-sent on reconnect. A full application restart starts at `idle`
  — desired state is not persisted across restarts.
- **Single device**: Exactly one device is supported at a time. If multiple compatible devices are
  present, the application connects to the first one discovered.
- **Diagnostics surface**: Diagnostics are exposed via an in-app view and/or a local log file containing
  only connection lifecycle and error information.
- **Development-only Device Studio**: "Development-only" means Device Studio is available in developer/
  debug builds and excluded from production end-user builds.
- **Orchestration boundary**: The desktop application is the sole orchestrator of behavior; the device
  renders and reports health but contains no integration logic.
- **Host environment**: Standard USB-serial device enumeration is available on the Windows host; the
  device appears as a serial port.
- **Placeholder artwork**: Scenes use placeholder character artwork sufficient to validate rendering,
  timing, and determinism. Final production artwork is out of scope.

## Out of Scope

- Wi-Fi and Bluetooth transports
- Cloud accounts, sign-in, and cloud sync
- Third-party plugins and plugin execution
- Music/media detection, GitHub, calendar, and AI-provider integrations
- Marketplace support
- Final production character artwork
- Operating systems other than Windows
- Managing more than one device simultaneously

## Dependencies

- A physical ESP32-C3 Kivori device (or its host-side simulation) is required for the end-to-end
  connection stories (US1–US3). Host-side rendering (FR-033) and codec/malformed-message tests (FR-034)
  and the simulation path (FR-035) require no physical hardware.
