//! The USB interrupt's service step ([`usb_service::service`]) against a model of the RP2040 USB
//! controller, which keeps its OUT buffer flag set until the packet is read. The real controller
//! needs the chip (docs/virtual-pip.md, Limits); this is the contract the receive queue, and with
//! it the run loop, depend on. Host simulation only.
#![cfg(feature = "host-sim")]

use std::collections::VecDeque;

use heapless::{Deque, Vec as HVec};
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::{Clock, InputSource, Transport};
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::runtime::{Runtime, RuntimeConfig};
use kivori_firmware::sim::{CaptureDisplay, VirtualClock};
use kivori_firmware::usb_service::{service, UsbPort, PACKET_BYTES};
use kivori_model::input::InputLevels;
use kivori_model::{Capabilities, ProtocolVersion};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, Message, MAX_FRAME, MAX_WIRE,
    PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

/// The size of the firmware's receive queue (`transport::RX_BYTES_BUFFERED`).
const RX_BYTES: usize = 1024;
type Rx = Deque<u8, RX_BYTES>;

/// OUT packets the host sent, as the controller holds them.
#[derive(Default)]
struct Controller {
    out: VecDeque<Vec<u8>>,
    polls: usize,
}

impl Controller {
    /// The host sends `bytes`, split into full-speed packets.
    fn host_sends(&mut self, bytes: &[u8]) {
        for packet in bytes.chunks(PACKET_BYTES) {
            self.out.push_back(packet.to_vec());
        }
    }

    /// The interrupt line: the RP2040 raises `USBCTRL_IRQ` for as long as an OUT packet is unread.
    fn irq_raised(&self) -> bool {
        !self.out.is_empty()
    }
}

impl UsbPort for Controller {
    fn poll(&mut self) {
        // `usb-device` services the endpoint but does not read the data: the flag stays set.
        self.polls += 1;
    }

    fn read_packet(&mut self, buf: &mut [u8]) -> Option<usize> {
        let packet = self.out.pop_front()?;
        buf[..packet.len()].copy_from_slice(&packet);
        Some(packet.len())
    }
}

fn bytes(range: std::ops::Range<usize>) -> Vec<u8> {
    range.map(|i| (i % 251) as u8 + 1).collect()
}

fn drained(rx: &mut Rx) -> Vec<u8> {
    std::iter::from_fn(|| rx.pop_front()).collect()
}

#[test]
fn one_service_clears_the_interrupt_and_queues_the_bytes_in_order() {
    let mut usb = Controller::default();
    let mut rx = Rx::new();
    let sent = bytes(0..40);
    usb.host_sends(&sent);
    assert!(usb.irq_raised());

    let done = service(&mut usb, &mut rx);

    assert!(!usb.irq_raised(), "an OUT packet was left in the endpoint");
    assert_eq!((done.packets, done.queued, done.dropped), (1, 40, 0));
    assert_eq!(drained(&mut rx), sent);
}

#[test]
fn every_pending_packet_is_read_in_one_service() {
    let mut usb = Controller::default();
    let mut rx = Rx::new();
    let sent = bytes(0..64 * 5 + 9);
    usb.host_sends(&sent);

    let done = service(&mut usb, &mut rx);

    assert!(!usb.irq_raised());
    assert_eq!(done.packets, 6);
    assert_eq!(done.queued, sent.len());
    assert_eq!(drained(&mut rx), sent);
}

#[test]
fn nothing_pending_polls_and_changes_nothing() {
    let mut usb = Controller::default();
    let mut rx = Rx::new();
    assert_eq!(service(&mut usb, &mut rx), Default::default());
    assert_eq!(usb.polls, 1);
    assert!(rx.is_empty());
}

#[test]
fn overflow_is_dropped_without_blocking_and_later_packets_still_flow() {
    let mut usb = Controller::default();
    let mut rx = Rx::new();
    // The run loop is not draining: more arrives than the queue holds.
    let flood = bytes(0..RX_BYTES + 300);
    usb.host_sends(&flood);

    let done = service(&mut usb, &mut rx);

    assert!(
        !usb.irq_raised(),
        "the flag must clear even when the queue is full"
    );
    assert_eq!(done.queued, RX_BYTES);
    assert_eq!(done.dropped, 300);
    assert_eq!(rx.len(), RX_BYTES);
    // The oldest bytes are the ones kept.
    assert_eq!(drained(&mut rx), flood[..RX_BYTES]);

    // The loop caught up: the next packets are queued again.
    let later = bytes(1000..1100);
    usb.host_sends(&later);
    let done = service(&mut usb, &mut rx);
    assert_eq!((done.queued, done.dropped), (100, 0));
    assert_eq!(drained(&mut rx), later);
}

