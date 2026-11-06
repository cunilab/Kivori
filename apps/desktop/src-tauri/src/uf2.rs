//! UF2 flashing for the RP2040's ROM USB bootloader: ELF to UF2 conversion and finding the
//! bootloader's mass-storage drive. No external tool is involved; `firmware` drives the flow.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The RP2040 maps its external flash here (2 MiB window).
const FLASH: std::ops::Range<u32> = 0x1000_0000..0x1020_0000;
/// The ROM bootloader accepts exactly this payload per block.
const PAYLOAD: usize = 256;
const MAGIC_START0: u32 = 0x0A32_4655;
const MAGIC_START1: u32 = 0x9E5D_5157;
const MAGIC_END: u32 = 0x0AB1_6F30;
/// "familyID present".
const FLAG_FAMILY: u32 = 0x0000_2000;
const FAMILY_RP2040: u32 = 0xE48B_FF56;

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?).into())
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Converts an ELF32 little-endian image to UF2 for the RP2040: every `PT_LOAD` segment with file
/// bytes is placed at its load address (`p_paddr`) in flash, in 256-byte pages padded with zeros.
/// `None` for a malformed ELF, a loadable segment outside flash, or an image with no flash segment.
#[must_use]
pub fn elf_to_uf2(elf: &[u8]) -> Option<Vec<u8>> {
    let (phoff, phentsize, phnum) = (
        u32_at(elf, 0x1C)? as usize,
        u16_at(elf, 0x2A)?,
        u16_at(elf, 0x2C)?,
    );
    let mut pages: BTreeMap<u32, [u8; PAYLOAD]> = BTreeMap::new();
    for index in 0..phnum {
        let header = phoff.checked_add(index.checked_mul(phentsize)?)?;
        let (kind, offset, paddr, filesz) = (
            u32_at(elf, header)?,
            u32_at(elf, header + 4)? as usize,
            u32_at(elf, header + 12)?,
            u32_at(elf, header + 16)?,
        );
        if kind != 1 || filesz == 0 {
            continue;
        }
        if !FLASH.contains(&paddr) || paddr.checked_add(filesz)? > FLASH.end {
            return None;
        }
        let data = elf.get(offset..offset.checked_add(filesz as usize)?)?;
        for (address, byte) in (paddr..).zip(data) {
            let page = pages
                .entry(address & !(PAYLOAD as u32 - 1))
                .or_insert([0; PAYLOAD]);
            page[(address as usize) % PAYLOAD] = *byte;
        }
    }
    if pages.is_empty() {
        return None;
    }
    let total = pages.len() as u32;
    let mut out = Vec::with_capacity(pages.len() * 512);
    for (number, (address, page)) in (0u32..).zip(&pages) {
        for word in [
            MAGIC_START0,
            MAGIC_START1,
            FLAG_FAMILY,
            *address,
            PAYLOAD as u32,
            number,
            total,
            FAMILY_RP2040,
        ] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(page);
        out.resize(out.len() + (476 - PAYLOAD), 0);
        out.extend_from_slice(&MAGIC_END.to_le_bytes());
    }
    Some(out)
}

/// Whether `dir` is the RP2040 ROM bootloader drive (its `INFO_UF2.TXT` names the board id).
#[must_use]
pub fn is_bootloader_drive(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("INFO_UF2.TXT"))
        .is_ok_and(|text| text.contains("Board-ID: RPI-RP2"))
}

/// Every bootloader drive among the direct children of `roots`. Off Windows only entries named
/// `RPI-RP2*` are read: probing every volume prompts for network and removable volumes on macOS and
/// stalls on slow mounts.
fn find_in(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .filter_map(|root| std::fs::read_dir(root).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("RPI-RP2"))
        .map(|entry| entry.path())
        .filter(|path| is_bootloader_drive(path))
        .collect()
}

