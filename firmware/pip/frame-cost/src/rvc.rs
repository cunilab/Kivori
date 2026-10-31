//! RV32C: expands a 16-bit compressed instruction to its 32-bit equivalent (RV32IMC has no
//! floating-point compressed forms, so those quadrants are illegal).

fn bits(x: u32, hi: u32, lo: u32) -> u32 {
    (x >> lo) & ((1 << (hi - lo + 1)) - 1)
}

fn sext(value: u32, width: u32) -> i32 {
    ((value << (32 - width)) as i32) >> (32 - width)
}

fn i_type(op: u32, rd: u32, f3: u32, rs1: u32, imm: i32) -> u32 {
    ((imm as u32 & 0xfff) << 20) | (rs1 << 15) | (f3 << 12) | (rd << 7) | op
}

fn s_type(op: u32, f3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    (bits(imm, 11, 5) << 25) | (rs2 << 20) | (rs1 << 15) | (f3 << 12) | (bits(imm, 4, 0) << 7) | op
}

fn b_type(f3: u32, rs1: u32, rs2: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    (bits(imm, 12, 12) << 31)
        | (bits(imm, 10, 5) << 25)
        | (rs2 << 20)
        | (rs1 << 15)
        | (f3 << 12)
        | (bits(imm, 4, 1) << 8)
        | (bits(imm, 11, 11) << 7)
        | 0x63
}

fn j_type(rd: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    (bits(imm, 20, 20) << 31)
        | (bits(imm, 10, 1) << 21)
        | (bits(imm, 11, 11) << 20)
        | (bits(imm, 19, 12) << 12)
        | (rd << 7)
        | 0x6f
}

fn r_type(f7: u32, rs2: u32, rs1: u32, f3: u32, rd: u32) -> u32 {
    (f7 << 25) | (rs2 << 20) | (rs1 << 15) | (f3 << 12) | (rd << 7) | 0x33
}

/// The CJ-format jump offset (`c.j`, `c.jal`).
fn cj_imm(h: u32) -> i32 {
    let v = (bits(h, 12, 12) << 11)
        | (bits(h, 11, 11) << 4)
        | (bits(h, 10, 9) << 8)
        | (bits(h, 8, 8) << 10)
        | (bits(h, 7, 7) << 6)
        | (bits(h, 6, 6) << 7)
        | (bits(h, 5, 3) << 1)
        | (bits(h, 2, 2) << 5);
    sext(v, 12)
}

/// Expands compressed instruction `half`; `None` if it is illegal.
pub fn expand(half: u16) -> Option<u32> {
    let h = u32::from(half);
    if h == 0 {
        return None;
    }
    let rd_full = bits(h, 11, 7);
    let rs2_full = bits(h, 6, 2);
    let rd_p = 8 + bits(h, 4, 2);
    let rs1_p = 8 + bits(h, 9, 7);
    let imm6 = sext((bits(h, 12, 12) << 5) | bits(h, 6, 2), 6);
    match (h & 3, bits(h, 15, 13)) {
        (0, 0) => {
            let imm = (bits(h, 10, 7) << 6)
                | (bits(h, 12, 11) << 4)
                | (bits(h, 5, 5) << 3)
                | (bits(h, 6, 6) << 2);
            (imm != 0).then(|| i_type(0x13, rd_p, 0, 2, imm as i32))
        }
        (0, 2 | 6) => {
            let imm = ((bits(h, 12, 10) << 3) | (bits(h, 6, 6) << 2) | (bits(h, 5, 5) << 6)) as i32;
            Some(if bits(h, 15, 13) == 2 {
                i_type(0x03, rd_p, 2, rs1_p, imm)
            } else {
                s_type(0x23, 2, rs1_p, rd_p, imm)
            })
        }
        (1, 0) => Some(i_type(0x13, rd_full, 0, rd_full, imm6)),
        (1, 1) => Some(j_type(1, cj_imm(h))),
        (1, 2) => Some(i_type(0x13, rd_full, 0, 0, imm6)),
        (1, 3) => {
            if rd_full == 2 {
                let v = (bits(h, 12, 12) << 9)
                    | (bits(h, 6, 6) << 4)
                    | (bits(h, 5, 5) << 6)
                    | (bits(h, 4, 3) << 7)
                    | (bits(h, 2, 2) << 5);
                (v != 0).then(|| i_type(0x13, 2, 0, 2, sext(v, 10)))
            } else if imm6 != 0 && rd_full != 0 {
                Some((((imm6 as u32) & 0xfffff) << 12) | (rd_full << 7) | 0x37)
            } else {
                None
            }
        }
        (1, 4) => {
            let shamt = bits(h, 6, 2);
            match bits(h, 11, 10) {
                0 | 1 if bits(h, 12, 12) == 0 => {
                    let f7 = if bits(h, 11, 10) == 1 { 0x20 } else { 0 };
                    Some(
                        (f7 << 25)
                            | (shamt << 20)
                            | (rs1_p << 15)
                            | (5 << 12)
                            | (rs1_p << 7)
                            | 0x13,
                    )
                }
                2 => Some(i_type(0x13, rs1_p, 7, rs1_p, imm6)),
                3 if bits(h, 12, 12) == 0 => {
                    let (f7, f3) = match bits(h, 6, 5) {
                        0 => (0x20, 0),
                        1 => (0, 4),
                        2 => (0, 6),
                        _ => (0, 7),
                    };
                    Some(r_type(f7, rd_p, rs1_p, f3, rs1_p))
                }
                _ => None,
            }
        }
        (1, 5) => Some(j_type(0, cj_imm(h))),
        (1, 6 | 7) => {
            let v = (bits(h, 12, 12) << 8)
                | (bits(h, 11, 10) << 3)
                | (bits(h, 6, 5) << 6)
                | (bits(h, 4, 3) << 1)
                | (bits(h, 2, 2) << 5);
            Some(b_type(
                if bits(h, 15, 13) == 6 { 0 } else { 1 },
                rs1_p,
                0,
                sext(v, 9),
            ))
        }
        (2, 0) if bits(h, 12, 12) == 0 => {
            Some((bits(h, 6, 2) << 20) | (rd_full << 15) | (1 << 12) | (rd_full << 7) | 0x13)
        }
        (2, 2) => {
            let imm = ((bits(h, 12, 12) << 5) | (bits(h, 6, 4) << 2) | (bits(h, 3, 2) << 6)) as i32;
            (rd_full != 0).then(|| i_type(0x03, rd_full, 2, 2, imm))
        }
        (2, 4) => match (bits(h, 12, 12), rs2_full) {
            (0, 0) => (rd_full != 0).then(|| i_type(0x67, 0, 0, rd_full, 0)),
            (0, _) => Some(r_type(0, rs2_full, 0, 0, rd_full)),
            (_, 0) if rd_full == 0 => Some(0x0010_0073),
            (_, 0) => Some(i_type(0x67, 1, 0, rd_full, 0)),
            _ => Some(r_type(0, rs2_full, rd_full, 0, rd_full)),
        },
        (2, 6) => {
            let imm = ((bits(h, 12, 9) << 2) | (bits(h, 8, 7) << 6)) as i32;
            Some(s_type(0x23, 2, 2, rs2_full, imm))
        }
        _ => None,
    }
}
