//! Physical ESP32-C3 + ST7789 240x240 Kivori runtime.
//!
//! All verified physical display facts live in
//! [`crate::profile::physical_st7789`].
//!
//! `esp-hal` exposes GPIOs as typed fields (`peripherals.GPIO6`, etc.), so the
//! concrete field selections remain here. Compile-time assertions below ensure
//! they cannot silently drift from the physical profile.

use core::convert::Infallible;

use embedded_hal::digital::{ErrorType, OutputPin};
use embedded_hal_bus::spi::ExclusiveDevice;

use esp_hal::{
    delay::Delay,
    dma::{DmaRxBuf, DmaTxBuf},
    dma_buffers_chunk_size, efuse,
    gpio::{DriveMode, Input, InputConfig, Io, Level, Output, OutputConfig, Pull},
    ledc::{
        channel::{self, Channel, ChannelIFace},
        timer::{self, TimerIFace},
        LSGlobalClkSource, Ledc, LowSpeed,
    },
    peripherals::Peripherals,
    spi::master::{Config, Spi},
    time::Rate,
};

use kivori_assets::AssetBlob;
use kivori_model::desk::BRIGHTNESS_DEFAULT;
use kivori_model::{Capabilities, Rect, Rgb565};
use kivori_protocol::identity::{ChipRecord, RECORD_LEN};
use kivori_protocol::FirmwareVersion;

use mipidsi::{interface::SpiInterface, Builder};

use crate::{
    chip_probe,
    device_id::derive_device_id,
    display::MipidsiSink,
    identity::{check_boot, ProductIdentity, IMAGE_RECORD},
    physical_rotary::PhysicalRotary,
    ports::{Clock, DisplaySink},
    profile::physical_st7789 as hw,
    proto::DeviceIdentity,
    render::FRAME_PIXELS,
    runtime::{run_buffered, RuntimeConfig},
    transport::{TxBuffered, UsbJtagTransport},
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

/// Reads the chip identity record from the user eFuse block (BLOCK3, `BLOCK_USR_DATA`, 192 bits).
/// Read only: this firmware never writes eFuses; the provisioning tool does (ADR-0014). Only the
/// first [`RECORD_LEN`] bytes are the record; the rest of the block (and the custom MAC bits
/// beyond it) are not touched.
fn read_chip_record() -> kivori_protocol::identity::ChipRecordState {
    let block: [u8; 24] = efuse::read_field_le(efuse::BLOCK_USR_DATA);
    ChipRecord::decode(&block[..RECORD_LEN])
}

/// Bytes held by each DMA transfer. This fits a complete 40x40 RGB565 tile
/// (3,200 bytes) and is also the maximum mipidsi SPI batch size.
const SPI_DMA_BUFFER_BYTES: usize = 4_096;
/// ESP32-C3 DMA descriptors can carry at most 4,095 bytes, so the 4 KiB
/// transfer capacity needs two TX descriptors.
const SPI_DMA_DESCRIPTOR_BYTES: usize = 4_095;
/// `SpiDmaBus` owns an RX buffer even for write-only traffic. One byte and one
/// descriptor satisfy the HAL constructor; write operations use its separate
/// empty RX transfer and never populate this buffer.
const SPI_DMA_RX_BUFFER_BYTES: usize = 1;

/// Physical-mode staging memory lives in `.bss`, never on the ESP32-C3 stack.
static FRAME_BUFFER: static_cell::ConstStaticCell<[Rgb565; FRAME_PIXELS]> =
    static_cell::ConstStaticCell::new([Rgb565::from_raw(0); FRAME_PIXELS]);

/// The PWM timer behind the backlight channel, which borrows it for as long as the firmware runs.
static BACKLIGHT_TIMER: static_cell::StaticCell<esp_hal::ledc::timer::Timer<'static, LowSpeed>> =
    static_cell::StaticCell::new();

/// The panel sink plus its backlight PWM channel: pixels go to the panel, brightness to the pin.
struct Backlit<S> {
    panel: S,
    channel: Channel<'static, LowSpeed>,
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
        let _ = self.channel.set_duty(hw::backlight_duty_pct(step));
    }
}

/// Keep the typed `esp-hal` GPIO field selections below synchronized with the
/// numeric hardware profile.
///
/// If somebody changes a pin in `profile.rs` without updating this file, the
/// physical firmware build fails instead of silently using the wrong wiring.
const _: () = {
    assert!(hw::SCK == 4);
    assert!(hw::MOSI == 3);
    assert!(hw::DC == 1);
    assert!(hw::RST == 2);
    assert!(hw::BL == 0);
    assert!(hw::CS.is_none());
    assert!(hw::ROTARY.clk == 7);
    assert!(hw::ROTARY.dt == 6);
    assert!(hw::ROTARY.sw == 5);
    assert!(hw::CONTEXT_BUTTONS[0] == 9);
    assert!(hw::CONTEXT_BUTTONS[1] == 8);
    assert!(hw::CONTEXT_BUTTONS[2] == 10);
};

