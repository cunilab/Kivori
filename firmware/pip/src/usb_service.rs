//! The USB interrupt's service step, hardware-neutral so the host tests can drive it (issue #179).
//!
//! The RP2040 USB controller keeps its OUT buffer flag set until the packet is read, so an interrupt
//! handler that only polls the stack is raised again at once, forever, and starves the run loop (the
//! first real unit enumerated and then answered nothing). [`service`] therefore polls and then reads
//! every pending OUT packet into the receive queue the run loop drains.

/// The most the controller delivers in one OUT packet (a full-speed bulk endpoint).
pub const PACKET_BYTES: usize = 64;

/// The USB stack as the service step needs it; `transport` implements it for `usb-device` plus
/// `usbd-serial`, and the host tests for a model of the controller.
pub trait UsbPort {
    /// Services the stack (`UsbDevice::poll`).
    fn poll(&mut self);

    /// Moves one pending OUT packet into `buf`, returning its length, or `None` when none is
    /// waiting (`WouldBlock`) or the read failed.
    fn read_packet(&mut self, buf: &mut [u8]) -> Option<usize>;
}

/// What one [`service`] call did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Serviced {
    /// OUT packets read from the stack.
    pub packets: usize,
    /// Bytes put in the receive queue.
    pub queued: usize,
    /// Bytes read but dropped because the queue was full.
    pub dropped: usize,
}

/// Polls `usb`, then reads packets until none is pending, appending their bytes to `rx` in order.
/// A full queue drops the excess instead of blocking (the framing's CRC and delimiters resynchronise
/// the protocol); the packets are read regardless, so the controller's flag is always cleared.
pub fn service<const N: usize>(
    usb: &mut impl UsbPort,
    rx: &mut heapless::Deque<u8, N>,
) -> Serviced {
    usb.poll();
    let mut done = Serviced::default();
    let mut packet = [0u8; PACKET_BYTES];
    while let Some(n) = usb.read_packet(&mut packet).filter(|&n| n > 0) {
        done.packets += 1;
        for &byte in &packet[..n] {
            if rx.push_back(byte).is_ok() {
                done.queued += 1;
            } else {
                done.dropped += 1;
            }
        }
    }
    done
}