/// What the interrupt handler did before the fix: poll the stack and leave the OUT packet to the run
/// loop. The controller keeps the flag set, so the interrupt is re-raised after every service, which
/// on the unit starved the main loop (0 bytes in, 36 queued bytes never delivered, a 750 ms loop).
#[test]
fn regression_a_poll_only_service_leaves_the_interrupt_raised_forever() {
    let mut usb = Controller::default();
    usb.host_sends(&bytes(0..36));
    for storm in 1..=1000 {
        usb.poll();
        assert!(usb.irq_raised(), "storm ended after {storm} services");
    }
    assert_eq!(usb.polls, 1000);

    // The production service ends it at the first interrupt.
    let mut rx = Rx::new();
    service(&mut usb, &mut rx);
    assert!(!usb.irq_raised());
    assert_eq!(rx.len(), 36);
}

/// Host bytes reach the runtime only through the service step: the transport the run loop reads is
/// the receive queue, as on the device.
struct UsbLink {
    usb: Controller,
    rx: Rx,
    /// Bytes the device wrote to the host.
    sent: Vec<u8>,
    /// Service as the fixed firmware does (read the packets), or as the old one did (poll only).
    reads_packets: bool,
}

impl UsbLink {
    /// One `USBCTRL_IRQ`: it fires for as long as the controller holds an event.
    fn interrupt(&mut self) {
        if !self.usb.irq_raised() {
            return;
        }
        if self.reads_packets {
            service(&mut self.usb, &mut self.rx);
        } else {
            self.usb.poll();
        }
    }
}

impl Transport for UsbLink {
    type Error = ();

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        let n = buf.len().min(self.rx.len());
        for slot in &mut buf[..n] {
            *slot = self.rx.pop_front().expect("length checked");
        }
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, ()> {
        self.sent.extend_from_slice(buf);
        Ok(buf.len())
    }
}

struct NoInput;

impl InputSource for NoInput {
    fn sample(&mut self) -> InputLevels {
        InputLevels::default()
    }

    fn drain(&mut self, now_ms: u32, f: &mut dyn FnMut(InputLevels, u32)) {
        f(InputLevels::default(), now_ms);
    }
}

/// Sends a `Hello` over the modelled USB link and runs the production runtime for 200 ms.
/// Returns the messages the host received.
fn handshake(reads_packets: bool) -> Vec<Message> {
    let blob_bytes: &'static [u8] = Box::leak(compile_default_blob().into_boxed_slice());
    let blob = AssetBlob::parse(blob_bytes).expect("valid blob");
    let version = FirmwareVersion {
        major: 1,
        minor: 0,
        patch: 0,
    };
    let caps = Capabilities::PHYSICAL_INPUT_V1;
    let mut runtime = Runtime::new(
        DeviceIdentity {
            device_id: [0xAB; 16],
            firmware_version: version,
            capabilities: caps,
        },
        RuntimeConfig::default(),
    );
    let clock = VirtualClock::new();
    let mut display = Box::new(CaptureDisplay::new());
    let mut link = UsbLink {
        usb: Controller::default(),
        rx: Rx::new(),
        sent: Vec::new(),
        reads_packets,
    };

    let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
    encode_message(
        &Message::Hello(Hello {
            desktop_version: version,
            desktop_caps: caps,
            nonce: 7,
        }),
        ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR),
        0,
        &mut wire,
    )
    .expect("encode");
    link.usb.host_sends(&wire);

    while clock.now_ms() < 200 {
        link.interrupt();
        clock.advance(2);
        runtime.step(&clock, &mut link, &mut NoInput, display.as_mut(), &blob);
    }

    let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
    link.sent
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .filter_map(|p| decode_message(p, &mut scratch, &[PROTOCOL_MAJOR]).ok())
        .map(|(_, message)| message)
        .collect()
}

#[test]
fn the_handshake_completes_when_host_bytes_arrive_only_through_the_service() {
    assert!(
        handshake(true)
            .iter()
            .any(|m| matches!(m, Message::HelloAck(_))),
        "no HelloAck"
    );
}

#[test]
fn regression_without_reading_the_packets_the_device_never_answers() {
    assert!(
        !handshake(false)
            .iter()
            .any(|m| matches!(m, Message::HelloAck(_))),
        "the host's bytes got through"
    );
}
