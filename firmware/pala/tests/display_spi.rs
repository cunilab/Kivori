//! Host tests for the T072 SPI `DisplaySink` adapter.
//!
//! The adapter speaks only `embedded-hal`, so the *real* `mipidsi` driver can be driven here against a
//! recording `SpiDevice`. That makes the SPI byte stream itself the assertion target: reset sequencing,
//! command-vs-data phases, address-window coordinates, panel offsets, RGB565 byte order, exact transfer
//! volume, and change-driven refresh — all deterministic, all without a simulator or a board.
//!
//! # Scope
//!
//! These are **generic** MIPI-DCS properties (scopes A and B of the display plan). The model used below
//! is a stand-in chosen only because it accepts a 240x240 window; nothing here validates the Kivori
//! panel's controller, its initialisation sequence, its real offsets, colour order, or orientation. The
//! physical controller is unconfirmed and remains a hardware task.
#![cfg(feature = "host-sim")]

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as DigitalErrorType, OutputPin};
use embedded_hal::spi::{ErrorType as SpiErrorType, Operation, SpiDevice};
use kivori_asset_compiler::compile_default_blob;
use kivori_assets::AssetBlob;
use kivori_firmware::display::{init_panel, DisplayError, MipidsiSink, PanelGeometry};
use kivori_firmware::ports::DisplaySink;
use kivori_firmware::render::{TileRenderer, TILE_COUNT, TILE_H, TILE_PIXELS, TILE_W};
use kivori_model::{CompanionState, MascotAnimator, Rect, Rgb565};
use mipidsi::models::ILI9341Rgb565;
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;

// ── DCS opcodes the adapter is expected to emit (verified against mipidsi 0.10 source) ───────────────
const SET_COLUMN_ADDRESS: u8 = 0x2A;
const SET_PAGE_ADDRESS: u8 = 0x2B;
const WRITE_MEMORY_START: u8 = 0x2C;

/// One observable bus event, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    /// Reset pin driven to a level.
    Reset(bool),
    /// D/C pin driven to a level (`false` = command phase, `true` = data phase).
    Dc(bool),
    /// Bytes clocked out while D/C held the level of the preceding [`Event::Dc`].
    Write(Vec<u8>),
}

type Log = Rc<RefCell<Vec<Event>>>;

/// A `SpiDevice` that records every transfer instead of driving a bus.
struct RecordingSpi {
    log: Log,
}

impl SpiErrorType for RecordingSpi {
    type Error = Infallible;
}

impl SpiDevice<u8> for RecordingSpi {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        for op in operations {
            if let Operation::Write(bytes) = op {
                self.log.borrow_mut().push(Event::Write(bytes.to_vec()));
            }
        }
        Ok(())
    }
}

/// A `SpiDevice` whose writes always fail, to prove the adapter surfaces the error.
struct FailingSpi;

#[derive(Debug)]
struct BusDown;

impl embedded_hal::spi::Error for BusDown {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        embedded_hal::spi::ErrorKind::Other
    }
}

impl SpiErrorType for FailingSpi {
    type Error = BusDown;
}

impl SpiDevice<u8> for FailingSpi {
    fn transaction(&mut self, _operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        Err(BusDown)
    }
}

/// A recording `OutputPin`, tagged so D/C and reset are distinguishable in one log.
struct RecordingPin {
    log: Log,
    is_reset: bool,
}

impl DigitalErrorType for RecordingPin {
    type Error = Infallible;
}

impl OutputPin for RecordingPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.push(false);
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.push(true);
        Ok(())
    }
}

impl RecordingPin {
    fn push(&mut self, level: bool) {
        let event = if self.is_reset {
            Event::Reset(level)
        } else {
            Event::Dc(level)
        };
        self.log.borrow_mut().push(event);
    }
}

/// A delay that does nothing (host tests must not sleep).
struct NoDelay;

impl DelayNs for NoDelay {
    fn delay_ns(&mut self, _ns: u32) {}
}

/// A reconstructed DCS command: opcode plus the argument bytes that followed it in the data phase.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Command {
    code: u8,
    args: Vec<u8>,
}

