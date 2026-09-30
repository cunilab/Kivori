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
    dma_buffers_chunk_size,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    peripherals::Peripherals,
    spi::master::{Config, Spi},
    time::Rate,
};

use kivori_assets::AssetBlob;
use kivori_model::{Capabilities, Rgb565};
use kivori_protocol::FirmwareVersion;

use mipidsi::{interface::SpiInterface, Builder};

use crate::{
    display::MipidsiSink,
    physical_rotary::PhysicalRotary,
    profile::physical_st7789 as hw,
    proto::DeviceIdentity,
    render::FRAME_PIXELS,
    runtime::{run_buffered, RuntimeConfig},
    transport::{TxBuffered, UsbJtagTransport},
};

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

/// Keep the typed `esp-hal` GPIO field selections below synchronized with the
/// numeric hardware profile.
///
/// If somebody changes a pin in `profile.rs` without updating this file, the
/// physical firmware build fails instead of silently using the wrong wiring.
const _: () = {
    assert!(hw::SCK == 6);
    assert!(hw::MOSI == 7);
    assert!(hw::DC == 2);
    assert!(hw::RST == 3);
    assert!(hw::BL == 8);
    assert!(hw::CS.is_none());
    assert!(hw::ROTARY.clk == 4);
    assert!(hw::ROTARY.dt == 5);
    assert!(hw::ROTARY.sw == 10);
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

    // -------------------------------------------------------------------------
    // USB Serial/JTAG transport
    // -------------------------------------------------------------------------

    let serial = UsbJtagTransport::new(peripherals.USB_DEVICE);
    let mut transport = TxBuffered::new(serial);

    // -------------------------------------------------------------------------
    // SPI2
    //
    // Verified physical wiring:
    //   SCK  -> GPIO6
    //   MOSI -> GPIO7
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
    .with_sck(peripherals.GPIO6)
    .with_mosi(peripherals.GPIO7);

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
    // Verified physical wiring:
    //   DC  -> GPIO2
    //   RST -> GPIO3
    //   BL  -> GPIO8
    // -------------------------------------------------------------------------

    let dc = Output::new(peripherals.GPIO2, Level::High, OutputConfig::default());

    let rst = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());

    // Keep the verified backlight enabled for the lifetime of the runtime.
    let _backlight = Output::new(
        peripherals.GPIO8,
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

    let mut display = MipidsiSink::new(display, hw::geometry());

    // -------------------------------------------------------------------------
    // Rotary encoder input
    //
    // Wiring specification (unverified on physical hardware; see
    // `profile::physical_st7789::ROTARY`):
    //   CLK -> GPIO4
    //   DT  -> GPIO5
    //   SW  -> GPIO10
    //
    // All three lines are active-low (HW-040 COM to GND), so they are read with
    // internal pull-ups. `PhysicalRotary` only reads and inverts pin levels; all
    // conditioning and semantics live above the port.
    // -------------------------------------------------------------------------

    let rotary_pull = InputConfig::default().with_pull(Pull::Up);
    let rotary_clk = Input::new(peripherals.GPIO4, rotary_pull);
    let rotary_dt = Input::new(peripherals.GPIO5, rotary_pull);
    let rotary_sw = Input::new(peripherals.GPIO10, rotary_pull);
    let mut rotary = PhysicalRotary::new(rotary_clk, rotary_dt, rotary_sw);

    // -------------------------------------------------------------------------
    // Compiled Kivori assets
    // -------------------------------------------------------------------------

    let blob = AssetBlob::parse(assets).expect("Kivori asset blob");

    esp_println::println!("KIVORI assets loaded");

    // -------------------------------------------------------------------------
    // Device identity
    // -------------------------------------------------------------------------

    let identity = DeviceIdentity {
        device_id: [
            0x4B, 0x49, 0x56, 0x4F, 0x52, 0x49, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01,
        ],

        firmware_version: FirmwareVersion {
            major: 1,
            minor: 0,
            patch: 0,
        },

        capabilities: Capabilities::MASCOT_INTERACTION
            .union(Capabilities::PHYSICAL_INPUT_V1)
            .union(Capabilities::PRESENTATION_V1),
    };

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
        |_tick, _transport| {},
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
