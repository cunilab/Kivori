//! The virtual Pip's chip probes: the facts a board reads from itself, taken from a committed
//! resource fixture instead (docs/virtual-pip.md, "Modelled timing and internals").
//!
//! v1 (ESP32-C3) has frozen historical facts from the unit's last C3 build. v2 (RP2040-Zero) has a
//! fixture generated from `scripts/firmware-budget.mjs --json` on the product ELF and kept current
//! by `scripts/virtual-resource-facts.mjs --check`. Temperature, reset reason and the live free-RAM
//! figure have no model and stay unavailable, never 0.
use kivori_model::internals::{ChipModel, DeviceInternals};
use serde::Deserialize;

const C3_FACTS_JSON: &str = include_str!("../fixtures/resource-facts.json");
const RP2040_FACTS_JSON: &str = include_str!("../fixtures/resource-facts-rp2040.json");

#[derive(Deserialize)]
struct Facts {
    elf: ElfFacts,
    board: BoardFacts,
}

#[derive(Deserialize)]
struct ElfFacts {
    app_image_bytes: u32,
    asset_blob_bytes: u32,
    static_ram_bytes: u32,
    stack_region_bytes: u32,
    stack_peak_bytes: u32,
}

#[derive(Deserialize)]
struct BoardFacts {
    app_partition_bytes: u32,
    flash_total_bytes: u32,
}

/// Silicon revision of the unit the model is calibrated on (ESP32-C3 v0.4).
const CHIP_REVISION: (u8, u8) = (0, 4);

fn facts(json: &str) -> Facts {
    serde_json::from_str(json).expect("committed resource-facts fixture")
}

/// The [`kivori_firmware::internals::ChipProbe`] of the v1 virtual Pip (ESP32-C3).
pub fn probe_c3(out: &mut DeviceInternals) {
    fill(out, C3_FACTS_JSON, ChipModel::Esp32C3, Some(CHIP_REVISION));
}

/// The [`kivori_firmware::internals::ChipProbe`] of the v2 virtual Pip (RP2040). The product reads no
/// silicon revision from the chip, so none is reported.
pub fn probe_rp2040(out: &mut DeviceInternals) {
    fill(out, RP2040_FACTS_JSON, ChipModel::Rp2040, None);
}

fn fill(out: &mut DeviceInternals, json: &str, chip: ChipModel, revision: Option<(u8, u8)>) {
    let Facts { elf, board } = facts(json);
    out.app_image_bytes = Some(elf.app_image_bytes);
    out.app_partition_bytes = Some(board.app_partition_bytes);
    out.asset_blob_bytes = Some(elf.asset_blob_bytes);
    out.flash_total_bytes = Some(board.flash_total_bytes);
    out.ram_static_bytes = Some(elf.static_ram_bytes);
    out.ram_total_bytes = Some(elf.static_ram_bytes + elf.stack_region_bytes);
    out.stack_free_min_bytes = Some(elf.stack_region_bytes.saturating_sub(elf.stack_peak_bytes));
    out.chip_model = Some(chip);
    out.chip_revision = revision;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_c3_fixture_fills_the_flash_ram_stack_and_chip_fields() {
        let mut out = DeviceInternals::default();
        probe_c3(&mut out);
        assert_eq!(out.app_image_bytes, Some(343_794));
        assert_eq!(out.app_partition_bytes, Some(0x3F_0000));
        assert_eq!(out.flash_total_bytes, Some(4 * 1024 * 1024));
        assert_eq!(out.ram_static_bytes, Some(158_332));
        assert_eq!(out.ram_total_bytes, Some(158_332 + 162_180));
        // The panel shows the stack peak as region minus this: the estimated 115,744 B.
        assert_eq!(out.stack_free_min_bytes, Some(162_180 - 115_744));
        assert_eq!(out.chip_model, Some(ChipModel::Esp32C3));
        assert_eq!(out.chip_revision, Some((0, 4)));
        assert_eq!(out.temperature_decicelsius, None);
        assert_eq!(out.reset_reason, None);
        assert_eq!(out.ram_free_bytes, None);
    }

    #[test]
    fn the_rp2040_fixture_fills_the_flash_ram_stack_and_chip_fields() {
        let mut out = DeviceInternals::default();
        probe_rp2040(&mut out);
        let elf = facts(RP2040_FACTS_JSON).elf;
        assert_eq!(out.app_image_bytes, Some(elf.app_image_bytes));
        // One 2 MiB flash and no partition table: the whole flash is the application's.
        assert_eq!(out.app_partition_bytes, Some(2 * 1024 * 1024));
        assert_eq!(out.flash_total_bytes, Some(2 * 1024 * 1024));
        // The RP2040 has 264 KiB of SRAM, all of it statics plus the stack region.
        assert_eq!(out.ram_total_bytes, Some(264 * 1024));
        assert_eq!(
            out.stack_free_min_bytes,
            Some(elf.stack_region_bytes - elf.stack_peak_bytes)
        );
        assert_eq!(out.chip_model, Some(ChipModel::Rp2040));
        assert_eq!(out.chip_revision, None);
        assert_eq!(out.temperature_decicelsius, None);
        assert_eq!(out.reset_reason, None);
        assert_eq!(out.ram_free_bytes, None);
    }

    #[test]
    fn the_rp2040_fixture_blob_size_is_the_blob_this_simulator_runs_on() {
        // An independent freshness check of the v2 fixture: the simulator compiles the same default
        // blob that the product firmware embeds. (The v1 fixture is frozen history.)
        let blob = include_bytes!(concat!(env!("OUT_DIR"), "/pip.assets"));
        assert_eq!(
            facts(RP2040_FACTS_JSON).elf.asset_blob_bytes as usize,
            blob.len()
        );
    }
}
