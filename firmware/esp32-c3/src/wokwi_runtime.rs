//! Wokwi PRODUCTION-RUNTIME mode (`wokwi-runtime` feature).
//!
//! Runs the **genuine** production loop — [`crate::runtime::run`], the same function the physical firmware
//! calls — against real esp-hal peripherals, with the simulation-only board profile supplying the pins and
//! panel model. There is no second runtime implementation here: this module only *constructs ports* and
//! prints observation markers.
//!
//! What differs from the physical binary: which GPIOs the SPI bus uses
//! ([`crate::profile::wokwi::WokwiSpiPins`]), which `mipidsi` model is instantiated (a generic MIPI-DCS
//! stand-in, not a claim about any real panel), and the panel geometry (`(0, 0)` offset, correct for the
//! Wokwi part and for nothing else). Everything above the ports is byte-identical.
//!
//! Markers are `KIVORI-RUN …`, printed only after the observation they describe. The completion marker is
//! emitted once every post-condition has genuinely been observed, so a scenario cannot pass because the
//! simulator exited.
//!
//! # Not a controller validation, and not physical evidence
//!
//! A green run proves the production loop boots, serves the protocol, keeps its lifecycle, renders through
//! the shared renderer, and drives tiles over SPI on the target ISA. It proves nothing about the physical
//! panel's controller, init sequence, offsets, orientation, colour order, or backlight, and nothing analog.

use crate::display::init_panel;
use crate::health::DeviceDiagnostic;
use crate::ports::InputSource;
use crate::profile::wokwi::{geometry, WokwiSpiPins};
use crate::proto::DeviceIdentity;
use crate::runtime::{run, RuntimeConfig, Tick};
use crate::transport::{TxBuffered, UsbJtagTransport};
use core::fmt::Write as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::peripherals::Peripherals;
use heapless::String;
use kivori_assets::AssetBlob;
use kivori_model::input::InputLevels;
use kivori_model::{Capabilities, CompanionState};
use kivori_protocol::FirmwareVersion;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ILI9341Rgb565;

/// A constant-level stub: always reports no motion on any channel.
///
/// Physical GPIO sampling for the rotary encoder is not implemented until Task 13; until then this
/// keeps the run loop's `InputSource` port wired with a placeholder that never produces a detent.
struct NoInput;

impl InputSource for NoInput {
    fn sample(&mut self) -> InputLevels {
        InputLevels {
            a: false,
            b: false,
            sw: false,
        }
    }
}

/// Marker prefix every line shares.
const TAG: &str = "KIVORI-RUN";

/// The transport the runtime mode drives: real USB Serial/JTAG, buffered so a whole frame survives a full
/// 64-byte FIFO.
type Link<'d> = TxBuffered<UsbJtagTransport<'d>>;

/// Prints one marker over the SAME USB Serial/JTAG link the protocol uses, so Wokwi's single serial
/// monitor sees both. The leading CRLF guarantees a marker starts a line even directly after binary
/// response bytes. Blocking on purpose: a lost marker would fail the scenario for the wrong reason.
fn line(link: &mut Link<'_>, text: &str) {
    let mut out: String<192> = String::new();
    let _ = write!(out, "\r\n{TAG} {text}\r\n");
    link.inner_mut().write_all(out.as_bytes());
}
/// Pixel-batch buffer for the `mipidsi` SPI interface.
const SPI_BATCH_BYTES: usize = 512;

fn identity() -> DeviceIdentity {
    DeviceIdentity {
        device_id: [0x5A; 16],
        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },
        capabilities: Capabilities::MASCOT_INTERACTION
            .union(Capabilities::PHYSICAL_INPUT_V1)
            .union(Capabilities::PRESENTATION_V1),
    }
}

/// Post-conditions the scenario asserts, all tracked from **observed** ticks.
#[derive(Default)]
struct Observed {
    booted_offline: bool,
    first_tiles: bool,
    quiet_frame: bool,
    idle_reported: bool,
    happy_reported: bool,
    diagnostic_seen: bool,
    health_seen: bool,
    recovered_after_reject: bool,
    hello_acked: bool,
    ponged: bool,
    announced: bool,
}

