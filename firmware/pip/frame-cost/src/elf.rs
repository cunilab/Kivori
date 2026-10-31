//! Minimal ELF32 little-endian loader: PT_LOAD segments at their virtual addresses.
use crate::cpu::Machine;

fn u16_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 2)
        .map(|s| u32::from(u16::from_le_bytes([s[0], s[1]])))
        .ok_or_else(|| "truncated ELF".to_string())
}

fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| "truncated ELF".to_string())
}

/// Loads `bytes` into `machine` and sets the entry point.
///
/// # Errors
/// A malformed ELF, or a segment outside the C3 memory map.
pub fn load(machine: &mut Machine, bytes: &[u8]) -> Result<(), String> {
    if bytes.get(..4) != Some(b"\x7fELF") || bytes.get(4) != Some(&1) || bytes.get(5) != Some(&1) {
        return Err("not a 32-bit little-endian ELF".into());
    }
    if u16_at(bytes, 18)? != 243 {
        return Err("not a RISC-V ELF".into());
    }
    machine.pc = u32_at(bytes, 24)?;
    let (phoff, phentsize, phnum) = (
        u32_at(bytes, 28)? as usize,
        u16_at(bytes, 42)? as usize,
        u16_at(bytes, 44)? as usize,
    );
    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        if u32_at(bytes, ph)? != 1 {
            continue; // PT_LOAD only
        }
        let (offset, vaddr, filesz) = (
            u32_at(bytes, ph + 4)? as usize,
            u32_at(bytes, ph + 8)?,
            u32_at(bytes, ph + 16)? as usize,
        );
        let data = bytes
            .get(offset..offset + filesz)
            .ok_or("segment outside the file")?;
        if !machine.load(vaddr, data) {
            return Err(format!(
                "segment at {vaddr:#x} is outside the ESP32-C3 memory map"
            ));
        }
    }
    Ok(())
}

/// Function symbols `(address, size, name)` from the symbol table, sorted by address.
pub fn function_symbols(bytes: &[u8]) -> Vec<(u32, u32, String)> {
    let mut out = Vec::new();
    let (Ok(shoff), Ok(shentsize), Ok(shnum)) =
        (u32_at(bytes, 32), u16_at(bytes, 46), u16_at(bytes, 48))
    else {
        return out;
    };
    for i in 0..shnum as usize {
        let sh = shoff as usize + i * shentsize as usize;
        if u32_at(bytes, sh + 4).ok() != Some(2) {
            continue; // SHT_SYMTAB
        }
        let (Ok(off), Ok(size), Ok(link)) = (
            u32_at(bytes, sh + 16),
            u32_at(bytes, sh + 20),
            u32_at(bytes, sh + 24),
        ) else {
            continue;
        };
        let strtab_sh = shoff as usize + link as usize * shentsize as usize;
        let Ok(str_off) = u32_at(bytes, strtab_sh + 16) else {
            continue;
        };
        for k in 0..(size as usize / 16) {
            let e = off as usize + k * 16;
            let (Ok(name), Ok(addr), Ok(sz)) =
                (u32_at(bytes, e), u32_at(bytes, e + 4), u32_at(bytes, e + 8))
            else {
                continue;
            };
            if bytes.get(e + 12).map(|i| i & 0xf) != Some(2) {
                continue; // STT_FUNC
            }
            let start = str_off as usize + name as usize;
            let text = bytes[start..]
                .iter()
                .take_while(|b| **b != 0)
                .map(|b| *b as char)
                .collect();
            out.push((addr, sz, text));
        }
    }
    out.sort();
    out
}