/// Splits a raw event log into commands and their data payloads.
///
/// This *is* the "command and data phases are distinguishable" assertion: the reconstruction only works
/// because every opcode is clocked with D/C low and every argument/pixel byte with D/C high.
fn commands(events: &[Event]) -> Vec<Command> {
    let mut out: Vec<Command> = Vec::new();
    let mut dc_low = false;
    for event in events {
        match event {
            Event::Dc(level) => dc_low = !*level,
            Event::Write(bytes) if bytes.is_empty() => {}
            Event::Write(bytes) if dc_low => {
                // A command phase carries exactly one opcode byte.
                assert_eq!(bytes.len(), 1, "command phase must clock a single opcode");
                out.push(Command {
                    code: bytes[0],
                    args: Vec::new(),
                });
            }
            Event::Write(bytes) => {
                if let Some(last) = out.last_mut() {
                    last.args.extend_from_slice(bytes);
                }
            }
            Event::Reset(_) => {}
        }
    }
    out
}

/// Total bytes written in data phases that follow `opcode`.
fn payload_len(cmds: &[Command], opcode: u8) -> usize {
    cmds.iter()
        .filter(|c| c.code == opcode)
        .map(|c| c.args.len())
        .sum()
}

type Sink = MipidsiSink<
    mipidsi::interface::SpiInterface<'static, RecordingSpi, RecordingPin>,
    ILI9341Rgb565,
    RecordingPin,
>;

/// Builds an initialised sink over a recording bus, plus the shared log.
fn harness(geometry: PanelGeometry) -> (Sink, Log) {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let spi = RecordingSpi {
        log: Rc::clone(&log),
    };
    let dc = RecordingPin {
        log: Rc::clone(&log),
        is_reset: false,
    };
    let reset = RecordingPin {
        log: Rc::clone(&log),
        is_reset: true,
    };
    // Leaked so the interface can hold a 'static buffer, as it would from a `static_cell` on device.
    let buffer: &'static mut [u8] = Box::leak(Box::new([0u8; 512]));
    let interface = mipidsi::interface::SpiInterface::new(spi, dc, buffer);
    let sink =
        init_panel(ILI9341Rgb565, interface, reset, &mut NoDelay, geometry).expect("panel init");
    (sink, log)
}

fn full_panel() -> PanelGeometry {
    PanelGeometry::new(240, 240, 0, 0)
}

fn solid(color: u16) -> Vec<Rgb565> {
    vec![Rgb565::from_raw(color); TILE_PIXELS]
}

#[test]
fn init_drives_a_hardware_reset_before_any_command() {
    let (_sink, log) = harness(full_panel());
    let events = log.borrow();

    let first_reset_low = events.iter().position(|e| *e == Event::Reset(false));
    let first_reset_high = events.iter().position(|e| *e == Event::Reset(true));
    let first_write = events
        .iter()
        .position(|e| matches!(e, Event::Write(b) if !b.is_empty()));

    let (low, high, write) = (
        first_reset_low.expect("reset asserted low"),
        first_reset_high.expect("reset released high"),
        first_write.expect("at least one command issued during init"),
    );
    assert!(low < high, "reset must go low before it goes high");
    assert!(
        high < write,
        "reset must complete before the first command is clocked"
    );
    assert!(
        !commands(&events).is_empty(),
        "init must issue DCS commands, otherwise the probe proves nothing"
    );
}

#[test]
fn a_tile_blit_is_one_window_pair_plus_exact_pixel_bytes() {
    let (mut sink, log) = harness(full_panel());
    log.borrow_mut().clear(); // ignore init traffic

    let rect = Rect::new(0, TILE_H, TILE_W, TILE_H); // second band
    sink.blit_tile(rect, &solid(0x1234)).expect("blit");

    let events = log.borrow();
    let cmds = commands(&events);
    let codes: Vec<u8> = cmds.iter().map(|c| c.code).collect();
    assert_eq!(
        codes,
        vec![SET_COLUMN_ADDRESS, SET_PAGE_ADDRESS, WRITE_MEMORY_START],
        "a windowed tile write is exactly CASET, RASET, RAMWR"
    );
    // Inclusive end coordinates, big-endian u16 pairs.
    assert_eq!(cmds[0].args, vec![0x00, 0x00, 0x00, 0x27], "columns 0..=39");
    assert_eq!(cmds[1].args, vec![0x00, 0x28, 0x00, 0x4F], "rows 40..=79");
    assert_eq!(
        payload_len(&cmds, WRITE_MEMORY_START),
        TILE_PIXELS * 2,
        "exactly two bytes per RGB565 pixel, no padding and no wrap"
    );
}

