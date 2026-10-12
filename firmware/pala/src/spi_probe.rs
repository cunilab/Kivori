//! Wokwi stage-2 GENERIC SPI / RGB565 tile-transfer probe (`wokwi-spi` feature; T131).
//!
//! Drives the **real** T072 [`crate::display::MipidsiSink`] over the **real** esp-hal SPI peripheral
//! against Wokwi's built-in `wokwi-ili9341` part, and asserts what a simulator can honestly prove about a
//! display bus:
//!
//! * **Scope A — generic SPI transactions**: the peripheral initialises, the reset line is sequenced, CS
//!   frames every transaction, D/C separates command from data phases, and transfer boundaries are
//!   counted.
//! * **Scope B — generic RGB565 tile stream**: thirty-six 40x40 tiles per frame, 1,600 pixels each, an
//!   exact deterministic byte volume, correct tile positions, no retransmission for an unchanged frame,
//!   and retransmission when the state changes.
//!
//! # What this is NOT
//!
//! **Scope C — controller-specific validation — is deliberately absent.** The Kivori panel's controller is
//! unconfirmed. `wokwi-ili9341` is used here ONLY as a generic SPI/RGB565 sink; nothing below asserts an
//! ILI9341, ST7789, or GC9A01 initialisation sequence, panel offset, orientation, colour order, or
//! backlight behaviour, and no such claim may be derived from a green run. The pin numbers in
//! [`crate::profile::wokwi::WokwiSpiPins`] are a **simulation-only** profile matching `diagram-spi.json`;
//! they are not the Kivori hardware pin map, which is still unassigned.
//!
//! Nothing electrical is proven: no signal integrity, no timing margin, no real panel latency.
//!
//! Every marker below is printed only *after* the work it names has completed and its post-condition has
//! been checked — a marker never stands in for the check.

use crate::display::init_panel;
use crate::ports::DisplaySink;
use crate::profile::wokwi::{geometry, WokwiSpiPins};
use crate::render::{TileRenderer, TILE_COLS, TILE_COUNT, TILE_H, TILE_PIXELS, TILE_W};
use core::cell::Cell;
use embedded_hal::digital::{ErrorType as DigitalErrorType, OutputPin};
use embedded_hal::spi::{ErrorType as SpiErrorType, Operation, SpiDevice};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::peripherals::Peripherals;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode;
use esp_hal::time::Rate;
use esp_println::println;
use kivori_assets::AssetBlob;
use kivori_model::{CompanionState, Rect, Rgb565};
use mipidsi::interface::SpiInterface;
use mipidsi::models::ILI9341Rgb565;

/// The canonical asset blob, compiled at build time (see `build.rs`). Never parsed from source art.
static ASSETS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/kivori.assets"));

/// Marker prefix every line shares, so scenarios can filter the stream.
const TAG: &str = "KIVORI-SPI";

/// Pixel-data buffer the `mipidsi` SPI interface batches through. Sized for a whole tile row.
const SPI_BATCH_BYTES: usize = 512;

/// Bytes a single tile contributes to one frame: two address-window commands with four argument bytes
/// each, plus two bytes per RGB565 pixel.
const TILE_DATA_BYTES: u32 = 4 + 4 + (TILE_PIXELS as u32) * 2;
/// Data-phase bytes for one full 240x240 frame.
const FRAME_DATA_BYTES: u32 = TILE_DATA_BYTES * TILE_COUNT as u32;
/// Command-phase bytes for one full frame: CASET + RASET + RAMWR per tile.
const FRAME_COMMAND_BYTES: u32 = 3 * TILE_COUNT as u32;

/// Bus-activity counters shared by the instrumented SPI device and pins.
///
/// Counting happens at the `embedded-hal` boundary, so the numbers describe what the firmware genuinely
/// clocked out — they do not depend on Wokwi's VCD signal naming.
#[derive(Default)]
pub struct BusCounters {
    /// Completed SPI transactions (each is one CS-framed exchange).
    transactions: Cell<u32>,
    /// Bytes clocked while D/C was high (arguments + pixels).
    data_bytes: Cell<u32>,
    /// Bytes clocked while D/C was low (opcodes).
    command_bytes: Cell<u32>,
    /// D/C level changes.
    dc_transitions: Cell<u32>,
    /// CS level changes.
    cs_transitions: Cell<u32>,
    /// Reset level changes.
    reset_transitions: Cell<u32>,
    /// Current D/C level, so byte counts can be attributed to a phase.
    dc_high: Cell<bool>,
    /// Last observed CS level.
    cs_high: Cell<bool>,
    /// Last observed reset level.
    reset_high: Cell<bool>,
}

