//! The outbound queue over a model of the RP2040 USB CDC link (`transport::UsbCdcTransport`
//! on `usbd-serial`): a 128-byte transmit buffer, at most one 64-byte packet offered per `write`,
//! `WouldBlock` (reported as 0 bytes taken) while it is full, and a host that reads one packet per
//! USB frame or nothing at all. The real transport needs the chip; this is the contract the run
//! loop depends on, host-tested. Host simulation only.
#![cfg(feature = "host-sim")]

use std::collections::VecDeque;
use std::convert::Infallible;

use kivori_firmware::ports::Transport;
use kivori_firmware::tx_buffer::{TxBuffered, FIFO_BYTES, TX_QUEUE_BYTES};
use kivori_protocol::MAX_WIRE;

/// `usbd-serial`'s transmit buffer.
const SERIAL_BUFFER: usize = 128;

/// What `UsbCdcTransport::write` presents to the run loop.
#[derive(Default)]
struct UsbModel {
    /// Bytes the serial class holds, not yet sent to the host.
    buffer: VecDeque<u8>,
    /// Every byte the host has received, in order.
    host: Vec<u8>,
    /// The longest slice any `write` call was handed.
    longest_write: usize,
    /// The host is not reading.
    stalled: bool,
}

impl UsbModel {
    /// One USB frame: a reading host takes one packet.
    fn host_frame(&mut self) {
        if self.stalled {
            return;
        }
        for _ in 0..FIFO_BYTES.min(self.buffer.len()) {
            self.host
                .push(self.buffer.pop_front().expect("length checked"));
        }
    }
}

impl Transport for UsbModel {
    type Error = Infallible;

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Ok(0)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.longest_write = self.longest_write.max(buf.len());
        // The adapter never offers the stack more than one packet; the stack takes what fits.
        let offered = buf.len().min(FIFO_BYTES);
        let taken = offered.min(SERIAL_BUFFER - self.buffer.len());
        self.buffer.extend(&buf[..taken]);
        Ok(taken)
    }
}

/// A frame as the dispatcher writes it: nonzero payload of one value, then the 0 delimiter.
fn frame(value: u8, len: usize) -> Vec<u8> {
    assert!(value != 0 && len >= 2);
    let mut bytes = vec![value; len - 1];
    bytes.push(0);
    bytes
}

fn rig(stalled: bool) -> TxBuffered<UsbModel> {
    TxBuffered::new(UsbModel {
        stalled,
        ..UsbModel::default()
    })
}

#[test]
fn the_usb_stack_is_never_offered_more_than_one_packet_per_write() {
    let mut tx = rig(false);
    // Frames far larger than a packet, each followed by the USB frames a host needs to read it.
    for round in 0..40u8 {
        assert_eq!(tx.write(&frame(round + 1, MAX_WIRE)), Ok(MAX_WIRE));
        for _ in 0..MAX_WIRE.div_ceil(FIFO_BYTES) {
            tx.inner_mut().host_frame();
            let _ = tx.pump();
        }
    }
    assert!(tx.inner_mut().longest_write <= FIFO_BYTES);
    assert!(tx.inner_mut().longest_write > 0);
}

#[test]
fn a_write_reports_exactly_the_frame_or_nothing_never_a_part() {
    let mut tx = rig(true);
    let mut accepted = 0;
    let mut refused = false;
    for round in 0..20u8 {
        let n = tx.write(&frame(round + 1, 100)).unwrap();
        assert!(n == 0 || n == 100, "a frame was split: {n} of 100");
        refused |= n == 0;
        accepted += n;
        // Room: the queue plus what the serial buffer holds, and not a byte more.
        assert!(tx.pending() <= TX_QUEUE_BYTES);
        assert!(accepted <= TX_QUEUE_BYTES + SERIAL_BUFFER);
    }
    assert!(
        refused,
        "a host that never reads must eventually be refused"
    );
    // The refusal came with the buffers nearly full: less than one frame of room was left.
    assert!(accepted + 100 > TX_QUEUE_BYTES + SERIAL_BUFFER);
}