#[test]
fn panel_offset_shifts_the_address_window() {
    // A 240x240 visible area at offset (0, 40) inside the model's 240x320 framebuffer.
    let (mut sink, log) = harness(PanelGeometry::new(240, 240, 0, 40));
    log.borrow_mut().clear();

    sink.blit_tile(Rect::new(0, 0, TILE_W, TILE_H), &solid(0))
        .expect("blit");

    let events = log.borrow();
    let cmds = commands(&events);
    assert_eq!(
        cmds[1].args,
        vec![0x00, 0x28, 0x00, 0x4F],
        "rows 0..=39 must land at 40..=79 once the panel offset is applied"
    );
}

#[test]
fn rgb565_pixels_go_out_big_endian() {
    let (mut sink, log) = harness(full_panel());
    log.borrow_mut().clear();

    // Pure red is 0xF800 in RGB565.
    sink.blit_tile(Rect::new(0, 0, TILE_W, TILE_H), &solid(0xF800))
        .expect("blit");

    let events = log.borrow();
    let cmds = commands(&events);
    let data: &Vec<u8> = &cmds
        .iter()
        .find(|c| c.code == WRITE_MEMORY_START)
        .expect("RAMWR")
        .args;
    assert_eq!(data.len(), TILE_PIXELS * 2);
    assert!(
        data.as_chunks::<2>().0.iter().all(|p| *p == [0xF8, 0x00]),
        "every pixel must be the high byte first"
    );
}

#[test]
fn a_wrong_pixel_count_is_rejected_before_the_bus_is_touched() {
    let (mut sink, log) = harness(full_panel());
    log.borrow_mut().clear();

    let short = vec![Rgb565::from_raw(0); TILE_PIXELS - 1];
    assert!(matches!(
        sink.blit_tile(Rect::new(0, 0, TILE_W, TILE_H), &short),
        Err(DisplayError::PixelCountMismatch)
    ));
    assert!(
        log.borrow().is_empty(),
        "a rejected tile must not clock a partial window (set_pixels would wrap)"
    );
}

#[test]
fn out_of_bounds_and_empty_tiles_are_rejected() {
    let (mut sink, log) = harness(full_panel());
    log.borrow_mut().clear();

    // Past the bottom edge.
    let past = Rect::new(0, 220, TILE_W, TILE_H);
    assert!(matches!(
        sink.blit_tile(past, &solid(0)),
        Err(DisplayError::TileOutOfBounds)
    ));
    // Degenerate rectangle: `set_pixels` has no bounds checking, so an empty window must never be sent.
    assert!(matches!(
        sink.blit_tile(Rect::new(0, 0, 0, 0), &[]),
        Err(DisplayError::TileOutOfBounds)
    ));
    assert!(log.borrow().is_empty(), "no bus traffic for rejected tiles");
}

#[test]
fn a_failed_spi_write_is_surfaced_not_swallowed() {
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let dc = RecordingPin {
        log: Rc::clone(&log),
        is_reset: false,
    };
    let reset = RecordingPin {
        log: Rc::clone(&log),
        is_reset: true,
    };
    let buffer: &'static mut [u8] = Box::leak(Box::new([0u8; 512]));
    let interface = mipidsi::interface::SpiInterface::new(FailingSpi, dc, buffer);
    // Init itself must fail on a dead bus rather than pretend the panel is ready.
    let result = init_panel(ILI9341Rgb565, interface, reset, &mut NoDelay, full_panel());
    assert!(
        matches!(result, Err(mipidsi::InitError::Interface(_))),
        "a dead SPI bus must fail init"
    );
}

