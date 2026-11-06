//! The board the virtual Pip models. v1 is the ESP32-C3 SuperMini the first units shipped on (the
//! default, behaving as before); v2 is the Waveshare RP2040-Zero the product builds for since #179
//! (opt-in). Chosen per process with `KIVORI_VIRTUAL_BOARD` (docs/virtual-pip.md, "Virtual boards").
//! The firmware library is compiled for one board only, so the reported identity is overridden here
//! at runtime, on the same `BootCheck` seam the identity faults use.
use kivori_firmware::{identity::ProductIdentity, internals::ChipProbe};

use crate::probe;

/// The environment variable that selects the board; unset means v1.
pub const ENV: &str = "KIVORI_VIRTUAL_BOARD";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Board {
    /// v1: `c3-supermini`, ESP32-C3.
    C3Supermini,
    /// v2: `rp2040-zero`, RP2040.
    Rp2040Zero,
}

impl Board {
    /// The board named by `id` (its id in `products/registry.toml`).
    pub fn from_id(id: &str) -> Result<Self, String> {
        match id {
            "c3-supermini" => Ok(Self::C3Supermini),
            "rp2040-zero" => Ok(Self::Rp2040Zero),
            _ => Err(format!(
                "unknown {ENV} {id:?}; expected c3-supermini or rp2040-zero"
            )),
        }
    }

    /// The board selected by the environment, v1 when unset.
    pub fn selected() -> Result<Self, String> {
        match std::env::var(ENV) {
            Ok(id) => Self::from_id(&id),
            Err(std::env::VarError::NotPresent) => Ok(Self::C3Supermini),
            Err(error) => Err(format!("{ENV}: {error}")),
        }
    }

    /// The board's wire number, as `products/registry.toml` hands it out (a test pins both).
    pub const fn number(self) -> u16 {
        match self {
            Self::C3Supermini => 1,
            Self::Rp2040Zero => 2,
        }
    }

    /// The build identity with this board's number: what the virtual device reports as its own.
    pub fn identity(self) -> ProductIdentity {
        ProductIdentity {
            board: self.number(),
            ..ProductIdentity::BUILD
        }
    }

    /// The chip facts the Device internals report.
    pub fn probe(self) -> ChipProbe {
        match self {
            Self::C3Supermini => probe::probe_c3,
            Self::Rp2040Zero => probe::probe_rp2040,
        }
    }

    /// Multiplies the committed frame-cost fixture's per-row compose time, in thousandths. The
    /// fixture is the C3's (RV32IMC at 160 MHz, fitted to the unit); v1 uses it as is.
    ///
    /// ESTIMATE for v2, to be replaced by a measurement on the RP2040-Zero (issue #179, hardware
    /// step). No instruction-set model of the Cortex-M0+ exists, so the figure is reasoned, not
    /// fitted: the clock is 125 MHz against 160 MHz (x1.28) and ARMv6-M Thumb code runs about 1.3x
    /// the instructions of RV32IMC (two-operand ALU, eight low registers, no 64-bit support),
    /// 1.28 x 1.3 = 1.66, rounded to 1.7. It assumes the C3 fit's flash-cache stall share carries
    /// over (both run from 16 KiB-cached XIP flash) and ignores the RP2040's faster SIO divider.
    pub const fn compose_scale_permille(self) -> u64 {
        match self {
            Self::C3Supermini => 1_000,
            Self::Rp2040Zero => 1_700,
        }
    }

    /// The SPI1 clock the panel really gets, in hertz. The firmware requests the profile's
    /// 20 MHz on both boards; `rp2040-hal` picks the highest rate not above the request that
    /// `clk_peri / (even prescaler x (1 + divider))` allows, which at 125 MHz is 125 / 8 = 15.625 MHz.
    ///
    /// ESTIMATE for v2 until the real clock is read on the RP2040-Zero (issue #179, hardware step).
    pub const fn spi_hz(self) -> u32 {
        match self {
            Self::C3Supermini => 20_000_000,
            Self::Rp2040Zero => 15_625_000,
        }
    }

    /// CPU time spent per pixel before its bytes reach the panel, in nanoseconds. v1 charges none:
    /// its flush is wire time only (DMA).
    ///
    /// ESTIMATE for v2, to be replaced by a measurement on the RP2040-Zero (issue #179, hardware
    /// step). The RP2040 build has no DMA: `mipidsi` converts each RGB565 pixel into a 4096-byte
    /// staging buffer and then writes it to SPI1 with a blocking transfer. About 12 cycles a pixel
    /// at 125 MHz is 96 ns, rounded to 100 ns: 5.8 ms for a full 240 x 240 frame, on top of its
    /// 59 ms of wire time at 15.625 MHz. Feeding the SPI FIFO adds nothing: the CPU needs about 15
    /// cycles a byte and the wire 64.
    pub const fn staging_ns_per_pixel(self) -> u64 {
        match self {
            Self::C3Supermini => 0,
            Self::Rp2040Zero => 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_are_the_registrys() {
        let registry = include_str!("../../../../products/registry.toml");
        assert!(registry.contains("c3-supermini = 1"));
        assert!(registry.contains("rp2040-zero = 2"));
        assert_eq!(
            Board::Rp2040Zero.number(),
            kivori_firmware::identity::BUILD_BOARD
        );
    }

    #[test]
    fn ids_select_boards_and_anything_else_is_refused() {
        assert_eq!(Board::from_id("c3-supermini"), Ok(Board::C3Supermini));
        assert_eq!(Board::from_id("rp2040-zero"), Ok(Board::Rp2040Zero));
        assert!(Board::from_id("v2").is_err());
        assert_eq!(Board::C3Supermini.identity().board, 1);
        assert_eq!(
            Board::Rp2040Zero.identity().product,
            ProductIdentity::BUILD.product
        );
    }
}
