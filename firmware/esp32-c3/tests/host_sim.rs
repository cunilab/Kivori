//! Host simulation tests for the device core (T068; FR-035, SC-008). No hardware required.
#![cfg(feature = "host-sim")]

use heapless::Vec as HVec;
use kivori_firmware::ports::Transport;
use kivori_firmware::proto::{DeviceIdentity, DispatchError, Dispatcher};
use kivori_firmware::sim::SimPipe;
use kivori_firmware::state::{DeviceEvent, DeviceState};
use kivori_model::{Capabilities, CompanionState, ProtocolVersion, SendableState};
use kivori_protocol::{
    decode_message, encode_message, Bye, ByeReason, FirmwareVersion, Hello, Message, Ping,
    SetState, MAX_FRAME, MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: [0xAB; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: Capabilities::NONE,
    }
}

fn wire_version() -> ProtocolVersion {
    ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR)
}

/// Host → device: frames `msg` with sequence `seq` onto the pipe.
fn host_write(pipe: &mut SimPipe, msg: &Message, seq: u16) {
    let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
    encode_message(msg, wire_version(), seq, &mut wire).expect("encode");
    pipe.host_send(&wire).expect("pipe has capacity");
}

/// Decodes every complete device→host frame currently queued.
fn host_drain(pipe: &mut SimPipe) -> Vec<Message> {
    let bytes = pipe.host_recv();
    let mut messages = Vec::new();
    let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
    for packet in bytes.split(|&b| b == 0) {
        if packet.is_empty() {
            continue;
        }
        if let Ok((_, msg)) = decode_message(packet, &mut scratch, &[PROTOCOL_MAJOR]) {
            messages.push(msg);
        }
    }
    messages
}

#[test]
fn responds_to_hello_with_identity_and_nonce_echo() {
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());

    host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::NONE,
            nonce: 0xDEAD_BEEF,
        }),
        0,
    );
    dispatcher.poll(&mut pipe, &mut device, 10).expect("poll");

    match host_drain(&mut pipe).as_slice() {
        [Message::HelloAck(ack)] => {
            assert_eq!(ack.nonce_echo, 0xDEAD_BEEF, "nonce must be echoed");
            assert_eq!(ack.device_id, [0xAB; 16]);
            assert_eq!(ack.firmware_version.major, 1);
        }
        other => panic!("expected a single HelloAck, got {other:?}"),
    }
}

#[test]
fn applies_set_state_and_reports_it() {
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());
    assert_eq!(
        device.apply(DeviceEvent::BootComplete),
        Some(CompanionState::Offline)
    );

    host_write(
        &mut pipe,
        &Message::SetState(SetState {
            desired: SendableState::Happy,
            at_ms: None,
        }),
        0,
    );
    dispatcher.poll(&mut pipe, &mut device, 250).expect("poll");

    assert_eq!(device.current(), CompanionState::Happy);
    match host_drain(&mut pipe).as_slice() {
        [Message::StateReport(report)] => {
            assert_eq!(report.reported, CompanionState::Happy);
            assert_eq!(report.elapsed_ms, 250);
        }
        other => panic!("expected a single StateReport, got {other:?}"),
    }
}

#[test]
fn ping_is_answered_with_pong_echo() {
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());

    host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::NONE,
            nonce: 7,
        }),
        0,
    );
    dispatcher.poll(&mut pipe, &mut device, 10).expect("poll");
    let _ = host_drain(&mut pipe);

    host_write(&mut pipe, &Message::Ping(Ping { t_ms: 4242 }), 1);
    dispatcher.poll(&mut pipe, &mut device, 900).expect("poll");

    match host_drain(&mut pipe).as_slice() {
        [Message::Pong(pong)] => {
            assert_eq!(pong.t_ms_echo, 4242);
            assert_eq!(pong.uptime_ms, 900);
        }
        other => panic!("expected a single Pong, got {other:?}"),
    }
}

#[test]
fn ping_is_silent_without_an_accepted_session() {
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());

    host_write(&mut pipe, &Message::Ping(Ping { t_ms: 1 }), 0);
    dispatcher.poll(&mut pipe, &mut device, 10).expect("poll");
    assert!(host_drain(&mut pipe).is_empty(), "pre-Hello Ping");

    host_write(
        &mut pipe,
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::NONE,
            nonce: 7,
        }),
        1,
    );
    dispatcher.poll(&mut pipe, &mut device, 20).expect("poll");
    let _ = host_drain(&mut pipe);
    host_write(
        &mut pipe,
        &Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        2,
    );
    dispatcher.poll(&mut pipe, &mut device, 30).expect("poll");
    host_write(&mut pipe, &Message::Ping(Ping { t_ms: 2 }), 0);
    dispatcher.poll(&mut pipe, &mut device, 40).expect("poll");
    assert!(host_drain(&mut pipe).is_empty(), "post-Bye Ping");
}

#[test]
fn a_new_hello_nonce_starts_a_fresh_sequence_baseline() {
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());
    let hello = |nonce| {
        Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: Capabilities::NONE,
            nonce,
        })
    };

    host_write(&mut pipe, &hello(11), 0);
    dispatcher.poll(&mut pipe, &mut device, 10).expect("poll");
    let _ = host_drain(&mut pipe);

    host_write(&mut pipe, &hello(22), 0);
    dispatcher.poll(&mut pipe, &mut device, 20).expect("poll");
    assert!(matches!(
        host_drain(&mut pipe).as_slice(),
        [Message::HelloAck(ack)] if ack.nonce_echo == 22
    ));

    host_write(&mut pipe, &hello(22), 0);
    dispatcher.poll(&mut pipe, &mut device, 30).expect("poll");
    assert!(
        host_drain(&mut pipe).is_empty(),
        "a same-session duplicate must remain suppressed"
    );
}