impl BusCounters {
    /// Fresh counters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            transactions: Cell::new(0),
            data_bytes: Cell::new(0),
            command_bytes: Cell::new(0),
            dc_transitions: Cell::new(0),
            cs_transitions: Cell::new(0),
            reset_transitions: Cell::new(0),
            dc_high: Cell::new(false),
            cs_high: Cell::new(true),
            reset_high: Cell::new(true),
        }
    }

    /// Data-phase bytes counted so far.
    #[must_use]
    pub fn data_bytes(&self) -> u32 {
        self.data_bytes.get()
    }

    /// Command-phase bytes counted so far.
    #[must_use]
    pub fn command_bytes(&self) -> u32 {
        self.command_bytes.get()
    }

    fn note_level(level: bool, last: &Cell<bool>, transitions: &Cell<u32>) {
        if last.get() != level {
            last.set(level);
            transitions.set(transitions.get() + 1);
        }
    }
}

/// Which line a [`CountedPin`] drives.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PinRole {
    /// Data/command select.
    Dc,
    /// Chip select.
    Cs,
    /// Panel reset.
    Reset,
}

/// An `OutputPin` that counts transitions before delegating.
struct CountedPin<'c, P> {
    inner: P,
    counters: &'c BusCounters,
    role: PinRole,
}

impl<P: OutputPin> DigitalErrorType for CountedPin<'_, P> {
    type Error = P::Error;
}

impl<P: OutputPin> CountedPin<'_, P> {
    fn note(&self, level: bool) {
        let c = self.counters;
        match self.role {
            PinRole::Dc => {
                BusCounters::note_level(level, &c.dc_high, &c.dc_transitions);
            }
            PinRole::Cs => BusCounters::note_level(level, &c.cs_high, &c.cs_transitions),
            PinRole::Reset => BusCounters::note_level(level, &c.reset_high, &c.reset_transitions),
        }
    }
}

impl<P: OutputPin> OutputPin for CountedPin<'_, P> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.note(false);
        self.inner.set_low()
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.note(true);
        self.inner.set_high()
    }
}

/// A `SpiDevice` that counts transactions and phase-attributed bytes before delegating.
struct CountedSpi<'c, S> {
    inner: S,
    counters: &'c BusCounters,
}

impl<S: SpiDevice<u8>> SpiErrorType for CountedSpi<'_, S> {
    type Error = S::Error;
}

impl<S: SpiDevice<u8>> SpiDevice<u8> for CountedSpi<'_, S> {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        let mut bytes = 0u32;
        for op in operations.iter() {
            if let Operation::Write(buf) = op {
                bytes += buf.len() as u32;
            }
        }
        let c = self.counters;
        if c.dc_high.get() {
            c.data_bytes.set(c.data_bytes.get() + bytes);
        } else {
            c.command_bytes.set(c.command_bytes.get() + bytes);
        }
        c.transactions.set(c.transactions.get() + 1);
        self.inner.transaction(operations)
    }
}

/// A [`DisplaySink`] decorator counting tile blits and pixels, so tile geometry can be asserted without
/// a second renderer.
struct CountingSink<S> {
    inner: S,
    blits: u32,
    pixels: u32,
    geometry_ok: bool,
}

impl<S: DisplaySink> CountingSink<S> {
    const fn new(inner: S) -> Self {
        Self {
            inner,
            blits: 0,
            pixels: 0,
            geometry_ok: true,
        }
    }

    fn reset(&mut self) {
        self.blits = 0;
        self.pixels = 0;
        self.geometry_ok = true;
    }
}

impl<S: DisplaySink> DisplaySink for CountingSink<S> {
    type Error = S::Error;

    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        let tile = self.blits as usize;
        let expected_x = (tile % TILE_COLS) as u16 * TILE_W;
        let expected_y = (tile / TILE_COLS) as u16 * TILE_H;
        if rect.x != expected_x
            || rect.y != expected_y
            || rect.w != TILE_W
            || rect.h != TILE_H
            || pixels.len() != TILE_PIXELS
        {
            self.geometry_ok = false;
        }
        self.blits += 1;
        self.pixels += pixels.len() as u32;
        self.inner.blit_tile(rect, pixels)
    }
}

/// Prints `PASS`/`FAIL` for one stage and folds the result into `pass`.
fn check(pass: &mut bool, ok: bool, stage: &str) {
    if ok {
        println!("{TAG} PASS {stage}");
    } else {
        println!("{TAG} FAIL {stage}");
        *pass = false;
    }
}

