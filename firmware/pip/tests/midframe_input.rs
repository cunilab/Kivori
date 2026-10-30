//! Issue #18: input keeps flowing while a long frame is transferred. Between display windows the
//! runtime drains the input source and sends pending `InputEvent`s, so a detent waits for at most
//! one window, not the whole frame. Deterministic: a virtual clock and a scripted input double.
#![cfg(feature = "host-sim")]

use heapless::Vec as HVec;
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::ports::{DisplaySink, InputSource};
use kivori_firmware::proto::DeviceIdentity;
use kivori_firmware::render::FRAME_PIXELS;
use kivori_firmware::runtime::{Runtime, RuntimeConfig};
use kivori_firmware::sim::{CaptureDisplay, SimPipe, VirtualClock};
use kivori_model::input::{Direction, InputLevels};
use kivori_model::{Capabilities, ElapsedMs, ProtocolVersion, Rect, Rgb565};
use kivori_protocol::{
    decode_message, encode_message, FirmwareVersion, Hello, InputKind, Message, Ready, MAX_FRAME,
    MAX_WIRE, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::rc::Rc;

const NONCE: u32 = 0x0A11_CE01;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Step {
    Window,
    DetentDelivered,
}

type Log = Rc<RefCell<Vec<Step>>>;

/// Counts windows and lets the input double know the first one has been written.
struct LoggingDisplay {
    inner: Box<CaptureDisplay>,
    log: Log,
    first_window_done: Rc<Cell<bool>>,
}

impl DisplaySink for LoggingDisplay {
    type Error = Infallible;
    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        self.log.borrow_mut().push(Step::Window);
        self.first_window_done.set(true);
        self.inner.blit_tile(rect, pixels)
    }
    fn blit_tiles(&mut self, rect: Rect, tiles: &[Rgb565], cols: u16) -> Result<(), Self::Error> {
        self.log.borrow_mut().push(Step::Window);
        self.first_window_done.set(true);
        self.inner.blit_tiles(rect, tiles, cols)
    }
}

/// A knob at rest until the first display window is out, then one clockwise detent.
struct DetentAfterFirstWindow {
    armed: Rc<Cell<bool>>,
    delivered: bool,
    log: Log,
}

fn lv(a: bool, b: bool) -> InputLevels {
    InputLevels {
        a,
        b,
        sw: false,
        keys: [false; 3],
    }
}

impl InputSource for DetentAfterFirstWindow {
    fn sample(&mut self) -> InputLevels {
        lv(false, false)
    }
    fn drain(&mut self, now_ms: ElapsedMs, f: &mut dyn FnMut(InputLevels, ElapsedMs)) {
        if self.armed.get() && !self.delivered {
            self.delivered = true;
            self.log.borrow_mut().push(Step::DetentDelivered);
            // The Gray cycle 00 -> 01 -> 11 -> 10 -> 00 is one clockwise detent.
            for (a, b) in [(false, true), (true, true), (true, false)] {
                f(lv(a, b), now_ms);
            }
        }
        f(lv(false, false), now_ms);
    }
}

fn host_messages(pipe: &mut SimPipe) -> Vec<Message> {
    let bytes = pipe.host_recv();
    let mut scratch: HVec<u8, MAX_FRAME> = HVec::new();
    bytes
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            decode_message(p, &mut scratch, &[PROTOCOL_MAJOR])
                .ok()
                .map(|(_, m)| m)
        })
        .collect()
}

#[test]
fn a_detent_during_a_long_frame_is_sent_before_the_frame_completes() {
    let caps = Capabilities::PHYSICAL_INPUT_V1.union(Capabilities::PRESENTATION_V1);
    let identity = DeviceIdentity {
        device_id: [0x1A; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: caps,
    };
    let frame_buffer: &'static mut [Rgb565; FRAME_PIXELS] = Box::leak(
        vec![Rgb565::from_raw(0); FRAME_PIXELS]
            .into_boxed_slice()
            .try_into()
            .unwrap(),
    );
    let mut runtime = Runtime::with_frame_buffer(identity, RuntimeConfig::default(), frame_buffer);
    let clock = VirtualClock::new();
    let mut pipe = SimPipe::new();
    let log: Log = Rc::default();
    let first_window_done = Rc::new(Cell::new(false));
    let mut display = LoggingDisplay {
        inner: Box::new(CaptureDisplay::new()),
        log: Rc::clone(&log),
        first_window_done: Rc::clone(&first_window_done),
    };
    let mut input = DetentAfterFirstWindow {
        armed: first_window_done,
        delivered: false,
        log: Rc::clone(&log),
    };
    let blob_bytes = compile_default_blob();
    let blob = AssetBlob::parse(&blob_bytes).unwrap();

    // Handshake queued ahead of the first tick: the very first frame is a full-screen flush with
    // input already negotiated.
    let version = ProtocolVersion::new(PROTOCOL_MAJOR, PROTOCOL_MINOR);
    for (seq, msg) in [
        Message::Hello(Hello {
            desktop_version: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            desktop_caps: caps,
            nonce: NONCE,
        }),
        Message::Ready(Ready {
            negotiated_minor: 0,
            negotiated_caps: caps,
        }),
    ]
    .iter()
    .enumerate()
    {
        let mut wire: HVec<u8, MAX_WIRE> = HVec::new();
        encode_message(msg, version, seq as u16, &mut wire).unwrap();
        pipe.host_send(&wire).unwrap();
    }

    let tick = runtime.step(&clock, &mut pipe, &mut input, &mut display, &blob);
    assert!(tick.frame_rendered);
    assert_eq!(tick.tiles_flushed, 36, "a full-screen frame");
    assert_eq!(tick.windows_flushed, 3, "in three band windows");

    // The detent was picked up between windows 1 and 2, not after the frame.
    assert_eq!(
        *log.borrow(),
        [
            Step::Window,
            Step::DetentDelivered,
            Step::Window,
            Step::Window
        ]
    );
    let sent: Vec<InputKind> = host_messages(&mut pipe)
        .into_iter()
        .filter_map(|m| match m {
            Message::InputEvent(e) => Some(e.kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        sent,
        [InputKind::GestureStarted, InputKind::Detent(Direction::Cw)],
        "the detent left the device during the tick that was drawing the frame"
    );
}
