//! Host presence on the device thread (M3 S2): what each sleep/lock/console action does to the
//! link, the orchestrator and the timers, driven through the same functions the loop calls and
//! an in-memory device. The OS sources themselves are covered per platform.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use kivori_desktop::device::discovery::CandidateRotator;
use kivori_desktop::device::fsm::ConnectionManager;
use kivori_desktop::device::session::{Session, SessionConfig};
use kivori_desktop::device::transport::SerialLink;
use kivori_desktop::orchestrator::Orchestrator;
use kivori_desktop::platform::host_events::{
    HostEvent, HostPresence, HostPresenceTracker, SUSPEND_ACK_BUDGET,
};
use kivori_desktop::runtime::device_task::{
    acknowledge, apply_presence_action, ConnectionDeadlines, PresenceTargets,
};
use kivori_model::{Capabilities, ConnectionState, ProtocolVersion, SendableState};
use kivori_protocol::{
    decode_message, encode_message, ByeReason, FirmwareVersion, HelloAck, Message, PROTOCOL_MAJOR,
    PROTOCOL_MINOR,
};

#[derive(Default)]
struct Wires {
    to_device: VecDeque<u8>,
    from_device: VecDeque<u8>,
}

/// An in-memory link whose device end outlives the handle the loop drops.
#[derive(Clone, Default)]
struct FakeLink(Arc<Mutex<Wires>>);

impl SerialLink for FakeLink {
    type Error = Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Infallible> {
        let mut wires = self.0.lock().unwrap();
        let mut n = 0;
        while n < buf.len() {
            match wires.from_device.pop_front() {
                Some(byte) => {
                    buf[n] = byte;
                    n += 1;
                }
                None => break,
            }
        }
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        self.0.lock().unwrap().to_device.extend(buf.iter().copied());
        Ok(buf.len())
    }
}

/// Decodes everything the desktop wrote since the last drain.
fn drain(link: &FakeLink) -> Vec<Message> {
    let bytes: Vec<u8> = link.0.lock().unwrap().to_device.drain(..).collect();
    let mut scratch: heapless::Vec<u8, { kivori_protocol::MAX_FRAME }> = heapless::Vec::new();
    bytes
        .split(|&b| b == 0)
        .filter(|packet| !packet.is_empty())
        .filter_map(|packet| decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]).ok())
        .map(|(_, message)| message)
        .collect()
}

struct Rig {
    link: Option<FakeLink>,
    /// The device's end of `link`.
    device: FakeLink,
    session: Session,
    manager: ConnectionManager,
    orchestrator: Orchestrator,
    connected_port: Option<String>,
    retry_at: Option<Instant>,
    rotator: CandidateRotator,
    deadlines: ConnectionDeadlines,
}

impl Rig {
    /// A connected session whose device advertises `caps`, with `desired` as the user's state.
    fn connected(caps: Capabilities, desired: SendableState) -> Self {
        let mut link = FakeLink::default();
        let mut session = Session::new(SessionConfig::default());
        let mut manager = ConnectionManager::new();
        let mut orchestrator = Orchestrator::new();
        orchestrator.set_desired(desired);
        session.open(&mut link, &mut manager).expect("open");
        let nonce = drain(&link)
            .into_iter()
            .find_map(|m| match m {
                Message::Hello(hello) => Some(hello.nonce),
                _ => None,
            })
            .expect("hello");
        let ack = Message::HelloAck(HelloAck {
            device_caps: caps,
            device_id: [0x5A; 16],
            firmware_version: FirmwareVersion {
                major: 1,
                minor: 3,
                patch: 0,
            },
            nonce_echo: nonce,
        });
        let mut wire: heapless::Vec<u8, { kivori_protocol::MAX_WIRE }> = heapless::Vec::new();
        encode_message(
            &ack,
            ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
            0,
            &mut wire,
        )
        .expect("encode");
        link.0
            .lock()
            .unwrap()
            .from_device
            .extend(wire.iter().copied());
        session
            .pump(&mut link, &mut manager, &mut orchestrator)
            .expect("pump");
        assert_eq!(manager.state(), ConnectionState::Connected);
        let _ = drain(&link);
        let mut deadlines = ConnectionDeadlines::new();
        deadlines.on_port_opened(Duration::ZERO);
        deadlines.on_handshake_complete(Duration::ZERO);
        Self {
            device: link.clone(),
            link: Some(link),
            session,
            manager,
            orchestrator,
            connected_port: Some("COM-fake".to_string()),
            retry_at: None,
            rotator: CandidateRotator::new(),
            deadlines,
        }
    }

