//! Physical RP2040-Zero + ST7789 240x240 Kivori runtime.
//!
//! All verified physical display facts live in
//! [`crate::profile::physical_st7789`].
//!
//! `rp2040-hal` exposes GPIOs as typed fields (`pins.gpio15`, etc.), so the concrete pin
//! selections remain here. Compile-time assertions below ensure they cannot silently drift from
//! the physical profile.

use core::convert::Infallible;

use embedded_hal::digital::{ErrorType, OutputPin};
use embedded_hal::pwm::SetDutyCycle;
use embedded_hal_bus::spi::ExclusiveDevice;

use rp2040_hal::fugit::RateExtU32;
use rp2040_hal::gpio::{FunctionSpi, PinState, Pins};
use rp2040_hal::pwm::{Channel, FreeRunning, Pwm6, Slice, Slices, A};
use rp2040_hal::usb::UsbBus;
use rp2040_hal::{Clock as _, Sio, Spi};
use usb_device::class_prelude::UsbBusAllocator;

use kivori_assets::AssetBlob;
use kivori_model::desk::BRIGHTNESS_DEFAULT;
use kivori_model::{Rect, Rgb565};
use kivori_protocol::identity::ChipRecordState;
use kivori_protocol::FirmwareVersion;

use mipidsi::{interface::SpiInterface, Builder};

use crate::{
    bsp::Board,
    chip_probe,
    device_id::{derive_device_id, usb_serial_number},
    display::MipidsiSink,
    identity::{check_boot, ProductIdentity, IMAGE_RECORD},
    physical_led::{self, RgbLed},
    physical_rotary::PhysicalRotary,
    ports::{Clock, DisplaySink, LedSink},
    profile::physical_st7789 as hw,
    proto::DeviceIdentity,
    render::FRAME_PIXELS,
    runtime::{run_buffered, RuntimeConfig},
    transport::{TxBuffered, UsbCdcTransport},
};

/// Parses one decimal component of the crate version at compile time. A malformed version fails
/// the build (a const panic) instead of shipping a wrong number.
const fn version_part(text: &str) -> u16 {
    let bytes = text.as_bytes();
    assert!(!bytes.is_empty(), "empty version component");
    let mut value: u16 = 0;
    let mut i = 0;
    while i < bytes.len() {
        let digit = bytes[i];
        assert!(digit.is_ascii_digit(), "non-numeric version component");
        value = value * 10 + (digit - b'0') as u16;
        i += 1;
    }
    value
}

/// The firmware version advertised in `HelloAck`: the firmware crate's own version, so there is
/// one place to bump it.
const FIRMWARE_VERSION: FirmwareVersion = FirmwareVersion {
    major: version_part(env!("CARGO_PKG_VERSION_MAJOR")),
    minor: version_part(env!("CARGO_PKG_VERSION_MINOR")),
    patch: version_part(env!("CARGO_PKG_VERSION_PATCH")),
};

/// The same version as searchable text inside the image, so tooling can read which firmware a
/// built `.elf` or `.bin` holds without running it. `main` reads it once at boot: `#[used]` alone
/// does not stop the linker from discarding an unreferenced section.
#[used]
static FW_VERSION_MARKER: &[u8] =
    concat!("KIVORI-FW-VERSION:", env!("CARGO_PKG_VERSION"), "\0").as_bytes();

/// The image identity record (product, board, firmware version, minimum protocol) as searchable
/// text beside the version marker; the app checks it against the connected device before it
/// flashes. Kept in the image the same way as [`FW_VERSION_MARKER`].
#[used]
static FW_IDENTITY_MARKER: &[u8] = IMAGE_RECORD.as_bytes();

