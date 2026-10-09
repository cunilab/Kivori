//! SPI `DisplaySink` adapter via `mipidsi` (T072; research R-3).
//!
//! Implements the [`DisplaySink`] port on top of a `mipidsi` [`Display`], turning each change-driven
//! tile from [`crate::render::TileRenderer`] into one **windowed** `set_pixels` write — an address-window
//! command pair followed by exactly `w * h` RGB565 pixels. No framebuffer is held here (an ESP32-C3
//! cannot spare 115 KB); tiles stream straight over SPI.
//!
//! # Deliberately controller-agnostic
//!
//! This module never names a panel controller. It is generic over `mipidsi`'s [`Model`], so the same
//! adapter drives an ST7789, a GC9A01, or (in simulation only) an ILI9341, and the choice stays a
//! [`kivori_model::DeviceProfile`] parameter exactly as R-3 requires. The Kivori panel is a verified ST7789
//! (validation.md 1.23), but that choice lives in the board profile, so no controller is hard-coded and
//! no initialisation sequence is asserted in this module.
//!
//! # Deliberately hardware-neutral
//!
//! Nothing here depends on `esp-hal`: the adapter speaks only `embedded-hal` (`SpiDevice`, `OutputPin`,
//! `DelayNs`) through `mipidsi`. Consequences: it compiles for the host as well as `riscv32imc`, it is
//! unit-testable against a recording SPI device (see `tests/display_spi.rs`), and board bring-up (which
//! GPIO, which SPI peripheral, what clock) lives entirely outside it.
//!
//! # What this adapter does not decide
//!
//! Panel offsets are a **panel fact**, not a code default. [`PanelGeometry`] therefore takes the offset
//! explicitly — there is no `Default` — so an unknown offset cannot silently become `(0, 0)`.
//!
//! # Errors are never swallowed
//!
//! Every failed SPI write propagates as [`DisplayError::Interface`]. Geometry mistakes are rejected
//! *before* touching the bus: `mipidsi::Display::set_pixels` documents that it performs **no** bounds
//! checking and wraps around when handed the wrong pixel count, so this adapter checks both.

use crate::ports::DisplaySink;
use crate::render::{TILE_H, TILE_PIXELS, TILE_W};
use embedded_graphics_core::pixelcolor::raw::RawU16;
use embedded_graphics_core::pixelcolor::Rgb565 as EgRgb565;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as PinErrorType, OutputPin};
use kivori_model::{DeviceProfile, Rect, Rgb565};
use mipidsi::interface::{Interface, InterfacePixelFormat};
use mipidsi::models::Model;
use mipidsi::{Builder, Display, InitError};

/// The visible panel window inside the controller's framebuffer.
///
/// `offset_x`/`offset_y` are the controller-frame origin of the visible area. They are a property of the
/// physical panel+controller pair (many 240x240 modules are offset inside a 240x320 or 132x162 frame) and
/// there is intentionally no default: pass what the panel's datasheet says, or `(0, 0)` only when that is
/// a verified fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelGeometry {
    /// Visible width in pixels.
    pub width: u16,
    /// Visible height in pixels.
    pub height: u16,
    /// Visible-area x origin inside the controller framebuffer.
    pub offset_x: u16,
    /// Visible-area y origin inside the controller framebuffer.
    pub offset_y: u16,
}

impl PanelGeometry {
    /// Geometry from an explicit size and controller-frame offset.
    #[must_use]
    pub const fn new(width: u16, height: u16, offset_x: u16, offset_y: u16) -> Self {
        Self {
            width,
            height,
            offset_x,
            offset_y,
        }
    }

    /// Geometry taking the resolution from `profile` and the offset from the caller.
    ///
    /// The profile carries resolution and controller *choice*; it deliberately does not carry an offset,
    /// because that is a panel measurement rather than a rendering parameter.
    #[must_use]
    pub const fn from_profile(profile: &DeviceProfile, offset_x: u16, offset_y: u16) -> Self {
        Self::new(profile.width, profile.height, offset_x, offset_y)
    }
}

/// Why a tile could not be blitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayError<E> {
    /// The tile rectangle is empty, or extends past the visible panel area.
    TileOutOfBounds,
    /// `pixels.len()` did not equal `rect.w * rect.h`, so the window would under- or over-run.
    PixelCountMismatch,
    /// The display interface (SPI write or D/C toggle) failed.
    Interface(E),
}

/// A [`DisplaySink`] that writes tiles to a `mipidsi` display as windowed RGB565 transfers.
pub struct MipidsiSink<DI, M, RST>
where
    DI: Interface,
    M: Model<ColorFormat = EgRgb565>,
    EgRgb565: InterfacePixelFormat<DI::Word>,
    RST: OutputPin,
{
    display: Display<DI, M, RST>,
    geometry: PanelGeometry,
}