/// Fills `buf` with a solid RGB565 colour.
fn fill_solid(buf: &mut [Rgb565; TILE_PIXELS], color: u16) {
    for px in buf.iter_mut() {
        *px = Rgb565::from_raw(color);
    }
}

/// Fills `buf` with an 8x8 checkerboard, deterministically derived from the tile's y origin.
fn fill_checkerboard(buf: &mut [Rgb565; TILE_PIXELS], tile_x: u16, tile_y: u16) {
    for (i, px) in buf.iter_mut().enumerate() {
        let x = tile_x + (i % TILE_W as usize) as u16;
        let y = tile_y + (i / TILE_W as usize) as u16;
        let on = ((x / 8) + (y / 8)).is_multiple_of(2);
        *px = Rgb565::from_raw(if on { 0xFFFF } else { 0x0000 });
    }
}

/// Blits a full 240x240 frame from `buf`, refilled per tile by `fill`.
fn blit_frame<S: DisplaySink>(
    sink: &mut CountingSink<S>,
    buf: &mut [Rgb565; TILE_PIXELS],
    mut fill: impl FnMut(&mut [Rgb565; TILE_PIXELS], u16, u16),
) -> bool {
    sink.reset();
    for tile in 0..TILE_COUNT {
        let x = (tile % TILE_COLS) as u16 * TILE_W;
        let y = (tile / TILE_COLS) as u16 * TILE_H;
        fill(buf, x, y);
        if sink
            .blit_tile(Rect::new(x, y, TILE_W, TILE_H), buf.as_slice())
            .is_err()
        {
            return false;
        }
    }
    sink.blits == TILE_COUNT as u32
        && sink.pixels == TILE_COUNT as u32 * TILE_PIXELS as u32
        && sink.geometry_ok
}