#[test]
fn incomplete_writes_are_reported_and_repaired() {
    struct ShortWrite {
        accepted: usize,
        wrote_once: bool,
        discarded: bool,
    }

    impl Transport for ShortWrite {
        type Error = ();

        fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
            Ok(0)
        }

        fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            if core::mem::replace(&mut self.wrote_once, true) {
                Ok(0)
            } else {
                Ok(self.accepted.min(buf.len()))
            }
        }

        fn discard_unsent(&mut self) {
            self.discarded = true;
        }
    }

    for accepted in [0, 1] {
        let mut transport = ShortWrite {
            accepted,
            wrote_once: false,
            discarded: false,
        };
        let result =
            Dispatcher::new(identity()).emit(&mut transport, &Message::Ping(Ping { t_ms: 1 }));
        assert!(matches!(
            result,
            Err(DispatchError::IncompleteWrite {
                written,
                expected,
            }) if written == accepted && expected > written
        ));
        assert!(transport.discarded, "accepted {accepted} bytes");
    }
}

#[test]
fn progressing_short_writes_complete_the_frame() {
    struct ProgressWrite {
        writes: usize,
        discarded: bool,
    }

    impl Transport for ProgressWrite {
        type Error = ();

        fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
            Ok(0)
        }

        fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            self.writes += 1;
            Ok(buf.len().min(3))
        }

        fn discard_unsent(&mut self) {
            self.discarded = true;
        }
    }

    let mut transport = ProgressWrite {
        writes: 0,
        discarded: false,
    };
    Dispatcher::new(identity())
        .emit(&mut transport, &Message::Ping(Ping { t_ms: 1 }))
        .expect("positive short writes make bounded progress");
    assert!(transport.writes > 1);
    assert!(!transport.discarded);
}

#[test]
fn a_transport_error_repairs_a_partially_written_frame() {
    struct FailingWrite {
        first: bool,
        discarded: bool,
    }

    impl Transport for FailingWrite {
        type Error = ();

        fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
            Ok(0)
        }

        fn write(&mut self, _buf: &[u8]) -> Result<usize, Self::Error> {
            if core::mem::replace(&mut self.first, false) {
                Ok(1)
            } else {
                Err(())
            }
        }

        fn discard_unsent(&mut self) {
            self.discarded = true;
        }
    }

    let mut transport = FailingWrite {
        first: true,
        discarded: false,
    };
    assert!(Dispatcher::new(identity())
        .emit(&mut transport, &Message::Ping(Ping { t_ms: 1 }))
        .is_err());
    assert!(transport.discarded);
}

#[test]
fn bye_drops_the_link_to_offline() {
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());
    device.apply(DeviceEvent::BootComplete);
    device.apply(DeviceEvent::SetState(SendableState::Busy));
    assert_eq!(device.current(), CompanionState::Busy);

    host_write(
        &mut pipe,
        &Message::Bye(Bye {
            reason: ByeReason::Shutdown,
        }),
        0,
    );
    dispatcher.poll(&mut pipe, &mut device, 1000).expect("poll");

    assert_eq!(device.current(), CompanionState::Offline);
}

#[test]
fn direct_link_down_transitions_to_offline() {
    let mut device = DeviceState::new();
    device.apply(DeviceEvent::BootComplete);
    device.apply(DeviceEvent::SetState(SendableState::Idle));
    assert_eq!(device.current(), CompanionState::Idle);
    assert_eq!(
        device.apply(DeviceEvent::LinkDown),
        Some(CompanionState::Offline)
    );
    // Re-applying LinkDown when already offline reports no change.
    assert_eq!(device.apply(DeviceEvent::LinkDown), None);
}

#[test]
fn malformed_frames_are_dropped_without_panic() {
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());
    device.apply(DeviceEvent::BootComplete);

    // Garbage COBS packets + delimiters: must not panic, change state, or respond.
    pipe.host_send(&[0x02, 0xFF, 0x00, 0x13, 0x37, 0x00])
        .expect("capacity");
    dispatcher
        .poll(&mut pipe, &mut device, 5)
        .expect("poll survives garbage");

    assert_eq!(device.current(), CompanionState::Offline);
    assert!(
        host_drain(&mut pipe).is_empty(),
        "no response to malformed input"
    );
}

#[test]
fn duplicate_sequence_is_not_reapplied() {
    // A duplicated sequence number must not re-apply side effects (§6): the second command carries a
    // different desired state but the same seq, so it must be ignored.
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let mut dispatcher = Dispatcher::new(identity());
    device.apply(DeviceEvent::BootComplete);

    host_write(
        &mut pipe,
        &Message::SetState(SetState {
            desired: SendableState::Happy,
            at_ms: None,
        }),
        0,
    );
    dispatcher.poll(&mut pipe, &mut device, 100).expect("poll");
    host_write(
        &mut pipe,
        &Message::SetState(SetState {
            desired: SendableState::Busy,
            at_ms: None,
        }),
        0, // duplicate seq
    );
    dispatcher.poll(&mut pipe, &mut device, 200).expect("poll");

    assert_eq!(
        device.current(),
        CompanionState::Happy,
        "duplicate must not change state"
    );
    assert_eq!(
        host_drain(&mut pipe).len(),
        1,
        "duplicate must not re-report"
    );
}
