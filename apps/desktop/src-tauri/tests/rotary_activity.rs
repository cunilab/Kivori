//! Slice 002 activity producers: the rotary pipeline and session-nonce failure land in the typed
//! `ActivityLog` vocabulary (no raw payloads), exercised without Tauri or serial hardware.

use std::convert::Infallible;

use kivori_desktop::activity::{ActivityEventKind, ActivityMetadata, SessionActivity};
use kivori_desktop::device::fsm::ConnectionManager;
use kivori_desktop::device::nonce::FailingNonceSource;
use kivori_desktop::device::session::{Session, SessionConfig, SessionError};
use kivori_desktop::device::transport::SerialLink;
use kivori_desktop::platform::{ActionAvailability, FakeVolumeBackend};
use kivori_desktop::runtime::device_task::RotaryPipeline;
use kivori_model::input::Direction;
use kivori_model::presentation::PrimaryState;
use kivori_model::ConnectionState;
use kivori_protocol::message::{ControlId, InputEvent, InputKind};
use kivori_protocol::ErrorCategory;

struct NullLink;

impl SerialLink for NullLink {
    type Error = Infallible;
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Infallible> {
        Ok(0)
    }
    fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        Ok(buf.len())
    }
}

fn ev(session: u32, gesture_id: u16, kind: InputKind) -> InputEvent {
    InputEvent {
        session,
        gesture_id,
        control: ControlId::Rotary,
        kind,
        device_ms: 0,
    }
}

fn kinds(observed: &[SessionActivity]) -> Vec<ActivityEventKind> {
    observed.iter().map(|o| o.kind).collect()
}

#[test]
fn nonce_unavailable_is_recorded_as_an_io_category_activity() {
    let mut session =
        Session::with_nonce_source(SessionConfig::default(), Box::new(FailingNonceSource));
    let result = session.open(&mut NullLink, &mut ConnectionManager::new());
    assert_eq!(result, Err(SessionError::NonceUnavailable));
    assert_eq!(
        session.drain_activity(),
        vec![SessionActivity::new(
            ActivityEventKind::SessionNonceUnavailable,
            Some(ActivityMetadata::HostDiagnostic {
                category: ErrorCategory::Io
            }),
        )]
    );
}

#[test]
fn rejected_input_and_failed_volume_writes_become_typed_activity() {
    let unimplemented =
        FakeVolumeBackend::with_availability(ActionAvailability::NotImplementedYet {
            target: "test",
        });
    let mut rotary = RotaryPipeline::new(&unimplemented);
    let mut presentations = Vec::new();
    let mut observed = Vec::new();

    // No session yet: stale.
    rotary.accept_inputs(
        &[ev(7, 1, InputKind::GestureStarted)],
        &unimplemented,
        &mut presentations,
        |o| observed.push(o),
    );
    rotary.on_connection_state(
        ConnectionState::Connecting,
        ConnectionState::Connected,
        Some(7),
    );
    rotary.accept_inputs(
        &[
            ev(6, 1, InputKind::GestureStarted),
            ev(7, 2, InputKind::Detent(Direction::Cw)),
            ev(7, 1, InputKind::GestureStarted),
            ev(7, 1, InputKind::Detent(Direction::Cw)),
            ev(7, 1, InputKind::Detent(Direction::Cw)),
        ],
        &unimplemented,
        &mut presentations,
        |o| observed.push(o),
    );

    assert_eq!(
        kinds(&observed),
        vec![
            ActivityEventKind::InputStaleSessionRejected,
            ActivityEventKind::InputStaleSessionRejected,
            ActivityEventKind::InputUnstartedGestureRejected,
            // Two failing detents, one failure-streak entry.
            ActivityEventKind::VolumeWriteFailed,
        ]
    );
    assert!(observed[..3].iter().all(|o| o.metadata
        == Some(ActivityMetadata::HostDiagnostic {
            category: ErrorCategory::BadPayload
        })));
    assert_eq!(presentations.len(), 2);
    assert!(presentations
        .iter()
        .all(|p| p.session == 7 && p.primary == PrimaryState::Error && p.value.is_none()));
}

#[test]
fn losing_the_audio_endpoint_is_recorded_once() {
    let available = FakeVolumeBackend::new(40);
    let gone = FakeVolumeBackend::with_availability(ActionAvailability::RuntimeUnavailable {
        reason: "no default render endpoint".to_string(),
    });
    let mut rotary = RotaryPipeline::new(&available);
    let mut observed = Vec::new();
    rotary.observe_backend_availability(&available, |o| observed.push(o));
    rotary.observe_backend_availability(&gone, |o| observed.push(o));
    rotary.observe_backend_availability(&gone, |o| observed.push(o));
    assert_eq!(kinds(&observed), vec![ActivityEventKind::AudioEndpointLost]);
}