// ── Scope B: RGB565 tile-stream properties through the canonical renderer ────────────────────────────

#[test]
fn a_full_frame_uses_small_update_windows_without_changing_its_payload() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).expect("valid blob");
    let (mut sink, log) = harness(full_panel());
    let mut renderer = TileRenderer::new();
    log.borrow_mut().clear();

    renderer
        .render(&blob, CompanionState::Idle, 0, &mut sink)
        .expect("first frame");

    let events = log.borrow();
    let cmds = commands(&events);
    assert_eq!(
        cmds.iter().filter(|c| c.code == WRITE_MEMORY_START).count(),
        TILE_COUNT,
        "small windows cover the 240x240 panel"
    );
    assert_eq!(
        cmds.iter().filter(|c| c.code == SET_COLUMN_ADDRESS).count(),
        TILE_COUNT,
        "every tile sets its own window"
    );
    assert_eq!(TILE_PIXELS, 1600);
    assert_eq!(
        payload_len(&cmds, WRITE_MEMORY_START),
        240 * 240 * 2,
        "115200 bytes for a full RGB565 frame"
    );
}

#[test]
fn an_idle_blink_transfers_less_than_one_third_of_a_frame() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).expect("valid blob");
    let (mut sink, log) = harness(full_panel());
    let mut storage: Box<[Rgb565; kivori_firmware::render::FRAME_PIXELS]> =
        vec![Rgb565::from_raw(0); kivori_firmware::render::FRAME_PIXELS]
            .into_boxed_slice()
            .try_into()
            .unwrap();
    let mut renderer = TileRenderer::with_frame_buffer(&mut storage);
    let animator = MascotAnimator::new(CompanionState::Idle, 0);

    renderer
        .render_animation(&blob, CompanionState::Idle, &animator.pose_at(0), &mut sink)
        .expect("initial frame");
    log.borrow_mut().clear();

    renderer
        .render_animation(
            &blob,
            CompanionState::Idle,
            &animator.pose_at(2_911),
            &mut sink,
        )
        .expect("blink frame");

    let commands = commands(&log.borrow());
    let bytes = payload_len(&commands, WRITE_MEMORY_START);
    assert!(bytes > 0, "the blink must reach the panel");
    assert!(
        bytes < (240 * 240 * 2) / 3,
        "a local facial change sent {bytes} bytes"
    );
}

#[test]
fn an_unchanged_frame_retransmits_nothing_and_a_changed_one_does() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).expect("valid blob");
    let (mut sink, log) = harness(full_panel());
    let mut renderer = TileRenderer::new();

    renderer
        .render(&blob, CompanionState::Idle, 0, &mut sink)
        .expect("first frame");
    log.borrow_mut().clear();

    renderer
        .render(&blob, CompanionState::Idle, 0, &mut sink)
        .expect("identical frame");
    assert!(
        commands(&log.borrow()).is_empty(),
        "an identical frame must not touch the bus at all"
    );

    renderer
        .render(&blob, CompanionState::Happy, 0, &mut sink)
        .expect("changed frame");
    let events = log.borrow();
    let cmds = commands(&events);
    assert!(
        cmds.iter().any(|c| c.code == WRITE_MEMORY_START),
        "a changed state must retransmit at least one tile"
    );
    assert!(
        payload_len(&cmds, WRITE_MEMORY_START).is_multiple_of(TILE_PIXELS * 2),
        "retransmissions are whole tiles"
    );
}