impl Observed {
    /// Folds one tick into the observation set, printing each transition exactly once.
    fn note(&mut self, tick: &Tick, rejected: u32, link: &mut Link<'_>) {
        if !self.booted_offline && tick.state == Some(CompanionState::Offline) {
            self.booted_offline = true;
            line(link, "PASS lifecycle-booting-to-offline");
        }
        if !self.first_tiles && tick.tiles_flushed == crate::render::TILE_COUNT as u32 {
            self.first_tiles = true;
            line(link, "PASS first-frame-36-tiles");
        }
        // A later tick that flushes nothing proves change-driven refresh over the real bus.
        if self.first_tiles && !self.quiet_frame && tick.tiles_flushed == 0 {
            self.quiet_frame = true;
            line(link, "PASS unchanged-frame-no-reflush");
        }
        match tick.state {
            Some(CompanionState::Idle) if !self.idle_reported => {
                self.idle_reported = true;
                line(link, "PASS state-idle");
            }
            Some(CompanionState::Happy) if !self.happy_reported => {
                self.happy_reported = true;
                line(link, "PASS state-happy");
            }
            _ => {}
        }
        if !self.hello_acked && tick.hello_acks > 0 {
            self.hello_acked = true;
            line(link, "PASS hello-ack");
        }
        if !self.ponged && tick.pongs > 0 {
            self.ponged = true;
            line(link, "PASS pong");
        }
        if let Some(diagnostic) = tick.diagnostic {
            if !self.diagnostic_seen && matches!(diagnostic, DeviceDiagnostic::FrameRejected(_)) {
                self.diagnostic_seen = true;
                // Category + code only — never the offending bytes (ADR-0005).
                let mut text: String<64> = String::new();
                let _ = write!(text, "PASS diagnostic label={}", diagnostic.label());
                line(link, &text);
            }
        }
        if !self.health_seen && tick.health_sent {
            self.health_seen = true;
            line(link, "PASS health-report");
        }
        // Recovery: a valid message answered after at least one frame had been rejected.
        if !self.recovered_after_reject && rejected > 0 && self.diagnostic_seen && self.ponged {
            self.recovered_after_reject = true;
            line(link, "PASS recovery-after-reject");
        }
    }

    fn complete(&self) -> bool {
        self.booted_offline
            && self.first_tiles
            && self.quiet_frame
            && self.idle_reported
            && self.happy_reported
            && self.diagnostic_seen
            && self.health_seen
            && self.hello_acked
            && self.ponged
            && self.recovered_after_reject
    }

    fn announce_if_complete(&mut self, link: &mut Link<'_>) {
        if !self.announced && self.complete() {
            self.announced = true;
            line(link, "ALL PASS production-runtime");
        }
    }
}

/// Brings up the board with the simulation-only profile and enters the production loop.
///
/// # Panics
/// Never returns. A bring-up failure prints `KIVORI-RUN FAIL` and parks, so a scenario fails on the missing
/// readiness marker rather than looking alive.
pub fn run_mode(peripherals: Peripherals, clock: crate::clock::EspClock, assets: &[u8]) -> ! {
    // The link comes up first, because every marker below — including a bring-up failure — travels over it.
    let mut serial: Link<'_> = TxBuffered::new(crate::bsp::serial(peripherals.USB_DEVICE));
    let mut boot: String<192> = String::new();
    let _ = write!(
        boot,
        "BOOT firmware=kivori-firmware target=esp32c3 mode=production-runtime profile=wokwi-sim \
         sck={} mosi={} cs={} dc={} rst={}",
        WokwiSpiPins::SCK,
        WokwiSpiPins::MOSI,
        WokwiSpiPins::CS,
        WokwiSpiPins::DC,
        WokwiSpiPins::RST
    );
    line(&mut serial, &boot);
    let cs = Output::new(peripherals.GPIO6, Level::High, OutputConfig::default());
    let dc = Output::new(peripherals.GPIO7, Level::Low, OutputConfig::default());
    let reset = Output::new(peripherals.GPIO10, Level::High, OutputConfig::default());
    let Ok(bus) =
        crate::bsp::display_bus(peripherals.SPI2, peripherals.GPIO4, peripherals.GPIO5, cs)
    else {
        line(&mut serial, "FAIL spi-bring-up");
        park();
    };

    let mut buffer = [0u8; SPI_BATCH_BYTES];
    let interface = SpiInterface::new(bus, dc, &mut buffer);
    let mut delay = Delay::new();
    // A GENERIC MIPI-DCS model, chosen because the Wokwi part accepts it. Not a claim about real hardware.
    let Ok(mut display) = init_panel(ILI9341Rgb565, interface, reset, &mut delay, geometry())
    else {
        line(&mut serial, "FAIL panel-init");
        park();
    };

    let Ok(blob) = AssetBlob::parse(assets) else {
        line(&mut serial, "FAIL assets-parse");
        park();
    };

    line(
        &mut serial,
        "READY iface=usb-serial-jtag display=generic-spi",
    );

    let mut observed = Observed::default();
    run(
        identity(),
        RuntimeConfig::default(),
        &clock,
        &mut serial,
        &mut NoInput,
        &mut display,
        &blob,
        |tick, link| {
            observed.note(tick, tick.rejected_frames, link);
            observed.announce_if_complete(link);
        },
    );
}

/// Parks the CPU after a bring-up failure.
fn park() -> ! {
    loop {
        esp_hal::delay::Delay::new().delay_millis(1000);
    }
}