impl<DI, M, RST> MipidsiSink<DI, M, RST>
where
    DI: Interface,
    M: Model<ColorFormat = EgRgb565>,
    EgRgb565: InterfacePixelFormat<DI::Word>,
    RST: OutputPin,
{
    /// Wraps an already-initialised `mipidsi` display.
    ///
    /// `geometry` must match the `display_size`/`display_offset` the display was built with; it is what
    /// bounds checks are performed against.
    pub const fn new(display: Display<DI, M, RST>, geometry: PanelGeometry) -> Self {
        Self { display, geometry }
    }

    /// The geometry this sink validates tiles against.
    #[must_use]
    pub const fn geometry(&self) -> PanelGeometry {
        self.geometry
    }

    /// Borrows the underlying display (orientation, sleep/wake, tearing effect).
    pub fn display_mut(&mut self) -> &mut Display<DI, M, RST> {
        &mut self.display
    }

    /// Releases the display back to the caller.
    pub fn into_display(self) -> Display<DI, M, RST> {
        self.display
    }
}

/// The result of [`init_panel`]: a ready sink, or the reason the panel could not be brought up.
pub type InitResult<DI, M, RST> = Result<
    MipidsiSink<DI, M, RST>,
    InitError<<DI as Interface>::Error, <RST as PinErrorType>::Error>,
>;

/// Builds and initialises a panel, then wraps it as a [`MipidsiSink`].
///
/// `model` is supplied by the caller, so this function commits to no controller. The reset pin is driven
/// by `mipidsi` during init (low, then high) — a hardware reset the caller does not have to sequence.
///
/// # Errors
/// [`InitError`] if the reset pin fails, the interface fails, or `geometry` does not fit inside the
/// model's framebuffer.
pub fn init_panel<DI, M, RST, D>(
    model: M,
    interface: DI,
    reset: RST,
    delay: &mut D,
    geometry: PanelGeometry,
) -> InitResult<DI, M, RST>
where
    DI: Interface,
    M: Model<ColorFormat = EgRgb565>,
    EgRgb565: InterfacePixelFormat<DI::Word>,
    RST: OutputPin,
    D: DelayNs,
{
    let display = Builder::new(model, interface)
        .display_size(geometry.width, geometry.height)
        .display_offset(geometry.offset_x, geometry.offset_y)
        .reset_pin(reset)
        .init(delay)?;
    Ok(MipidsiSink::new(display, geometry))
}

impl<DI, M, RST> DisplaySink for MipidsiSink<DI, M, RST>
where
    DI: Interface,
    M: Model<ColorFormat = EgRgb565>,
    EgRgb565: InterfacePixelFormat<DI::Word>,
    RST: OutputPin,
{
    type Error = DisplayError<DI::Error>;

    fn blit_tile(&mut self, rect: Rect, pixels: &[Rgb565]) -> Result<(), Self::Error> {
        if rect.is_empty()
            || rect.right() > u32::from(self.geometry.width)
            || rect.bottom() > u32::from(self.geometry.height)
        {
            return Err(DisplayError::TileOutOfBounds);
        }
        if pixels.len() as u32 != rect.area() {
            return Err(DisplayError::PixelCountMismatch);
        }
        // Inclusive end coordinates, per `set_pixels`; the emptiness check above makes `- 1` safe.
        let ex = rect.x + rect.w - 1;
        let ey = rect.y + rect.h - 1;
        self.display
            .set_pixels(
                rect.x,
                rect.y,
                ex,
                ey,
                pixels.iter().map(|c| EgRgb565::from(RawU16::new(c.raw()))),
            )
            .map_err(DisplayError::Interface)
    }

    /// One window for the whole block. `set_pixels` only needs an iterator, and the interface
    /// fills its SPI buffer from it and writes whenever that is full, so a large window costs more
    /// buffer-sized transfers, not more memory or more commands.
    fn blit_tiles(&mut self, rect: Rect, tiles: &[Rgb565], cols: u16) -> Result<(), Self::Error> {
        let tile_rows = usize::from(rect.h / TILE_H);
        let cols = usize::from(cols);
        if rect.is_empty()
            || rect.right() > u32::from(self.geometry.width)
            || rect.bottom() > u32::from(self.geometry.height)
        {
            return Err(DisplayError::TileOutOfBounds);
        }
        if cols == 0
            || rect.w != cols as u16 * TILE_W
            || rect.h != tile_rows as u16 * TILE_H
            || tiles.len() as u32 != rect.area()
        {
            return Err(DisplayError::PixelCountMismatch);
        }
        let (ex, ey) = (rect.x + rect.w - 1, rect.y + rect.h - 1);
        // Window order: for each pixel row, that row of every tile across the block.
        let pixels = (0..tile_rows).flat_map(move |tr| {
            (0..usize::from(TILE_H)).flat_map(move |y| {
                (0..cols).flat_map(move |tc| {
                    let start = (tr * cols + tc) * TILE_PIXELS + y * usize::from(TILE_W);
                    tiles[start..start + usize::from(TILE_W)].iter()
                })
            })
        });
        self.display
            .set_pixels(
                rect.x,
                rect.y,
                ex,
                ey,
                pixels.map(|c| EgRgb565::from(RawU16::new(c.raw()))),
            )
            .map_err(DisplayError::Interface)
    }
}
