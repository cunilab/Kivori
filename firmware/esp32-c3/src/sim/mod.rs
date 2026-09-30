//! Host simulation adapters (`host-sim` feature): an in-memory transport, a capture display, and a
//! virtual clock. Together they let the entire device core run and be asserted on the host with no
//! hardware (FR-035).

use crate::input::gesture::{RotaryEvent, RotaryGesture};
use crate::input::quadrature::QuadratureDecoder;
use crate::ports::{Clock, DisplaySink, InputSource, Transport};
use crate::proto::{DeviceIdentity, Dispatcher};
use crate::runtime::GESTURE_END_MS;
use crate::state::DeviceState;
use core::cell::Cell;
use core::convert::Infallible;
use heapless::Vec;
use kivori_model::input::{Direction, InputLevels};
use kivori_model::{Capabilities, ElapsedMs, ProtocolVersion, Rect, Rgb565};
use kivori_protocol::{
    encode_message, FirmwareVersion, Hello, Message, Nonce, Ready, MAX_WIRE, PROTOCOL_MAJOR,
    PROTOCOL_MINOR,
};

/// Byte capacity of each direction of the simulated pipe.
pub const PIPE_CAPACITY: usize = 8192;
/// Simulated panel width.
pub const FRAME_W: usize = 240;
/// Simulated panel height.
pub const FRAME_H: usize = 240;
/// Simulated panel pixel count.
pub const FRAME_PIXELS: usize = FRAME_W * FRAME_H;
/// Capacity of a [`ScriptedInput`] level script and of the [`drive_rotary`] event log.
///
/// The crate is `#![no_std]` and the sim module must stay buildable for the device target — the
/// positive control in `scripts/check-release-surface.sh` compiles `--features host-sim` for
/// `riscv32imc` to prove these adapters *would* be visible if they leaked — so these helpers use a
/// fixed-capacity [`heapless::Vec`] rather than `std::vec::Vec`. The longest scenario in the
/// firmware test suite is a two-detent reversal: 9 levels producing 4 events. 32 leaves ample
/// headroom; overflowing it panics loudly rather than silently truncating a scenario.
pub const SCRIPT_CAPACITY: usize = 32;

/// A bidirectional in-memory byte pipe. The device sees it as a [`Transport`]; the test plays the
/// host role via [`SimPipe::host_send`] / [`SimPipe::host_recv`].
pub struct SimPipe {
    to_device: Vec<u8, PIPE_CAPACITY>,
    to_host: Vec<u8, PIPE_CAPACITY>,
}

impl SimPipe {
    /// An empty pipe.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            to_device: Vec::new(),
            to_host: Vec::new(),
        }
    }

    /// Host → device: queues `bytes` for the device to read. `Err` if the pipe is full.
    // Mirrors heapless `extend_from_slice`'s own `Result<(), ()>`; a sim helper needs no richer error.
    #[allow(clippy::result_unit_err)]
    pub fn host_send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.to_device.extend_from_slice(bytes)
    }

    /// Device → host: drains and returns everything the device has written so far.
    #[must_use]
    pub fn host_recv(&mut self) -> Vec<u8, PIPE_CAPACITY> {
        let out = self.to_host.clone();
        self.to_host.clear();
        out
    }

    /// Whether the device has produced any unread output.
    #[must_use]
    pub fn host_has_output(&self) -> bool {
        !self.to_host.is_empty()
    }
}

impl Default for SimPipe {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for SimPipe {
    type Error = Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let n = buf.len().min(self.to_device.len());
        buf[..n].copy_from_slice(&self.to_device[..n]);
        let mut rest: Vec<u8, PIPE_CAPACITY> = Vec::new();
        let _ = rest.extend_from_slice(&self.to_device[n..]);
        self.to_device = rest;
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let space = PIPE_CAPACITY - self.to_host.len();
        let n = space.min(buf.len());
        let _ = self.to_host.extend_from_slice(&buf[..n]);
        Ok(n)
    }
}

