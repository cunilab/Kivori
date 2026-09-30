//! Wire message set (contracts/protocol.md §3). The top-level [`Message`] is an **append-only** enum;
//! the `postcard` variant index is the wire tag.

use crate::error::{ByeReason, ErrorCategory};
use kivori_model::input::Direction;
use kivori_model::presentation::{PrimaryState, ValueDisplay};
use kivori_model::{Capabilities, CompanionState, MascotAction, MascotPersonality, SendableState};
use serde::{Deserialize, Serialize};

/// A handshake nonce the device must echo to prove liveness/identity.
pub type Nonce = u32;

/// An opaque device identity. Never logged raw — hashed before it reaches any log field.
pub type DeviceId = [u8; 16];

/// A firmware or desktop application version (`major.minor.patch`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FirmwareVersion {
    /// Major version.
    pub major: u16,
    /// Minor version.
    pub minor: u16,
    /// Patch version.
    pub patch: u16,
}

/// `Hello` — desktop → device: opens the handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// Desktop application version.
    pub desktop_version: FirmwareVersion,
    /// Capabilities the desktop advertises.
    pub desktop_caps: Capabilities,
    /// A fresh nonce the device must echo.
    pub nonce: Nonce,
}

/// `HelloAck` — device → desktop: answers the handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloAck {
    /// Capabilities the device advertises.
    pub device_caps: Capabilities,
    /// Opaque device identity.
    pub device_id: DeviceId,
    /// Device firmware version.
    pub firmware_version: FirmwareVersion,
    /// Echo of the `Hello` nonce.
    pub nonce_echo: Nonce,
}

/// `Ready` — desktop → device: confirms the negotiated session parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ready {
    /// Negotiated minor version (`min` of both peers).
    pub negotiated_minor: u16,
    /// Negotiated capability set (intersection of both peers).
    pub negotiated_caps: Capabilities,
}

/// `Bye` — either side: closes the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bye {
    /// Why the session is closing.
    pub reason: ByeReason,
}

/// `SetState` — desktop → device: command a sendable companion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetState {
    /// The desired sendable state (production boundary — never `booting`/`offline`).
    pub desired: SendableState,
    /// Optional canonical timeline position (ms) at which to apply the state.
    pub at_ms: Option<u32>,
}

/// `StateReport` — device → desktop: the device's current companion state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateReport {
    /// The state the device is currently showing (any of the six).
    pub reported: CompanionState,
    /// The device's current canonical elapsed time (ms).
    pub elapsed_ms: u32,
}

/// `Ping` — desktop → device: heartbeat request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ping {
    /// Desktop timestamp (ms), echoed in the `Pong`.
    pub t_ms: u32,
}

/// `Pong` — device → desktop: heartbeat reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    /// Echo of the `Ping` timestamp.
    pub t_ms_echo: u32,
    /// Device uptime (ms).
    pub uptime_ms: u32,
}

/// `Health` — device → desktop: safe device health diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// Free heap/SRAM in bytes (a safe diagnostic).
    pub free_bytes: u32,
}

/// `Diagnostic` — device → desktop: a safe, categorized diagnostic (no payload contents).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Diagnostic category.
    pub category: ErrorCategory,
    /// A category-specific code.
    pub code: u16,
}

/// `Error` — either side: a categorized protocol error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorReport {
    /// Error category.
    pub category: ErrorCategory,
    /// A category-specific code.
    pub code: u16,
}

/// `PlayMascotAction` — desktop → device: start one transient social reaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayMascotAction {
    /// Social action to perform.
    pub action: MascotAction,
    /// Personality shaping reaction motion strength.
    pub personality: MascotPersonality,
    /// Explicit seed keeping any variation deterministic across preview and hardware.
    pub seed: u32,
}

/// `MascotActionApplied` — device → desktop: confirms when a reaction entered its timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MascotActionApplied {
    /// Social action accepted by the device.
    pub action: MascotAction,
    /// Personality accepted by the device, matching the originating cue.
    pub personality: MascotPersonality,
    /// Deterministic cue seed, used to correlate an acknowledgment with one action.
    pub seed: u32,
    /// Device uptime at application, in milliseconds.
    pub applied_at_ms: u32,
}

/// Which physical control produced an event. Extensible; only `Rotary` in Slice 002.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlId {
    /// The rotary encoder knob.
    Rotary,
}

/// Semantic input. Raw electrical edges never reach the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputKind {
    /// A gesture (e.g. a rotation) has begun.
    GestureStarted,
    /// One completed, validated logical detent in the given direction.
    Detent(Direction),
    /// The gesture has ended.
    GestureEnded,
}

/// Device -> desktop physical input.
///
/// `session` is the handshake nonce of the session that produced this event; the
/// desktop rejects any event that does not match its current session.
/// `device_ms` is carried for the deferred acceleration slice and is unused here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputEvent {
    /// The handshake nonce of the session that produced this event.
    pub session: Nonce,
    /// Identifies the gesture this event belongs to; stable across a gesture's lifetime.
    pub gesture_id: u16,
    /// Which physical control produced this event.
    pub control: ControlId,
    /// What kind of input this event represents.
    pub kind: InputKind,
    /// Device-local elapsed time (ms) at which this event was produced. Unused in this slice.
    pub device_ms: u32,
}

/// Desktop -> device semantic presentation.
///
/// `primary` is the underlying truth; `value` is a transient overlay that the
/// device expires locally after `transient_ms` (0 = persistent), falling back to
/// `primary`. `revision` is strictly increasing WITHIN a session and resets with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presentation {
    /// The handshake nonce of the session this presentation applies to.
    pub session: Nonce,
    /// Strictly increasing within a session; resets with it.
    pub revision: u32,
    /// The underlying truth beneath any transient overlay.
    pub primary: PrimaryState,
    /// A transient value overlay, if any.
    pub value: Option<ValueDisplay>,
    /// How long (ms) the device should show `value` before falling back to `primary`. 0 = persistent.
    pub transient_ms: u16,
}

/// The top-level wire message.
///
/// **Append-only**: new variants are added at the end within a major version — the `postcard`
/// variant index is the wire tag, so reordering or removing variants is a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    /// Desktop → device: opens the handshake.
    Hello(Hello),
    /// Device → desktop: answers the handshake.
    HelloAck(HelloAck),
    /// Desktop → device: confirms the negotiated session.
    Ready(Ready),
    /// Either side: closes the session.
    Bye(Bye),
    /// Desktop → device: command a companion state.
    SetState(SetState),
    /// Device → desktop: reports the current companion state.
    StateReport(StateReport),
    /// Desktop → device: heartbeat request.
    Ping(Ping),
    /// Device → desktop: heartbeat reply.
    Pong(Pong),
    /// Device → desktop: health report.
    Health(Health),
    /// Device → desktop: a safe diagnostic.
    Diagnostic(Diagnostic),
    /// Either side: a categorized protocol error.
    Error(ErrorReport),
    /// Desktop requests a transient social reaction.
    PlayMascotAction(PlayMascotAction),
    /// Device confirms the reaction's canonical start time.
    MascotActionApplied(MascotActionApplied),
    /// Tag 13 — device -> desktop physical input (capability `PHYSICAL_INPUT_V1`).
    InputEvent(InputEvent),
    /// Tag 14 — desktop -> device semantic presentation (capability `PRESENTATION_V1`).
    Presentation(Presentation),
}