/// Runs Kivori on the verified physical ESP32-C3 + ST7789 hardware.
///
/// This function owns the USB Serial/JTAG transport, SPI peripheral, display
/// control pins, backlight, parsed asset blob, and production runtime. It never
/// returns.
pub fn run_mode(
    peripherals: Peripherals,
    clock: crate::clock::EspClock,
    assets: &'static [u8],
) -> ! {
    esp_println::println!("KIVORI physical ST7789 runtime");

    // Paint the unused stack now, while little of it is in use, so the device internals snapshot
    // can report how deep it has ever gone.
    chip_probe::paint_stack();

    // -------------------------------------------------------------------------
    // USB Serial/JTAG transport
    // -------------------------------------------------------------------------

    let serial = UsbJtagTransport::new(peripherals.USB_DEVICE);
    let mut transport = TxBuffered::new(serial);

    // -------------------------------------------------------------------------
    // SPI2
    //
    // Physical wiring (the enclosure's map, #123):
    //   SCK  -> GPIO4
    //   MOSI -> GPIO3
    //
    // No MISO is needed because the display path is write-only.
    // -------------------------------------------------------------------------

    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(Rate::from_hz(hw::SPI_CLOCK_HZ))
            .with_mode(hw::spi_mode()),
    )
    .expect("SPI config")
    .with_sck(peripherals.GPIO4)
    .with_mosi(peripherals.GPIO3);

    // The panel is write-only, so SPI DMA needs only a transmit buffer. The
    // 4 KiB transfer buffer replaces the previous 512-byte CPU-driven bursts;
    // it is large enough for a whole change-driven tile in one transaction.
    // `SpiDmaBus` retains the standard `embedded-hal` SPI bus interface that
    // mipidsi and ExclusiveDevice expect.
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers_chunk_size!(
        SPI_DMA_RX_BUFFER_BYTES,
        SPI_DMA_BUFFER_BYTES,
        SPI_DMA_DESCRIPTOR_BYTES
    );
    let dma_rx_buffer = DmaRxBuf::new(rx_descriptors, rx_buffer).expect("SPI DMA RX buffer");
    let dma_tx_buffer = DmaTxBuf::new(tx_descriptors, tx_buffer).expect("SPI DMA TX buffer");
    let spi = spi
        .with_dma(peripherals.DMA_CH0)
        .with_buffers(dma_rx_buffer, dma_tx_buffer);

    // -------------------------------------------------------------------------
    // Display control pins
    //
    // Physical wiring (the enclosure's map, #123):
    //   DC  -> GPIO1
    //   RST -> GPIO2
    //   BL  -> GPIO0
    // -------------------------------------------------------------------------

    let dc = Output::new(peripherals.GPIO1, Level::High, OutputConfig::default());

    let rst = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    // The backlight is plainly on while the panel initialises, and handed to the LEDC PWM below
    // once the panel is up.
    let backlight = Output::new(
        peripherals.GPIO0,
        hw::backlight_level(),
        OutputConfig::default(),
    );

    // -------------------------------------------------------------------------
    // SPI device
    //
    // The verified physical ST7789 module does not use a CS line, so
    // `NoChipSelect` satisfies the `SpiDevice` abstraction without toggling
    // another GPIO.
    // -------------------------------------------------------------------------

    let spi_dev = ExclusiveDevice::new(spi, NoChipSelect, Delay::new()).expect("SPI device");

    let mut interface_buffer = [0u8; SPI_DMA_BUFFER_BYTES];

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
        .init(&mut Delay::new())
        .expect("ST7789 init");

    esp_println::println!("KIVORI display initialized");

    // -------------------------------------------------------------------------
    // Kivori display sink
    //
    // Converts the initialized mipidsi display into Kivori's generic
    // DisplaySink. The renderer will send change-driven 40x40 RGB565 tiles.
    // -------------------------------------------------------------------------

    let display = MipidsiSink::new(display, hw::geometry());

    // -------------------------------------------------------------------------
    // Backlight PWM (GPIO0, LEDC low-speed channel 0)
    //
    // Starts at full brightness, exactly what the plain output gave, and follows the desktop's
    // brightness setting from then on. 24 kHz is above hearing; 8-bit duty is far finer than the
    // five steps need.
    // -------------------------------------------------------------------------

    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    let backlight_timer = BACKLIGHT_TIMER.init(ledc.timer::<LowSpeed>(timer::Number::Timer0));
    backlight_timer
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty8Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })
        .expect("backlight PWM timer");
    let mut backlight_channel = ledc.channel::<LowSpeed>(
        channel::Number::Channel0,
        backlight.into_peripheral_output(),
    );
    backlight_channel
        .configure(channel::config::Config {
            timer: &*backlight_timer,
            duty_pct: hw::backlight_duty_pct(BRIGHTNESS_DEFAULT),
            drive_mode: DriveMode::PushPull,
        })
        .expect("backlight PWM channel");
    let mut display = Backlit {
        panel: display,
        channel: backlight_channel,
    };

    // -------------------------------------------------------------------------
    // Rotary encoder input
    //
    // Wiring specification (unverified on physical hardware; see
    // `profile::physical_st7789::ROTARY`):
    //   CLK -> GPIO7
    //   DT  -> GPIO6
    //   SW  -> GPIO5
    //
    // All three lines are active-low (HW-040 COM to GND), so they are read with
    // internal pull-ups. `PhysicalRotary` only reads and inverts pin levels; all
    // conditioning and semantics live above the port.
    // -------------------------------------------------------------------------

    let rotary_pull = InputConfig::default().with_pull(Pull::Up);
    let rotary_clk = Input::new(peripherals.GPIO7, rotary_pull);
    let rotary_dt = Input::new(peripherals.GPIO6, rotary_pull);
    let rotary_sw = Input::new(peripherals.GPIO5, rotary_pull);
    // Contextual buttons: GPIO9 / GPIO8 / GPIO10 to GND, same pull-up active-low reading
    // (`profile::physical_st7789::CONTEXT_BUTTONS`).
    let keys = [
        Input::new(peripherals.GPIO9, rotary_pull),
        Input::new(peripherals.GPIO8, rotary_pull),
        Input::new(peripherals.GPIO10, rotary_pull),
    ];
    let mut io = Io::new(peripherals.IO_MUX);
    let mut rotary = PhysicalRotary::new(&mut io, rotary_clk, rotary_dt, rotary_sw, keys);

    // -------------------------------------------------------------------------
    // Compiled Kivori assets
    // -------------------------------------------------------------------------

    let blob = AssetBlob::parse(assets).expect("Kivori asset blob");
    chip_probe::init(peripherals.TSENS, assets.len());

    esp_println::println!("KIVORI assets loaded");

    // -------------------------------------------------------------------------
    // Device identity
    // -------------------------------------------------------------------------

    let identity = DeviceIdentity {
        // Stable per-unit ID: hash of the factory eFuse unique ID (base MAC fallback). The raw
        // eFuse bytes never go on the wire.
        device_id: derive_device_id(
            &efuse::read_field_le::<[u8; 16]>(efuse::OPTIONAL_UNIQUE_ID),
            &efuse::base_mac_address()
                .as_bytes()
                .try_into()
                .expect("6-byte base MAC"),
        ),

        firmware_version: FIRMWARE_VERSION,

        capabilities: Capabilities::MASCOT_INTERACTION
            .union(Capabilities::PHYSICAL_INPUT_V1)
            .union(Capabilities::PRESENTATION_V1)
            .union(Capabilities::BUTTON_INPUT_V1)
            .union(Capabilities::DESK_STATUS_V1)
            .union(Capabilities::ACTION_FEEDBACK_V1)
            .union(Capabilities::DOUBLE_PRESS_V1)
            .union(Capabilities::MEDIA_INFO_V1)
            .union(Capabilities::CONTROL_LABELS_V1)
            .union(Capabilities::CONTEXT_BUTTONS_V1)
            .union(Capabilities::HOST_TAKEOVERS_V1)
            .union(Capabilities::QUIET_ACK_V1)
            .union(Capabilities::DEVICE_INTERNALS_V1)
            .union(Capabilities::IDLE_LIFE_V1)
            .union(Capabilities::MIC_STATE_V1)
            .union(Capabilities::PRODUCT_IDENTITY_V1),
    };

    // Tier A (ADR-0016): a valid chip record for another product or board means this image must
    // not run the product. The check result also sets what the handshake reports.
    let boot = check_boot(ProductIdentity::BUILD, read_chip_record());

    // Keeps `FW_VERSION_MARKER` (and so the version text) in the linked image.
    // SAFETY: a valid, aligned read of the first byte of a live static.
    let _ = unsafe { core::ptr::read_volatile(FW_VERSION_MARKER.as_ptr()) };
    // SAFETY: as above, for the identity record.
    let _ = unsafe { core::ptr::read_volatile(FW_IDENTITY_MARKER.as_ptr()) };

    esp_println::println!("KIVORI runtime starting");

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
    // is being composed. The frame costs 115,200 bytes of ESP32-C3 SRAM.
    let frame_buffer = FRAME_BUFFER.take();

    run_buffered(
        identity,
        RuntimeConfig::default(),
        &clock,
        &mut transport,
        &mut rotary,
        &mut display,
        &blob,
        frame_buffer,
        chip_probe::probe,
        boot,
        |tick, transport| {
            if tick.reboot {
                // Give the queued `Bye` a bounded moment to reach the host, then reboot. The
                // recovery hold must work with no host at all, so nothing here waits for one.
                let deadline = clock.now_ms().saturating_add(20);
                while transport.pending() > 0 && clock.now_ms() < deadline {
                    let _ = transport.pump();
                }
                esp_hal::system::software_reset();
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
