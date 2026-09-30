# Contract: Desktop Core ↔ Webview IPC (Tauri v2)

**Feature**: `001-device-connection-foundation` | **Date**: 2026-07-17 | **Crate**: `apps/desktop/src-tauri`

The **entire** privileged surface exposed to the React webview. This is the security boundary
(Principle VIII, constraint 4): the webview may call **only** these typed commands and subscribe to
these events/channels. Raw serial, filesystem, shell, and credential access are **never** exposed.

Naming: `snake_case` Rust commands; TS wrappers in `apps/desktop/src/lib/ipc/`. DTOs are the only types
crossing the boundary (they mirror [data-model.md](../data-model.md) but carry no internals like raw
`device_id`, sequence buffers, or serial handles).

## 1. Commands (webview → core, `invoke`)

| Command                 | Args                                                                 | Returns                           | Availability |
| ----------------------- | -------------------------------------------------------------------- | --------------------------------- | ------------ |
| `get_app_info`          | —                                                                    | `AppInfoDto`                      | all builds   |
| `get_connection_status` | —                                                                    | `ConnectionStatusDto`             | all builds   |
| `get_firmware_status`   | —                                                                    | `FirmwareStatusDto`               | all builds   |
| `flash_firmware`        | —                                                                    | `Result<()>` (request accepted)   | all builds   |
| `set_desired_state`     | `{ state: SendableState }`                                           | `Result<()>`                      | all builds   |
| `configure_companion`   | `{ personality: MascotPersonality, selfPlay: boolean }`              | `Result<()>`                      | all builds   |
| `play_mascot_action`    | `{ action: MascotAction }`                                           | `Result<()>`                      | all builds   |
| `get_activity_log`      | `{ limit: u16 }`                                                     | `ActivityEventDto[]`              | all builds   |
| `list_states`           | —                                                                    | `CompanionState[]`                | all builds   |
| `render_preview_frame`  | `{ state: CompanionState, elapsed_ms: u32 }`                         | `ArrayBuffer` (RGBA8888, 240×240) | **dev-only** |
| `mirror_state`          | `{ state: SendableState }`                                           | `Result<()>`                      | **dev-only** |
| `open_preview_stream`   | `{ state: CompanionState, fps: u16, channel: Channel<ArrayBuffer> }` | `Result<StreamHandle>`            | **dev-only** |
| `close_preview_stream`  | `{ handle: StreamHandle }`                                           | `Result<()>`                      | **dev-only** |

- `set_desired_state` updates the orchestrator's `DesiredState` and, if `Connected`, transmits `SetState`
  to the device. This is the production path (future integrations call the same orchestrator internally).
- `mirror_state` is the Device Studio affordance; semantically identical to `set_desired_state` but
  gated to dev builds and labeled as a developer action.
- `render_preview_frame` returns raw **RGBA8888** bytes via `tauri::ipc::Response` (no JSON/base64); the
  canonical RGB565→RGBA expansion happens in Rust so the canvas only blits (constraint 2).
- **dev-only** commands are compiled out of release builds (Cargo feature `device-studio`); the frontend
  route is also build-flag gated (FR-028).

## 2. Events (core → webview, broadcast)

Firmware updates use native-owned status, polled by Overview every 750 ms. `flash_firmware` accepts
no firmware path, executable path, port, or command-line arguments. It installs only the firmware
embedded in the desktop build onto the already-connected device. The native boundary rejects
disconnected and concurrent requests. Acceptance is not completion: the UI waits for the status to
reach `succeeded` after flashing and a verified reconnection. A failed flash is surfaced as `failed`.

```ts
interface FirmwareStatusDto {
  available: boolean;
  phase: 'idle' | 'preparing' | 'flashing' | 'reconnecting' | 'succeeded' | 'failed';
  message: string;
  imageSize: number;
}
```