/// Runs the probe. Returns `true` only if every stage passed.
///
/// # Panics
/// Does not panic; every fallible step is folded into a `FAIL` marker instead.
#[allow(clippy::too_many_lines)]
pub fn run(peripherals: Peripherals) -> bool {
    let mut pass = true;
    println!(
        "{TAG} BOOT firmware=kivori-firmware target=esp32c3 panel=generic-spi sck={} mosi={} cs={} dc={} rst={}",
        WokwiSpiPins::SCK,
        WokwiSpiPins::MOSI,
        WokwiSpiPins::CS,
        WokwiSpiPins::DC,
        WokwiSpiPins::RST
    );

    let counters = BusCounters::new();

    // ── Stage 1: SPI bus + GPIO bring-up ────────────────────────────────────────────────────────────
    let spi_config = SpiConfig::default()
        .with_frequency(Rate::from_mhz(20))
        .with_mode(Mode::_0);
    let bus = match Spi::new(peripherals.SPI2, spi_config) {
        Ok(spi) => spi.with_sck(peripherals.GPIO4).with_mosi(peripherals.GPIO5),
        Err(_) => {
            check(&mut pass, false, "bus-init");
            println!("{TAG} ALL FAIL");
            return false;
        }
    };
    let cs = CountedPin {
        inner: Output::new(peripherals.GPIO6, Level::High, OutputConfig::default()),
        counters: &counters,
        role: PinRole::Cs,
    };
    let dc = CountedPin {
        inner: Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default()),
        counters: &counters,
        role: PinRole::Dc,
    };
    let reset = CountedPin {
        inner: Output::new(peripherals.GPIO10, Level::High, OutputConfig::default()),
        counters: &counters,
        role: PinRole::Reset,
    };
    let device = match ExclusiveDevice::new(bus, cs, Delay::new()) {
        Ok(device) => CountedSpi {
            inner: device,
            counters: &counters,
        },
        Err(_) => {
            check(&mut pass, false, "bus-init");
            println!("{TAG} ALL FAIL");
            return false;
        }
    };
    check(&mut pass, true, "bus-init");

    // ── Stage 2: panel reset + initialisation activity ──────────────────────────────────────────────
    //
    // The model below is a GENERIC MIPI-DCS sink, not a claim about the Kivori panel. Offsets are (0, 0)
    // because that is what the Wokwi part expects — it is not a measurement of any real panel.
    let mut buffer = [0u8; SPI_BATCH_BYTES];
    let interface = SpiInterface::new(device, dc, &mut buffer);
    let mut delay = Delay::new();
    let geometry = geometry();
    let sink = match init_panel(ILI9341Rgb565, interface, reset, &mut delay, geometry) {
        Ok(sink) => sink,
        Err(_) => {
            check(&mut pass, false, "reset-sequence");
            println!("{TAG} ALL FAIL");
            return false;
        }
    };
    // A real bring-up must have driven reset both ways, separated command from data with D/C, framed
    // transactions with CS, and clocked at least one opcode.
    let init_ok = counters.reset_transitions.get() >= 2
        && counters.dc_transitions.get() >= 2
        && counters.cs_transitions.get() >= 2
        && counters.command_bytes.get() > 0;
    check(&mut pass, init_ok, "reset-sequence");
    println!(
        "{TAG} INFO init transactions={} cmd-bytes={} data-bytes={} dc={} cs={} rst={}",
        counters.transactions.get(),
        counters.command_bytes.get(),
        counters.data_bytes.get(),
        counters.dc_transitions.get(),
        counters.cs_transitions.get(),
        counters.reset_transitions.get()
    );

    let mut sink = CountingSink::new(sink);
    let mut tile = [Rgb565::from_raw(0); TILE_PIXELS];

    // ── Stages 3-5: solid RGB565 frames ────────────────────────────────────────────────────────────
    for (color, stage) in [
        (0xF800u16, "rgb-red"),
        (0x07E0, "rgb-green"),
        (0x001F, "rgb-blue"),
    ] {
        let before = counters.data_bytes();
        let ok = blit_frame(&mut sink, &mut tile, |buf, _x, _y| fill_solid(buf, color));
        let bytes = counters.data_bytes() - before;
        check(&mut pass, ok && bytes == FRAME_DATA_BYTES, stage);
    }

    // ── Stage 6: checkerboard ──────────────────────────────────────────────────────────────────────
    let before = counters.data_bytes();
    let checker_ok = blit_frame(&mut sink, &mut tile, fill_checkerboard);
    let checker_bytes = counters.data_bytes() - before;
    check(
        &mut pass,
        checker_ok && checker_bytes == FRAME_DATA_BYTES,
        "checkerboard",
    );

    // ── Stages 7-10: the canonical shared renderer over the same bus ────────────────────────────────
    match AssetBlob::parse(ASSETS) {
        Ok(blob) => {
            let mut renderer = TileRenderer::new();
            sink.reset();
            let before_data = counters.data_bytes();
            let before_cmd = counters.command_bytes();
            let rendered = renderer
                .render(&blob, CompanionState::Idle, 0, &mut sink)
                .is_ok();
            let frame_data = counters.data_bytes() - before_data;
            let frame_cmd = counters.command_bytes() - before_cmd;

            // Six full-width bands, in top-to-bottom order, 9,600 pixels each.
            let geometry_ok = rendered
                && sink.blits == TILE_COUNT as u32
                && sink.geometry_ok
                && sink.pixels == TILE_COUNT as u32 * TILE_PIXELS as u32;
            check(&mut pass, geometry_ok, "tile-count-36");
            println!(
                "{TAG} INFO frame data-bytes={frame_data} cmd-bytes={frame_cmd} pixels={}",
                sink.pixels
            );
            // Exact, deterministic transfer volume — nothing padded, nothing dropped.
            check(
                &mut pass,
                frame_data == FRAME_DATA_BYTES && frame_cmd == FRAME_COMMAND_BYTES,
                "tile-bytes",
            );

            // An identical frame must not touch the bus at all (FR-013).
            sink.reset();
            let quiet_before = counters.data_bytes();
            let again = renderer
                .render(&blob, CompanionState::Idle, 0, &mut sink)
                .is_ok();
            check(
                &mut pass,
                again && sink.blits == 0 && counters.data_bytes() == quiet_before,
                "unchanged-no-reflush",
            );

            // A changed state transmits only tiles whose pixels changed; common background stays cached.
            sink.reset();
            let changed_before = counters.data_bytes();
            let changed = renderer
                .render(&blob, CompanionState::Happy, 0, &mut sink)
                .is_ok();
            check(
                &mut pass,
                changed
                    && sink.blits > 0
                    && (sink.blits as usize) < TILE_COUNT
                    && counters.data_bytes() - changed_before == sink.blits * TILE_DATA_BYTES,
                "changed-reflush",
            );
        }
        Err(_) => {
            check(&mut pass, false, "tile-count-36");
            check(&mut pass, false, "tile-bytes");
            check(&mut pass, false, "unchanged-no-reflush");
            check(&mut pass, false, "changed-reflush");
        }
    }

    println!(
        "{TAG} INFO totals transactions={} cmd-bytes={} data-bytes={}",
        counters.transactions.get(),
        counters.command_bytes.get(),
        counters.data_bytes.get()
    );
    if pass {
        println!("{TAG} ALL PASS");
    } else {
        println!("{TAG} ALL FAIL");
    }
    pass
}
