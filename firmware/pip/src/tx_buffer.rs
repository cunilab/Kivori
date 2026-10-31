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

/// Inbound buffer filled by [`Transport::buffer_inbound`] while a frame is composed: the host's
/// connect burst (about 400 bytes) and a few heartbeats fit.
pub const RX_BUFFER_BYTES: usize = 1024;

/// A [`Transport`] wrapper giving the dispatcher whole-frame writes over a bounded hardware FIFO.
///
/// `write` enqueues (so a frame is never truncated by a full FIFO), `read` delegates straight through, and
/// [`Self::pump`] moves queued bytes toward the hardware without blocking. The run loop pumps every tick.
pub struct TxBuffered<T> {
    inner: T,
    queue: Deque<u8, TX_QUEUE_BYTES>,
    inbound: Deque<u8, RX_BUFFER_BYTES>,
}

impl<T: Transport> TxBuffered<T> {
    /// Wraps `inner` with an empty outbound queue.
    #[must_use]
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            queue: Deque::new(),
            inbound: Deque::new(),
        }
    }

    /// Bytes still waiting to reach the hardware.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Borrows the wrapped transport (for adapter-specific calls on the inner transport).
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
        // Bytes buffered while a frame was composed come first, in arrival order.
        let mut n = 0;
        while n < buf.len() {
            match self.inbound.pop_front() {
                Some(byte) => {
                    buf[n] = byte;
                    n += 1;
                }
                None => break,
            }
        }
        if n < buf.len() {
            n += self.inner.read(&mut buf[n..])?;
        }
        Ok(n)
    }

    fn buffer_inbound(&mut self) -> Result<(), Self::Error> {
        let mut chunk = [0u8; FIFO_BYTES];
        loop {
            let room = (RX_BUFFER_BYTES - self.inbound.len()).min(FIFO_BYTES);
            if room == 0 {
                return Ok(());
            }
            let n = self.inner.read(&mut chunk[..room])?;
            for &byte in &chunk[..n] {
                let _ = self.inbound.push_back(byte);
            }
            if n < room {
                return Ok(());
            }
        }
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

    fn tx_free(&mut self) -> usize {
        // A failed pump resurfaces on the next write; here only the room matters.
        let _ = self.pump();
        self.queue.capacity() - self.queue.len()
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