#[test]
fn a_host_that_resumes_gets_every_accepted_byte_once_in_order() {
    let mut tx = rig(true);
    let mut sent = Vec::new();
    for round in 0..20u8 {
        let f = frame(round + 1, 90);
        if tx.write(&f) == Ok(f.len()) {
            sent.extend(f);
        }
    }
    assert!(sent.len() > SERIAL_BUFFER, "the queue held a backlog");
    tx.inner_mut().stalled = false;
    for _ in 0..200 {
        let _ = tx.pump();
        tx.inner_mut().host_frame();
    }
    assert_eq!(tx.pending(), 0);
    assert_eq!(tx.inner_mut().host, sent);
}

#[test]
fn a_host_reading_slower_than_the_device_writes_loses_and_reorders_nothing() {
    // The device writes a frame every USB frame while the host reads only every third one.
    let mut tx = rig(false);
    let mut sent = Vec::new();
    for tick in 0..300u32 {
        let f = frame((tick % 250) as u8 + 1, 40 + (tick % 7) as usize);
        if tx.write(&f) == Ok(f.len()) {
            sent.extend(f);
        }
        if tick % 3 == 0 {
            tx.inner_mut().host_frame();
        }
    }
    for _ in 0..300 {
        let _ = tx.pump();
        tx.inner_mut().host_frame();
    }
    assert_eq!(tx.inner_mut().host, sent);
}

#[test]
fn discard_unsent_drops_the_backlog_and_closes_a_frame_the_usb_stack_already_holds() {
    let mut tx = rig(true);
    let mut complete = Vec::new();
    for round in 0..20u8 {
        let f = frame(round + 1, 90);
        if tx.write(&f) == Ok(f.len()) {
            complete.push(f);
        }
    }
    assert!(tx.pending() > 0);
    tx.discard_unsent();
    assert_eq!(tx.pending(), 1, "only the closing delimiter is left");

    // The host starts reading, and a new session's first frame goes out behind the delimiter.
    tx.inner_mut().stalled = false;
    let fresh = frame(0xEE, 60);
    assert_eq!(tx.write(&fresh), Ok(fresh.len()));
    for _ in 0..50 {
        let _ = tx.pump();
        tx.inner_mut().host_frame();
    }
    let received = tx.inner_mut().host.clone();
    let packets: Vec<&[u8]> = received
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .collect();
    // The new frame arrives whole and last; everything before it is stale frames from the old
    // session, at most the ones the stack already held (128 bytes), the last possibly cut short.
    assert_eq!(packets.last().copied(), Some(&fresh[..fresh.len() - 1]));
    let stale = received.len() - fresh.len();
    assert!(
        stale <= SERIAL_BUFFER + 1,
        "{stale} stale bytes reached the host"
    );
    assert!(
        complete.len() > packets.len() - 1,
        "the backlog was not dropped: {} frames for {} accepted",
        packets.len() - 1,
        complete.len()
    );
}

#[test]
fn discard_unsent_with_nothing_queued_writes_nothing() {
    let mut tx = rig(false);
    tx.discard_unsent();
    assert_eq!(tx.pending(), 0);
    // A frame the host has fully received needs no closing delimiter.
    assert_eq!(tx.write(&frame(5, 30)), Ok(30));
    tx.inner_mut().host_frame();
    tx.discard_unsent();
    assert_eq!(tx.pending(), 0);
}

#[test]
fn the_queue_reports_room_and_a_stalled_host_never_blocks_a_write() {
    let mut tx = rig(true);
    // Fill the queue from empty with a stalled host: every call returns at once.
    for round in 0..64u8 {
        let _ = tx.write(&frame(round % 250 + 1, 64));
    }
    let free = tx.tx_free();
    assert!(free < 64, "the queue is full, {free} B left");
    assert_eq!(tx.write(&frame(1, 64)), Ok(0));
    // Draining the hardware frees room again.
    tx.inner_mut().stalled = false;
    for _ in 0..40 {
        tx.inner_mut().host_frame();
        let _ = tx.drain_pending();
    }
    assert_eq!(tx.tx_free(), TX_QUEUE_BYTES);
}