/// Every mounted RP2040 bootloader drive.
#[must_use]
pub fn bootloader_drives() -> Vec<PathBuf> {
    if cfg!(windows) {
        // shortcut: reads INFO_UF2.TXT on every letter; std cannot give volume labels, and a
        // label lookup needs a new dependency. Upgrade if a probe of a dead letter proves slow.
        return ('A'..='Z')
            .map(|letter| PathBuf::from(format!("{letter}:\\")))
            .filter(|path| is_bootloader_drive(path))
            .collect();
    }
    let user = std::env::var("USER").unwrap_or_default();
    find_in(&[
        "/Volumes".into(),
        format!("/media/{user}").into(),
        format!("/run/media/{user}").into(),
        "/media".into(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 32-bit little-endian ELF with the given `(paddr, bytes)` `PT_LOAD` segments.
    fn elf(segments: &[(u32, &[u8])]) -> Vec<u8> {
        let mut bytes = vec![0; 52];
        bytes[..6].copy_from_slice(b"\x7fELF\x01\x01");
        bytes[18] = 40;
        bytes[0x1C..0x20].copy_from_slice(&52u32.to_le_bytes());
        bytes[0x2A..0x2C].copy_from_slice(&32u16.to_le_bytes());
        bytes[0x2C..0x2E].copy_from_slice(&(segments.len() as u16).to_le_bytes());
        let mut offset = 52 + 32 * segments.len();
        for (paddr, data) in segments {
            let mut header = [0u8; 32];
            header[..4].copy_from_slice(&1u32.to_le_bytes());
            header[4..8].copy_from_slice(&(offset as u32).to_le_bytes());
            header[12..16].copy_from_slice(&paddr.to_le_bytes());
            header[16..20].copy_from_slice(&(data.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&header);
            offset += data.len();
        }
        for (_, data) in segments {
            bytes.extend_from_slice(data);
        }
        bytes
    }

    fn word(block: &[u8], index: usize) -> u32 {
        u32::from_le_bytes(block[index * 4..index * 4 + 4].try_into().unwrap())
    }

    #[test]
    fn pages_cover_the_flash_segments_with_numbering_family_and_padding() {
        // 300 bytes at the flash start (two pages) and 4 bytes later in a page of their own.
        let first = vec![0xAA; 300];
        let uf2 = elf_to_uf2(&elf(&[(0x1000_0000, &first), (0x1000_0400, &[1, 2, 3, 4])])).unwrap();
        assert_eq!(uf2.len(), 3 * 512);
        for (number, (block, address)) in uf2
            .chunks(512)
            .zip([0x1000_0000, 0x1000_0100, 0x1000_0400])
            .enumerate()
        {
            assert_eq!(word(block, 0), 0x0A32_4655);
            assert_eq!(word(block, 1), 0x9E5D_5157);
            assert_eq!(word(block, 2), 0x2000);
            assert_eq!(word(block, 3), address);
            assert_eq!(word(block, 4), 256);
            assert_eq!(word(block, 5), number as u32);
            assert_eq!(word(block, 6), 3);
            assert_eq!(word(block, 7), 0xE48B_FF56);
            assert_eq!(word(block, 127), 0x0AB1_6F30);
        }
        // The partial second page is zero padded after byte 44; the third starts with its data.
        assert_eq!(&uf2[512 + 32..512 + 32 + 44], &[0xAA; 44][..]);
        assert!(uf2[512 + 32 + 44..512 + 32 + 256].iter().all(|&b| b == 0));
        assert_eq!(&uf2[1024 + 32..1024 + 36], &[1, 2, 3, 4]);
    }

    #[test]
    fn segments_sharing_a_page_merge_into_one_block() {
        let uf2 = elf_to_uf2(&elf(&[(0x1000_0000, &[1; 8]), (0x1000_0010, &[2; 8])])).unwrap();
        assert_eq!(uf2.len(), 512);
        assert_eq!(&uf2[32..40], &[1; 8]);
        assert_eq!(&uf2[48..56], &[2; 8]);
    }

    #[test]
    fn images_without_a_flash_segment_or_with_one_leaving_flash_are_refused() {
        assert_eq!(elf_to_uf2(&elf(&[])), None);
        assert_eq!(elf_to_uf2(&elf(&[(0x2000_0000, &[1; 4])])), None);
        assert_eq!(elf_to_uf2(&elf(&[(0x101F_FFFC, &[1; 8])])), None);
        assert_eq!(elf_to_uf2(&elf(&[(0x1000_0000, &[])])), None);
        assert_eq!(elf_to_uf2(b"\x7fELF"), None);
    }

    #[test]
    fn only_a_drive_naming_the_rp2040_bootloader_qualifies() {
        let root = tempfile::tempdir().unwrap();
        let drive = root.path().join("RPI-RP2");
        std::fs::create_dir(&drive).unwrap();
        assert!(!is_bootloader_drive(&drive));
        std::fs::write(
            drive.join("INFO_UF2.TXT"),
            "UF2 Bootloader v3.0\nBoard-ID: OTHER\n",
        )
        .unwrap();
        assert!(!is_bootloader_drive(&drive));
        assert!(find_in(&[root.path().into()]).is_empty());
        std::fs::write(
            drive.join("INFO_UF2.TXT"),
            "UF2 Bootloader v3.0\nModel: Raspberry Pi RP2\nBoard-ID: RPI-RP2\n",
        )
        .unwrap();
        assert!(is_bootloader_drive(&drive));
        assert_eq!(
            find_in(&["/nonexistent".into(), root.path().into()]),
            vec![drive.clone()]
        );
        // A volume with another name is never read, even when it carries the bootloader's file.
        let other = root.path().join("Other");
        std::fs::create_dir(&other).unwrap();
        std::fs::copy(drive.join("INFO_UF2.TXT"), other.join("INFO_UF2.TXT")).unwrap();
        assert_eq!(find_in(&[root.path().into()]), vec![drive]);
    }
}