/// Replays the SPI stream a panel would see into a 240x240 image: CASET/RASET set the window,
/// RAMWR pixels fill it row-major, wrapping inside the window.
fn replay_frame(cmds: &[Command]) -> Vec<Rgb565> {
    let mut frame = vec![Rgb565::from_raw(0); 240 * 240];
    let (mut x0, mut x1, mut y0, mut y1) = (0usize, 239usize, 0usize, 239usize);
    for cmd in cmds {
        let pair = |i: usize| {
            (
                usize::from(u16::from_be_bytes([cmd.args[i], cmd.args[i + 1]])),
                usize::from(u16::from_be_bytes([cmd.args[i + 2], cmd.args[i + 3]])),
            )
        };
        match cmd.code {
            SET_COLUMN_ADDRESS => (x0, x1) = pair(0),
            SET_PAGE_ADDRESS => (y0, y1) = pair(0),
            WRITE_MEMORY_START => {
                let width = x1 - x0 + 1;
                for (i, px) in cmd.args.as_chunks::<2>().0.iter().enumerate() {
                    let (x, y) = (x0 + i % width, y0 + i / width);
                    assert!(y <= y1, "pixels overran the address window");
                    frame[y * 240 + x] = Rgb565::from_raw(u16::from_be_bytes(*px));
                }
            }
            _ => {}
        }
    }
    frame
}

#[test]
fn a_buffered_full_frame_streams_three_band_windows_in_small_spi_writes() {
    let bytes = compile_default_blob();
    let blob = AssetBlob::parse(&bytes).expect("valid blob");
    let pose = MascotAnimator::new(CompanionState::Happy, 0).pose_at(200);

    // Reference: the per-tile path through the same real driver.
    let (mut tile_sink, tile_log) = harness(full_panel());
    tile_log.borrow_mut().clear();
    TileRenderer::new()
        .render_animation(&blob, CompanionState::Happy, &pose, &mut tile_sink)
        .expect("per-tile frame");
    let tile_cmds = commands(&tile_log.borrow());

    let (mut sink, log) = harness(full_panel());
    let mut storage: Box<[Rgb565; kivori_firmware::render::FRAME_PIXELS]> =
        vec![Rgb565::from_raw(0); kivori_firmware::render::FRAME_PIXELS]
            .into_boxed_slice()
            .try_into()
            .unwrap();
    log.borrow_mut().clear();
    TileRenderer::with_frame_buffer(&mut storage)
        .render_animation(&blob, CompanionState::Happy, &pose, &mut sink)
        .expect("buffered frame");
    let events = log.borrow();
    let cmds = commands(&events);

    let windows = |c: &[Command]| c.iter().filter(|c| c.code == SET_COLUMN_ADDRESS).count();
    assert_eq!(windows(&tile_cmds), TILE_COUNT, "before: a window per tile");
    assert_eq!(windows(&cmds), 3, "after: three two-row bands");
    assert_eq!(
        payload_len(&cmds, WRITE_MEMORY_START),
        payload_len(&tile_cmds, WRITE_MEMORY_START),
        "same transfer volume"
    );
    assert_eq!(
        replay_frame(&cmds),
        replay_frame(&tile_cmds),
        "the panel ends up with byte-identical pixels"
    );

    // A window is only an address range: the interface still streams it in buffer-sized writes
    // (512 bytes in this harness, 4 KiB on the board), never one write per window.
    let writes: Vec<usize> = events
        .iter()
        .filter_map(|e| match e {
            Event::Write(b) if b.len() > 1 => Some(b.len()),
            _ => None,
        })
        .collect();
    assert!(writes.iter().all(|&len| len <= 512), "{writes:?}");
    assert!(
        writes.iter().filter(|&&len| len == 512).count() >= 3 * 19_200 / 512 - 3,
        "a 19.2 KB band is streamed as many buffer-sized chunks"
    );
}

#[test]
fn a_tile_block_rejects_inconsistent_geometry_before_the_bus_is_touched() {
    let (mut sink, log) = harness(full_panel());
    log.borrow_mut().clear();
    let tiles = vec![Rgb565::from_raw(0); TILE_PIXELS * 2];

    // Two tiles do not fill a 3-tile-wide window.
    assert!(matches!(
        sink.blit_tiles(Rect::new(0, 0, TILE_W * 3, TILE_H), &tiles, 3),
        Err(DisplayError::PixelCountMismatch)
    ));
    // Past the right edge.
    assert!(matches!(
        sink.blit_tiles(Rect::new(200, 0, TILE_W * 2, TILE_H), &tiles, 2),
        Err(DisplayError::TileOutOfBounds)
    ));
    assert!(log.borrow().is_empty());
}