/// A [`DisplaySink`] that composites blitted tiles into a full-frame buffer and counts flushes.
pub struct CaptureDisplay {
    frame: [Rgb565; FRAME_PIXELS],
    /// Number of tile blits received (change-driven rendering flushes only changed tiles).
    pub blits: u32,
}

impl CaptureDisplay {
    /// A capture display cleared to black.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            frame: [Rgb565::from_raw(0); FRAME_PIXELS],
            blits: 0,
        }
    }

    /// The full composited frame (row-major, `FRAME_W * FRAME_H`).
    #[must_use]
    pub fn frame(&self) -> &[Rgb565] {
        &self.frame
    }

    /// The pixel at `(x, y)`.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Rgb565 {
        self.frame[y * FRAME_W + x]
    }
}

impl Default for CaptureDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplaySink for CaptureDisplay {
    type Error = Infallible;

    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        let (x0, y0) = (rect.x as usize, rect.y as usize);
        let (w, h) = (rect.w as usize, rect.h as usize);
        for row in 0..h {
            for col in 0..w {
                let (dx, dy) = (x0 + col, y0 + row);
                if dx < FRAME_W && dy < FRAME_H {
                    self.frame[dy * FRAME_W + dx] = pixels[row * w + col];
                }
            }
        }
        self.blits += 1;
        Ok(())
    }
}

/// A settable, monotonic virtual [`Clock`].
pub struct VirtualClock {
    now: Cell<ElapsedMs>,
}

impl VirtualClock {
    /// A clock at `0` ms.
    #[must_use]
    pub const fn new() -> Self {
        Self { now: Cell::new(0) }
    }

    /// Sets the current time.
    pub fn set(&self, ms: ElapsedMs) {
        self.now.set(ms);
    }