    /// Applies every action the tracker returns for `event`, as the device loop does. Returns
    /// whether any write failed.
    fn feed(&mut self, tracker: &mut HostPresenceTracker, event: HostEvent) -> bool {
        tracker
            .on_event(event)
            .into_iter()
            .fold(false, |failed, action| {
                failed
                    | apply_presence_action(
                        action,
                        &mut PresenceTargets {
                            session: &mut self.session,
                            manager: &mut self.manager,
                            orchestrator: &mut self.orchestrator,
                            link: &mut self.link,
                            connected_port: &mut self.connected_port,
                            retry_at: &mut self.retry_at,
                            rotator: &mut self.rotator,
                            deadlines: &mut self.deadlines,
                        },
                    )
            })
    }
}

fn takeover_caps() -> Capabilities {
    Capabilities::MASCOT_INTERACTION.union(Capabilities::HOST_TAKEOVERS_V1)
}

#[test]
fn suspending_writes_a_sleeping_bye_releases_the_link_and_acks_within_budget() {
    let mut rig = Rig::connected(takeover_caps(), SendableState::Busy);
    let mut tracker = HostPresenceTracker::new();
    let (ack_tx, ack_rx) = mpsc::sync_channel(1);

    // The OS callback waits on a different thread, as it does for real.
    let waiter = std::thread::spawn(move || {
        let started = Instant::now();
        ack_rx
            .recv_timeout(SUSPEND_ACK_BUDGET)
            .map(|()| started.elapsed())
    });
    let failed = rig.feed(&mut tracker, HostEvent::Suspending);
    acknowledge(Some(&ack_tx));
    assert!(!failed);
    let waited = waiter.join().expect("waiter").expect("ack arrives");
    assert!(waited < SUSPEND_ACK_BUDGET);

    let sent = drain(&rig.device);
    assert!(
        sent.iter()
            .any(|m| matches!(m, Message::Bye(bye) if bye.reason == ByeReason::HostSleeping)),
        "got {sent:?}"
    );
    assert!(rig.link.is_none(), "the handle is dropped");
    assert_eq!(rig.manager.state(), ConnectionState::Disconnected);
    assert_eq!(rig.session.current_session(), None);
    assert!(!rig.deadlines.heartbeat_due(Duration::from_secs(60)));
    assert_eq!(tracker.presence(), HostPresence::Sleeping);
    assert!(!tracker.may_use_port());
}

/// The same flow, reading the Bye straight off the link the loop wrote to.
#[test]
fn the_goodbye_is_host_sleeping_for_a_device_that_knows_it_and_shutdown_for_one_that_does_not() {
    for (caps, expected) in [
        (takeover_caps(), ByeReason::HostSleeping),
        (Capabilities::MASCOT_INTERACTION, ByeReason::Shutdown),
    ] {
        let mut rig = Rig::connected(caps, SendableState::Idle);
        let mut link = rig.link.take().expect("link");
        let notice = apply_notice(&mut rig.session, &mut link);
        assert!(notice);
        let sent = drain(&link);
        assert!(
            sent.iter()
                .any(|m| matches!(m, Message::Bye(bye) if bye.reason == expected)),
            "{expected:?} expected, got {sent:?}"
        );
    }
}

fn apply_notice(session: &mut Session, link: &mut FakeLink) -> bool {
    kivori_desktop::runtime::device_task::send_sleep_notice(session, link).is_ok()
}