| Event                  | Payload               | When                                           |
| ---------------------- | --------------------- | ---------------------------------------------- |
| `connection://status`  | `ConnectionStatusDto` | any change to connection/desired/reported      |
| `activity-log://event` | `ActivityEventDto`    | a new typed session activity event is recorded |

Events carry small JSON DTOs (status/activity) — never frame bytes (large binary uses `Channel`,
[research §R-7](../research.md#r-7-tauri-v2-binary-frame-transport-rust--webview)).

## 3. Channels (core → webview, streaming)

- **Preview frame stream** (`open_preview_stream`): a Tauri `Channel<ArrayBuffer>` streams RGBA8888
  frames for Device Studio **play** mode at a capped FPS. The core derives `elapsed_ms` from a fixed
  origin (drift-free) and renders on demand. Closed via `close_preview_stream` or on window teardown.

## 4. DTOs

```ts
type SendableState = 'idle' | 'happy' | 'busy' | 'sleeping';
type CompanionState = 'booting' | 'idle' | 'happy' | 'busy' | 'sleeping' | 'offline';
type ConnectionState = 'connecting' | 'connected' | 'incompatible' | 'disconnected' | 'error';

interface AppInfoDto {
  appVersion: string;
  protocolVersion: { major: number; minor: number };
  supportedMajors: number[];
  deviceStudioEnabled: boolean; // false in release
}

interface ConnectionStatusDto {
  connection: ConnectionState;
  desired: SendableState; // always present (default "idle")
  reported: CompanionState | null; // device mirror; null until first StateReport
  device: {
    // present only when connected
    firmwareVersion: string;
    protocolVersion: { major: number; minor: number };
    deviceIdHashShort: string; // hashed identity ONLY (never raw)
  } | null;
  incompatibleReason: string | null; // human-readable when connection === "incompatible"
  retryCount: number;
  connectionGeneration: number; // changes whenever device uptime may reset
  mascotInteraction: boolean; // negotiated for current connected session
  mascotAction: {
    action: 'greet' | 'pet' | 'tickle' | 'surprise' | 'comfort';
    personality: 'cozy' | 'playful' | 'calm';
    seed: number;
    appliedAtMs: number; // original device uptime
  } | null;
}

type ActivityEventType =
  | 'connectionAttempted'
  | 'connectionOpened'
  | 'handshakeStarted'
  | 'handshakeSucceeded'
  | 'connectionRetryScheduled'
  | 'incompatibleFirmware'
  | 'connectionIoFailure'
  | 'handshakeTimedOut'
  | 'heartbeatTimedOut'
  | 'connectionRecovered'
  | 'connectionDisconnected'
  | 'connectionStateChanged'
  | 'deviceNegotiated'
  | 'personalityConfigured'
  | 'selfPlayConfigured'
  | 'stateRequested'
  | 'mirroredStateRequested'
  | 'manualSocialActionRequested'
  | 'autonomousSocialActionRequested'
  | 'socialActionApplied'
  | 'stateSynchronized'
  | 'deviceStateObserved'
  | 'deviceDiagnosticFraming'
  | 'deviceDiagnosticChecksum'
  | 'deviceDiagnosticVersion'
  | 'deviceDiagnosticPayload'
  | 'deviceSequenceGap'
  | 'deviceDisplayFault'
  | 'deviceLinkLost'
  | 'deviceDiagnosticUnknown'
  | 'deviceError'
  | 'deviceBusy'
  | 'deviceTimedOut'
  | 'protocolMalformedFrame'
  | 'protocolSequenceGap'
  | 'actionRequested'
  | 'actionCompleted'
  | 'actionFailed'
  | 'deviceDiscovered'
  | 'deviceRejected'
  | 'protocolMessageRejected'
  | 'protocolFailed'
  | 'firmwareUpdateStarted'
  | 'firmwareUpdateCompleted'
  | 'firmwareUpdateFailed'
  | 'firmwareAvailable'
  | 'firmwareUnavailable'
  | 'firmwareFlashRequested'
  | 'firmwarePreparing'
  | 'firmwareSerialReleased'
  | 'firmwareFlasherStarted'
  | 'firmwareFlashSucceeded'
  | 'firmwareReconnectWaiting'
  | 'firmwareReconnectTimedOut'
  | 'firmwarePostFlashVerified'
  | 'firmwarePreparationRejected';
type ActivitySeverity = 'info' | 'warning' | 'error';
type ActivitySource = 'connection' | 'action' | 'device' | 'protocol' | 'firmware';
type ActivityOutcome =
  | 'observed'
  | 'started'
  | 'succeeded'
  | 'failed'
  | 'rejected'
  | 'retrying'
  | 'applied'
  | 'synchronized'
  | 'busy'
  | 'timedOut'
  | 'available'
  | 'unavailable';

interface ActivityEventDto {
  // fixed typed allowlist ONLY (ADR-0005)
  id: number;
  at: string;
  type: ActivityEventType;
  summary: string;
  severity: ActivitySeverity;
  source: ActivitySource;
  outcome: ActivityOutcome;
  metadata: ActivityMetadataDto | null;
}

interface ActivityMetadataDto {
  connection?: ConnectionState;
  retryCount: number;
  elapsedMs: number;
  diagnosticCategory?:
    'io' | 'handshake' | 'version' | 'framing' | 'checksum' | 'timeout' | 'busy' | 'badPayload';
  diagnosticCode?: number;
  firmwareVersion?: string;
  protocolVersion?: { major: number; minor: number };
  deviceIdHashShort?: string;
  capabilities?: number;
  state?: SendableState;
  personality?: 'cozy' | 'playful' | 'calm';
  selfPlay?: boolean;
  action?: 'greet' | 'pet' | 'tickle' | 'surprise' | 'comfort';
  seed?: number;
  appliedAtMs?: number;
  autonomous?: boolean;
  protocolCategory?: 'framing' | 'checksum' | 'version' | 'payload';
  payloadLen?: number;
  sequence?: number;
  skipped?: number;
  reported?: CompanionState;
  // NEVER: raw payload bytes, raw device id, usernames/paths, tokens, secrets, or tool error text.
}
```

## 5. Explicitly NOT exposed (security boundary)

- No command to read/write raw serial bytes or choose a COM port (discovery is automatic; FR-001).
- No filesystem, shell, process, or environment access.
- No command returns raw `device_id`, raw payload bytes, or file paths (redaction; ADR-0005).
- No network/socket surface (offline-first; the device link is USB only).

## 6. Testing hooks

- Contract tests assert each command's argument/return shape and that dev-only commands are absent from
  release builds (FR-028).
- A redaction test drives representative errors and asserts every emitted `ActivityEventDto` and log
  line conforms to the allowlist — no sensitive fields populated outside a `debug-payloads` dev build
  (SC-010).
- A canvas test feeds known RGBA bytes through the blit wrapper and asserts `putImageData` is called with
  them unmodified (constraint 2).

# Mascot preview amendment (2026-09-08)

Dev-only `render_preview_frame` and `open_preview_stream` accept optional `animation`:
`{ initialState, events: [{ atMs, state }] }`. Events use existing companion tokens, are ordered by
integer milliseconds, and are capped at 256. `open_preview_stream` also accepts `elapsedMs`.
Supplying animation selects externally-clocked playback. `update_preview_stream(handle, animation,
elapsedMs)` coalesces pending requests without recreating the channel. Pause/seek uses the same
history through `render_preview_frame`. Raw RGBA delivery and acknowledgement remain unchanged.
Legacy calls without animation still work. These additions do not expose any production device commands.

Mascot timelines also accept `actionEvents: [{ atMs, action, personality, seed }]`. Device Studio
maps live applied-action acknowledgments into its monotonic preview clock per `connectionGeneration`;
it retains device uptime as metadata and does not replay the initial status snapshot as a new event.
