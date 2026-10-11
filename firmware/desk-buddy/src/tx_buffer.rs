//! Whole-frame outbound queue over a bounded, non-blocking transport (hardware-neutral, so it is
//! host-tested; the USB Serial/JTAG adapter in `transport` is its production inner transport).

use crate::ports::Transport;
use heapless::Deque;
use kivori_protocol::MAX_WIRE;

/// Endpoint FIFO depth in bytes. One `write` call never offers the hardware more than this.
pub const FIFO_BYTES: usize = 64;

/// Outbound queue capacity: two full wire packets, so a whole frame always fits even with a partial
/// packet still draining.
pub const TX_QUEUE_BYTES: usize = MAX_WIRE * 2;

/// A [`Transport`] wrapper giving the dispatcher whole-frame writes over a bounded hardware FIFO.
///
/// `write` enqueues (so a frame is never truncated by a full FIFO), `read` delegates straight through, and
/// [`Self::pump`] moves queued bytes toward the hardware without blocking. The run loop pumps every tick.
pub struct TxBuffered<T> {
    inner: T,
    queue: Deque<u8, TX_QUEUE_BYTES>,
}

impl<T: Transport> TxBuffered<T> {
    /// Wraps `inner` with an empty outbound queue.
    #[must_use]
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            queue: Deque::new(),
        }
    }

    /// Bytes still waiting to reach the hardware.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Borrows the wrapped transport (for adapter-specific calls such as blocking marker writes).
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Pushes as much of the queue to the hardware as it accepts right now. Returns bytes written.
    ///
    /// # Errors
    /// The wrapped transport's error, unchanged.
    pub fn pump(&mut self) -> Result<usize, T::Error> {
        let mut written = 0;
        while !self.queue.is_empty() {
            // Copy a contiguous chunk out of the ring, then only drop what the hardware took.
            let mut chunk = [0u8; FIFO_BYTES];
            let n = self.queue.len().min(FIFO_BYTES);
            for (slot, byte) in chunk[..n].iter_mut().zip(self.queue.iter()) {
                *slot = *byte;
            }
            let taken = self.inner.write(&chunk[..n])?;
            for _ in 0..taken {
                let _ = self.queue.pop_front();
            }
            written += taken;
            if taken < n {
                break; // hardware is full for now
            }
        }
        Ok(written)
    }
}

impl<T: Transport> Transport for TxBuffered<T> {
    type Error = T::Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.inner.read(buf)
    }

    /// Accepts the whole `buf` or nothing. Callers write one frame at a time, so a full queue
    /// drops whole frames: half a frame on the wire would corrupt the next one too (it has no
    /// delimiter), which is how a `HelloAck` used to get lost after the host left the port unread.
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if self.queue.capacity() - self.queue.len() < buf.len() {
            let _ = self.pump()?;
            if self.queue.capacity() - self.queue.len() < buf.len() {
                return Ok(0);
            }
        }
        for &byte in buf {
            let _ = self.queue.push_back(byte);
        }
        // Opportunistically start draining so a steady stream never relies on the next tick alone.
        let _ = self.pump()?;
        Ok(buf.len())
    }

    fn drain_pending(&mut self) -> Result<usize, Self::Error> {
        self.pump()
    }

    fn discard_unsent(&mut self) {
        if !self.queue.is_empty() {
            // The head of the queue may be the rest of a frame already partly in the hardware
            // FIFO: one delimiter closes it, and the peer drops it as a bad frame.
            self.queue.clear();
            let _ = self.queue.push_back(0);
        }
    }
}