/// The flash chip's 64-bit unique id and its JEDEC id (the RP2040 has neither). Both are read
/// through the ROM's flash commands, which need XIP off: interrupts are disabled here, and USB, DMA
/// and the input interrupt are not started yet.
fn read_flash_ids() -> ([u8; 8], u32) {
    let mut unique = [0u8; 8];
    // SAFETY: nothing else reads flash while interrupts are off and core 1 is not started; the
    // second-stage bootloader is restored afterwards (`use_boot2`).
    let jedec = critical_section::with(|_| unsafe {
        rp2040_flash::flash::flash_unique_id(&mut unique, true);
        rp2040_flash::flash::flash_jedec_id(true)
    });
    (unique, jedec)
}

/// The USB serial number the host shows: the derived device id as hex, never the raw flash id.
static USB_SERIAL: static_cell::StaticCell<heapless::String<32>> = static_cell::StaticCell::new();

/// The USB bus allocator, which the CDC class and device borrow for as long as the firmware runs.
static USB_BUS: static_cell::StaticCell<UsbBusAllocator<UsbBus>> = static_cell::StaticCell::new();

/// PWM period shared by the backlight and the LED: 125 MHz / (20 * 256) = 24 kHz, above hearing,
/// with 8-bit duty (far finer than the five backlight steps need).
const PWM_DIV: u8 = 20;
const PWM_TOP: u16 = 255;

/// Starts one PWM slice with the shared period.
fn start_pwm<I: rp2040_hal::pwm::SliceId>(slice: &mut Slice<I, FreeRunning>)
where
    FreeRunning: rp2040_hal::pwm::ValidSliceMode<I>,
{
    slice.set_div_int(PWM_DIV);
    slice.set_top(PWM_TOP);
    slice.enable();
}

/// Bytes mipidsi stages per SPI transaction. This fits a complete 40x40 RGB565 tile (3,200 bytes)
/// and is also the maximum mipidsi SPI batch size.
const SPI_BUFFER_BYTES: usize = 4_096;

/// Physical-mode staging memory lives in `.bss`, never on the RP2040 stack.
static FRAME_BUFFER: static_cell::ConstStaticCell<[Rgb565; FRAME_PIXELS]> =
    static_cell::ConstStaticCell::new([Rgb565::from_raw(0); FRAME_PIXELS]);

/// The parsed asset manifest (about 31 KiB). It lives in `.bss`, not in `run_mode`'s stack frame,
/// which stays live for the whole run.
static ASSET_BLOB: static_cell::StaticCell<AssetBlob<'static>> = static_cell::StaticCell::new();

/// Parses the compiled assets into [`ASSET_BLOB`]. Kept out of line so the parse temporaries (each
/// move of the manifest is another copy on the stack) are released before the run loop starts.
#[inline(never)]
pub fn load_assets(assets: &'static [u8]) -> &'static AssetBlob<'static> {
    ASSET_BLOB.init_with(|| match AssetBlob::parse(assets) {
        Ok(blob) => blob,
        Err(error) => panic!("Kivori asset blob: {error:?}"),
    })
}

/// The panel sink plus its backlight PWM channel (GP12 = slice 6 A): pixels go to the panel,
/// brightness to the pin.
struct Backlit<S> {
    panel: S,
    channel: Channel<Slice<Pwm6, FreeRunning>, A>,
}

impl<S: DisplaySink> DisplaySink for Backlit<S> {
    type Error = S::Error;

    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        self.panel.blit_tile(rect, pixels)
    }

    fn blit_tiles(&mut self, rect: Rect, tiles: &[Rgb565], cols: u16) -> Result<(), Self::Error> {
        self.panel.blit_tiles(rect, tiles, cols)
    }

    fn set_brightness(&mut self, step: u8) {
        // A failed duty write leaves the previous brightness: harmless, and the next setting retries.
        let _ = self
            .channel
            .set_duty_cycle_percent(hw::backlight_duty_pct(step));
    }
}

