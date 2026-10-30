//! The ESP32-C3 cycle model: what each instruction costs, and the flash-cache penalty.
//!
//! The ESP32-C3 core is a single-issue, in-order, four-stage RV32IMC pipeline at up to 160 MHz
//! (ESP32-C3 datasheet). Espressif does not publish per-instruction latencies, so every number below
//! is an ASSUMPTION taken from the behaviour of comparable small in-order RV32IMC cores (Ibex,
//! PicoRV32-class) and then checked against two measurements on a real unit (see `docs/virtual-pip.md`).
//! Only `scale_milli` is fitted to hardware; the rest are fixed.
use crate::cache::CacheGeometry;
use serde::{Deserialize, Serialize};

/// Bumped whenever a cost rule changes, so a fixture records which model produced it.
pub const MODEL_VERSION: u32 = 1;

/// CPU clock of the unit the model is calibrated on.
pub const CPU_HZ: u64 = 160_000_000;

/// Cycle costs; all counts are CPU cycles beyond the 1 cycle every instruction takes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct CycleModel {
    /// A load result is used by the very next instruction: one bubble (load data arrives a stage
    /// later than ALU results in a 4-stage pipeline).
    pub load_use_stall: u32,
    /// Taken conditional branch: the fetch stream is redirected in EX, two instructions are flushed
    /// (no branch predictor assumed).
    pub branch_taken: u32,
    /// `jal`: target known at decode, one bubble.
    pub jal: u32,
    /// `jalr`: target known in EX, two bubbles.
    pub jalr: u32,
    /// `mul`, `mulh*`: multi-cycle iterative/pipelined multiplier assumed at 3 cycles total (Ibex
    /// "slow" multiplier), i.e. 2 beyond the base.
    pub mul: u32,
    /// `div*`, `rem*`: serial divider, assumed a flat 35 cycles total (33-bit radix-2 plus handoff;
    /// Ibex-class), i.e. 34 beyond the base. Division by zero finishes early in real cores; kept flat.
    pub div: u32,
    /// A load that hits the flash cache (DROM): the cache is read in a second stage.
    pub flash_load_hit: u32,
    /// Global factor on every non-miss cycle, in thousandths. THE FITTED PARAMETER (one number,
    /// fitted to the unit's 79 ms compose): the real core spends about twice the cycles the rules
    /// above add up to (flash-cache instruction path, SRAM bus arbitration and anything not modelled
    /// individually). It scales cycles uniformly, so it cannot reorder two costs.
    pub scale_milli: u32,
    /// Extra stall while a flash-cache line (instruction or data) is fetched over SPI. Assumed, not
    /// fitted: 32-byte line at 80 MHz quad SPI is about 84 flash clocks, 170 CPU cycles. A 40 MHz
    /// dual-SPI flash would be about 600; the validation shows this moves the old-renderer
    /// prediction by 18 points (see `docs/virtual-pip.md`), the 79 ms fit absorbing the rest.
    pub miss_penalty: u32,
    /// Flash cache geometry.
    pub cache: CacheGeometry,
}

impl CycleModel {
    /// Model version 1 with the miss penalty fitted on the unit (see `docs/virtual-pip.md`).
    pub fn v1() -> Self {
        CycleModel {
            load_use_stall: 1,
            branch_taken: 2,
            jal: 1,
            jalr: 2,
            mul: 2,
            div: 34,
            flash_load_hit: 1,
            scale_milli: FITTED_SCALE_MILLI,
            miss_penalty: 170,
            cache: CacheGeometry::C3,
        }
    }
}

/// Fitted by `frame-cost calibrate` against 79 ms on the unit; recorded here and in the fixture.
pub const FITTED_SCALE_MILLI: u32 = 2064;

impl CycleModel {
    /// Cycles of a span: the modelled base cycles scaled by the global factor, plus the stalls of
    /// flash-cache misses.
    pub fn cycles(&self, base_cycles: u64, misses: u64) -> u64 {
        (base_cycles * u64::from(self.scale_milli) + 500) / 1000
            + misses * u64::from(self.miss_penalty)
    }
}
