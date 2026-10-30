//! ESP32-C3 frame-cost model: RV32IMC instruction-set simulator, flash-cache model and a per-row
//! cost report for the real renderer compiled for `riscv32imc` (`bench/`).
pub mod cache;
pub mod cpu;
pub mod elf;
pub mod model;
pub mod report;
pub mod rvc;
