//! The virtual Pip's chip probe: the facts the ESP32-C3 board reads from itself, taken from the
//! committed resource fixture instead (docs/virtual-pip.md, "Modelled timing and internals").
//!
//! The fixture is generated from `scripts/firmware-budget.mjs --json` on the product ELF and kept
//! current by `scripts/virtual-resource-facts.mjs --check`. Temperature, reset reason and the live
//! free-RAM figure have no model and stay unavailable, never 0.
use kivori_model::internals::{ChipModel, DeviceInternals};
use serde::Deserialize;

const RESOURCE_FACTS_JSON: &str = include_str!("../fixtures/resource-facts.json");

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

fn facts() -> Facts {
    serde_json::from_str(RESOURCE_FACTS_JSON).expect("committed resource-facts fixture")
}

/// The [`kivori_firmware::internals::ChipProbe`] of the virtual Pip.
pub fn probe(out: &mut DeviceInternals) {
    let Facts { elf, board } = facts();
    out.app_image_bytes = Some(elf.app_image_bytes);
    out.app_partition_bytes = Some(board.app_partition_bytes);
    out.asset_blob_bytes = Some(elf.asset_blob_bytes);
    out.flash_total_bytes = Some(board.flash_total_bytes);
    out.ram_static_bytes = Some(elf.static_ram_bytes);
    out.ram_total_bytes = Some(elf.static_ram_bytes + elf.stack_region_bytes);
    out.stack_free_min_bytes = Some(elf.stack_region_bytes.saturating_sub(elf.stack_peak_bytes));
    out.chip_model = Some(ChipModel::Esp32C3);
    out.chip_revision = Some(CHIP_REVISION);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_fills_the_flash_ram_stack_and_chip_fields() {
        let mut out = DeviceInternals::default();
        probe(&mut out);
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
    fn the_fixture_blob_size_is_the_blob_this_simulator_runs_on() {
        // An independent freshness check of the fixture: the simulator compiles the same default
        // blob that the product firmware embeds.
        let blob = include_bytes!(concat!(env!("OUT_DIR"), "/pip.assets"));
        assert_eq!(facts().elf.asset_blob_bytes as usize, blob.len());
    }
}