    /// Advances the clock by `delta` ms (saturating).
    pub fn advance(&self, delta: ElapsedMs) {
        self.now.set(self.now.get().saturating_add(delta));
    }
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for VirtualClock {
    fn now_ms(&self) -> ElapsedMs {
        self.now.get()
    }
}

/// Replays a fixed level sequence, then holds the final level forever.
pub struct ScriptedInput {
    steps: Vec<InputLevels, SCRIPT_CAPACITY>,
    index: usize,
}

impl ScriptedInput {
    /// Creates a scripted source that replays `steps` in order, then holds the last level.
    ///
    /// # Panics
    /// Panics if `steps` is empty — a scripted source with nothing to replay is a test bug.
    #[must_use]
    pub fn new(steps: Vec<InputLevels, SCRIPT_CAPACITY>) -> Self {
        assert!(!steps.is_empty(), "ScriptedInput needs at least one level");
        Self { steps, index: 0 }
    }
}

impl InputSource for ScriptedInput {
    fn sample(&mut self) -> InputLevels {
        let level = self.steps[self.index];
        if self.index + 1 < self.steps.len() {
            self.index += 1;
        }
        level
    }
}

/// Test-only mirror of the emitted stream, in the order the runtime produced it.
#[derive(Debug, PartialEq, Eq)]
pub enum SeenInput {
    /// A new gesture opened.
    GestureStarted {
        /// Session-unique identifier of the gesture that just opened.
        gesture_id: u16,
    },
    /// One validated detent belonging to a gesture.
    Detent {
        /// Identifier of the gesture this detent belongs to.
        gesture_id: u16,
        /// Rotational direction of the detent.
        direction: Direction,
    },
    /// A gesture closed.
    GestureEnded {
        /// Identifier of the gesture that just closed.
        gesture_id: u16,
    },
}

/// Drive decoder + gesture over a level script and collect the emitted events.
///
/// Each level consumes 1 ms; the clock then advances past the gesture window so a
/// trailing `GestureEnded` is produced deterministically. The window is the production
/// [`GESTURE_END_MS`], not a local copy, so retuning the runtime retunes this helper with it.
///
/// # Panics
/// Panics if more than [`SCRIPT_CAPACITY`] events are produced — a scenario that outgrew the
/// fixed capacity is a test bug, and truncating it silently would make the assertion vacuous.
pub fn drive_rotary(levels: Vec<InputLevels, SCRIPT_CAPACITY>) -> Vec<SeenInput, SCRIPT_CAPACITY> {
    let mut decoder = QuadratureDecoder::new();
    let mut gesture = RotaryGesture::new(GESTURE_END_MS);
    let mut src = ScriptedInput::new(levels.clone());
    let mut out = Vec::new();

    for tick in 0..levels.len() as u32 {
        let l = src.sample();
        if let Some(direction) = decoder.update(l.a, l.b) {
            let (started, detent) = gesture.on_detent(direction, tick);
            if let Some(RotaryEvent::GestureStarted { gesture_id }) = started {
                out.push(SeenInput::GestureStarted { gesture_id })
                    .expect("SCRIPT_CAPACITY");
            }
            if let RotaryEvent::Detent {
                gesture_id,
                direction,
            } = detent
            {
                out.push(SeenInput::Detent {
                    gesture_id,
                    direction,
                })
                .expect("SCRIPT_CAPACITY");
            }
        }
    }

    let end_at = levels.len() as u32 + GESTURE_END_MS;
    if let Some(RotaryEvent::GestureEnded { gesture_id }) = gesture.poll(end_at) {
        out.push(SeenInput::GestureEnded { gesture_id })
            .expect("SCRIPT_CAPACITY");
    }
    out
}

/// Drives Hello -> HelloAck -> Ready over a fresh in-memory pipe with the given session `nonce`,
/// mirroring the same encode/[`SimPipe`]/[`Dispatcher::poll`] pattern
/// `firmware/esp32-c3/tests/host_sim.rs` already exercises, and returns the resulting
/// [`Dispatcher`] with the session accepted and both Slice 002 capabilities negotiated.
///
/// A test fixture, not a second handshake implementation: `tests/` integration files are separate
/// crates and cannot import a helper from another `tests/*.rs` file, so this reproduces the same
/// low-level pattern (`encode_message` + [`SimPipe::host_send`] + [`Dispatcher::poll`]) inside the
/// library instead of inventing a different one.
#[must_use]
pub fn handshaken_dispatcher(nonce: Nonce) -> Dispatcher {
    let identity = DeviceIdentity {
        device_id: [0xEE; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: Capabilities::MASCOT_INTERACTION
            .union(Capabilities::PHYSICAL_INPUT_V1)
            .union(Capabilities::PRESENTATION_V1),
    };
    let mut dispatcher = Dispatcher::new(identity);
    let mut pipe = SimPipe::new();
    let mut device = DeviceState::new();
    let version = ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR);

    let mut hello_wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(
        &Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: identity.capabilities,
            nonce,
        }),
        version,
        0,
        &mut hello_wire,
    )
    .expect("encode Hello");
    pipe.host_send(&hello_wire).expect("pipe capacity");
    dispatcher
        .poll(&mut pipe, &mut device, 0)
        .expect("poll Hello");
    let _ = pipe.host_recv(); // discard HelloAck bytes; the caller wants the Dispatcher, not the wire

    let mut ready_wire: Vec<u8, MAX_WIRE> = Vec::new();
    encode_message(
        &Message::Ready(Ready {
            negotiated_minor: PROTOCOL_MINOR,
            negotiated_caps: identity.capabilities,
        }),
        version,
        1,
        &mut ready_wire,
    )
    .expect("encode Ready");
    pipe.host_send(&ready_wire).expect("pipe capacity");
    dispatcher
        .poll(&mut pipe, &mut device, 0)
        .expect("poll Ready");

    dispatcher
}