#[test]
fn lock_shows_sleeping_and_unlock_restores_the_users_state() {
    let mut rig = Rig::connected(takeover_caps(), SendableState::Busy);
    let mut tracker = HostPresenceTracker::new();

    assert!(!rig.feed(&mut tracker, HostEvent::Locked));
    assert_eq!(rig.orchestrator.desired(), SendableState::Sleeping);
    assert_eq!(rig.orchestrator.user_desired(), SendableState::Busy);
    let sent = drain(&rig.device);
    assert!(sent
        .iter()
        .any(|m| matches!(m, Message::SetState(s) if s.desired == SendableState::Sleeping)));
    assert_eq!(rig.manager.state(), ConnectionState::Connected);

    assert!(!rig.feed(&mut tracker, HostEvent::Unlocked));
    let sent = drain(&rig.device);
    assert!(sent
        .iter()
        .any(|m| matches!(m, Message::SetState(s) if s.desired == SendableState::Busy)));
    assert_eq!(rig.orchestrator.desired(), SendableState::Busy);
}

#[test]
fn a_state_change_while_locked_stays_hidden_until_unlock() {
    let mut rig = Rig::connected(takeover_caps(), SendableState::Idle);
    let mut tracker = HostPresenceTracker::new();
    rig.feed(&mut tracker, HostEvent::Locked);
    let _ = drain(&rig.device);

    rig.session
        .set_desired(
            rig.link.as_mut().expect("link"),
            &rig.manager,
            &mut rig.orchestrator,
            SendableState::Happy,
        )
        .expect("set");
    let sent = drain(&rig.device);
    assert!(sent
        .iter()
        .any(|m| matches!(m, Message::SetState(s) if s.desired == SendableState::Sleeping)));

    rig.feed(&mut tracker, HostEvent::Unlocked);
    assert_eq!(rig.orchestrator.desired(), SendableState::Happy);
}

#[test]
fn resume_with_no_device_clears_the_wait_and_creates_no_backoff() {
    let mut rig = Rig::connected(takeover_caps(), SendableState::Idle);
    let mut tracker = HostPresenceTracker::new();
    rig.feed(&mut tracker, HostEvent::Suspending);
    // A stale retry from before the sleep must not delay the first look after the wake.
    rig.retry_at = Some(Instant::now() + Duration::from_secs(30));

    rig.feed(&mut tracker, HostEvent::Resumed);
    assert_eq!(rig.retry_at, None);
    assert_eq!(rig.manager.retry_count(), 0);
    assert_eq!(rig.manager.state(), ConnectionState::Disconnected);
    assert!(tracker.may_use_port());
    assert_eq!(tracker.presence(), HostPresence::Active);
}

#[test]
fn console_loss_releases_the_port_and_return_rediscovers() {
    let mut rig = Rig::connected(takeover_caps(), SendableState::Idle);
    let mut tracker = HostPresenceTracker::new();
    rig.feed(&mut tracker, HostEvent::ConsoleDisconnected);
    assert!(rig.link.is_none());
    assert_eq!(rig.connected_port, None);
    assert!(!tracker.may_use_port());

    rig.retry_at = Some(Instant::now() + Duration::from_secs(30));
    rig.feed(&mut tracker, HostEvent::ConsoleConnected);
    assert_eq!(rig.retry_at, None);
    assert!(tracker.may_use_port());
}

#[test]
fn a_sleep_drops_input_queued_before_it() {
    // Gate 2: the Bye closes the session, which discards queued input, and the manager leaves
    // Connected, which is what `synchronize_session` keys the session boundary on.
    let mut rig = Rig::connected(takeover_caps(), SendableState::Idle);
    let mut tracker = HostPresenceTracker::new();
    rig.feed(&mut tracker, HostEvent::Suspending);
    assert!(rig.session.take_input_events().is_empty());
    assert_eq!(rig.session.current_session(), None);
    assert!(!rig.manager.state().can_drive_device());
}