/// Keep the typed `rp2040-hal` GPIO selections below synchronized with the numeric hardware
/// profile.
///
/// If somebody changes a pin in `profile.rs` without updating this file, the physical firmware
/// build fails instead of silently using the wrong wiring.
const _: () = {
    assert!(hw::SCK == 26);
    assert!(hw::MOSI == 15);
    assert!(hw::DC == 13);
    assert!(hw::RST == 14);
    assert!(hw::BL == 12);
    assert!(hw::CS.is_none());
    assert!(hw::LED[0] == 9);
    assert!(hw::LED[1] == 10);
    assert!(hw::LED[2] == 11);
    assert!(hw::ROTARY.clk == 5);
    assert!(hw::ROTARY.dt == 4);
    assert!(hw::ROTARY.sw == 3);
    assert!(hw::CONTEXT_BUTTONS[0] == 6);
    assert!(hw::CONTEXT_BUTTONS[1] == 7);
    assert!(hw::CONTEXT_BUTTONS[2] == 8);
};

/// Runs Kivori on the physical RP2040-Zero + ST7789 hardware.
///
/// This function owns the USB CDC transport, SPI1, display control pins, backlight, RGB LED and the
/// production runtime. It never returns.
///
/// `blob` comes from [`load_assets`], which `main` calls first: its parse needs about 110 KiB of
/// stack, so it runs before this function's own frame (7.5 KiB) is on the stack.
pub fn run_mode(mut board: Board, blob: &'static AssetBlob<'static>, asset_bytes: usize) -> ! {
    // The flash ids come first: they need interrupts off and no other flash user.
    let (flash_unique_id, flash_jedec_id) = read_flash_ids();

    let sio = Sio::new(board.sio);
    let pins = Pins::new(board.io, board.pads, sio.gpio_bank0, &mut board.resets);

    // -------------------------------------------------------------------------
    // SPI1
    //
    // Physical wiring (the v7 carrier's map, #179):
    //   SCK  -> GP26
    //   MOSI -> GP15
    //
    // No MISO is needed because the display path is write-only. The CPU feeds the SPI FIFO
    // (no DMA yet), so a window is one blocking transfer.
    // -------------------------------------------------------------------------

    let spi = Spi::<_, _, _, 8>::new(
        board.spi1,
        (
            pins.gpio15.into_function::<FunctionSpi>(),
            pins.gpio26.into_function::<FunctionSpi>(),
        ),
    )
    .init(
        &mut board.resets,
        board.clocks.peripheral_clock.freq(),
        hw::SPI_CLOCK_HZ.Hz(),
        hw::spi_mode(),
    );

    // -------------------------------------------------------------------------
    // Display control pins
    //
    // Physical wiring (the v7 carrier's map, #179):
    //   DC  -> GP13
    //   RST -> GP14
    //   BL  -> GP12 (PWM slice 6, channel A)
    // -------------------------------------------------------------------------

    let dc = pins.gpio13.into_push_pull_output();
    let rst = pins.gpio14.into_push_pull_output_in_state(PinState::High);

    // -------------------------------------------------------------------------
    // PWM: backlight and RGB LED
    //
    // The backlight is plainly on while the panel initialises (full duty) and follows the desktop's
    // brightness setting once it is up. The LED starts off, and stays off until the app sends settings.
    // -------------------------------------------------------------------------

    let mut slices = Slices::new(board.pwm, &mut board.resets);
    start_pwm(&mut slices.pwm4);
    start_pwm(&mut slices.pwm5);
    start_pwm(&mut slices.pwm6);
    let mut backlight_channel = slices.pwm6.channel_a;
    backlight_channel.output_to(pins.gpio12);
    let _ = backlight_channel.set_duty_cycle_percent(100);
    let mut led = RgbLed {
        red: slices.pwm4.channel_b,
        green: slices.pwm5.channel_a,
        blue: slices.pwm5.channel_b,
    };
    led.red.output_to(pins.gpio9);
    led.green.output_to(pins.gpio10);
    led.blue.output_to(pins.gpio11);
    led.set_rgb([0; 3]);
    // From here a 50 Hz timer interrupt drives the LED from the engine the run loop publishes.
    physical_led::start(board.timer.alarm_0().expect("alarm 0 is free"), led);

    // DEVELOPMENT ONLY: the payload trace goes out of UART0 (GP0 TX, GP1 RX).
    #[cfg(feature = "debug-payloads")]
    {
        use rp2040_hal::gpio::FunctionUart;
        use rp2040_hal::uart::{DataBits, StopBits, UartConfig, UartPeripheral};
        let uart_pins = (
            pins.gpio0.into_function::<FunctionUart>(),
            pins.gpio1.into_function::<FunctionUart>(),
        );
        let uart = UartPeripheral::new(board.uart0, uart_pins, &mut board.resets)
            .enable(
                UartConfig::new(115_200.Hz(), DataBits::Eight, None, StopBits::One),
                board.clocks.peripheral_clock.freq(),
            )
            .expect("debug UART");
        crate::debug_payloads::init(uart);
    }

    // -------------------------------------------------------------------------
    // SPI device
    //
    // The verified physical ST7789 module does not use a CS line, so
    // `NoChipSelect` satisfies the `SpiDevice` abstraction without toggling
    // another GPIO.
    // -------------------------------------------------------------------------

    let spi_dev = ExclusiveDevice::new(spi, NoChipSelect, board.timer).expect("SPI device");

    let mut interface_buffer = [0u8; SPI_BUFFER_BYTES];

    let interface = SpiInterface::new(spi_dev, dc, &mut interface_buffer);

    // -------------------------------------------------------------------------
    // ST7789 initialization
    //
    // Controller, geometry, orientation, RGB/BGR order, inversion, SPI mode,
    // and offsets all come from the verified physical profile.
    // -------------------------------------------------------------------------

    let display = Builder::new(hw::panel_model(), interface)
        .reset_pin(rst)
        .display_size(hw::WIDTH, hw::HEIGHT)
        .display_offset(hw::OFFSET_X, hw::OFFSET_Y)
        .orientation(hw::orientation())
        .color_order(hw::color_order())
        .invert_colors(hw::color_inversion())
        .init(&mut board.timer)
        .expect("ST7789 init");

    // -------------------------------------------------------------------------
    // Kivori display sink
    //
    // Converts the initialized mipidsi display into Kivori's generic
    // DisplaySink. The renderer will send change-driven 40x40 RGB565 tiles.
    // -------------------------------------------------------------------------

    let display = MipidsiSink::new(display, hw::geometry());
    let _ = backlight_channel.set_duty_cycle_percent(hw::backlight_duty_pct(BRIGHTNESS_DEFAULT));
    let mut display = Backlit {
        panel: display,
        channel: backlight_channel,
    };

    // -------------------------------------------------------------------------
    // Rotary encoder input
    //
    // Wiring specification (unverified on physical hardware; see
    // `profile::physical_st7789::ROTARY`):
    //   SW  -> GP3
    //   DT  -> GP4
    //   CLK -> GP5
    //
    // All lines are active-low (HW-040 COM to GND, its own pull-ups to 3V3), read with the
    // internal pull-up too. `PhysicalRotary` only reads and inverts pin levels; all conditioning
    // and semantics live above the port.
    // -------------------------------------------------------------------------

    let rotary_clk = pins.gpio5.into_pull_up_input().into_dyn_pin();
    let rotary_dt = pins.gpio4.into_pull_up_input().into_dyn_pin();
    let rotary_sw = pins.gpio3.into_pull_up_input().into_dyn_pin();
    // Contextual buttons: GP6 / GP7 / GP8 to GND, same pull-up active-low reading
    // (`profile::physical_st7789::CONTEXT_BUTTONS`).
    let keys = [
        pins.gpio6.into_pull_up_input().into_dyn_pin(),
        pins.gpio7.into_pull_up_input().into_dyn_pin(),
        pins.gpio8.into_pull_up_input().into_dyn_pin(),
    ];
    let mut rotary = PhysicalRotary::new(rotary_clk, rotary_dt, rotary_sw, keys);

    // -------------------------------------------------------------------------
    // Compiled Kivori assets
    // -------------------------------------------------------------------------

    chip_probe::init(board.adc, &mut board.resets, asset_bytes, flash_jedec_id);

    // -------------------------------------------------------------------------
    // Device identity
    // -------------------------------------------------------------------------

    let identity = DeviceIdentity {
        // Stable per-unit ID: hash of the flash chip's unique ID (the RP2040 has none of its own).
        // The raw ID never goes on the wire.
        device_id: derive_device_id(&flash_unique_id),

        firmware_version: FIRMWARE_VERSION,

        capabilities: hw::CAPABILITIES,
    };

    // Tier A (ADR-0016): a valid chip record for another product or board means this image must
    // not run the product. The RP2040 has no one-time memory to burn a record into, so there is
    // never one and the build identity applies.
    let boot = check_boot(ProductIdentity::BUILD, ChipRecordState::NotProvisioned);

    // Keeps `FW_VERSION_MARKER` (and so the version text) in the linked image.
    // SAFETY: a valid, aligned read of the first byte of a live static.
    let _ = unsafe { core::ptr::read_volatile(FW_VERSION_MARKER.as_ptr()) };
    // SAFETY: as above, for the identity record.
    let _ = unsafe { core::ptr::read_volatile(FW_IDENTITY_MARKER.as_ptr()) };

    // -------------------------------------------------------------------------
    // USB CDC transport
    //
    // Started last, after the flash-id reads that need interrupts off. From here the USB interrupt
    // services the stack, so enumeration proceeds even while a display flush occupies the loop.
    // -------------------------------------------------------------------------

    let serial_number = USB_SERIAL.init(usb_serial_number(&identity.device_id));
    let usb_bus = UsbBus::new(
        board.usb.0,
        board.usb.1,
        board.clocks.usb_clock,
        true,
        &mut board.resets,
    );
    let usb_bus = USB_BUS.init(UsbBusAllocator::new(usb_bus));
    let mut transport = TxBuffered::new(UsbCdcTransport::new(usb_bus, serial_number.as_str()));

    // -------------------------------------------------------------------------
    // Production runtime
    //
    // Handles:
    // - USB protocol
    // - Hello / handshake
    // - SetState
    // - StateReport
    // - heartbeat
    // - lifecycle
    // - shared renderer
    // - change-driven display updates
    //
    // Never returns.
    // -------------------------------------------------------------------------

    // Keeping a complete RGB565 frame in static storage lets the renderer
    // compose all changed pixels before it sends the first window to ST7789.
    // This avoids exposing a mixture of old and new mascot rows while a pose
    // is being composed. The frame costs 115,200 bytes of RP2040 SRAM.
    let frame_buffer = FRAME_BUFFER.take();
    let clock = board.clock;

    run_buffered(
        identity,
        RuntimeConfig::default(),
        &clock,
        &mut transport,
        &mut rotary,
        &mut display,
        blob,
        frame_buffer,
        chip_probe::probe,
        physical_led::publish,
        boot,
        |tick, transport| {
            if tick.reboot {
                // Give the queued `Bye` a bounded moment to reach the host, then reboot. The
                // recovery hold must work with no host at all, so nothing here waits for one.
                let deadline = clock.now_ms().saturating_add(20);
                while transport.pending() > 0 && clock.now_ms() < deadline {
                    let _ = transport.pump();
                }
                // The HAL's full-chip reset (watchdog + PSM): it also resets USB, so the host sees
                // the device re-enumerate, and the next boot reads the reason as a forced reset.
                rp2040_hal::reset();
            }
        },
    );
}

/// Dummy chip-select pin for the verified physical panel, which has no CS line.
struct NoChipSelect;

impl ErrorType for NoChipSelect {
    type Error = Infallible;
}

impl OutputPin for NoChipSelect {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
