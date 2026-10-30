//! Instruction semantics, compressed decode, cycle rules and the flash cache of the ISS. The programs
//! were assembled with `llvm-mc -triple=riscv32 -mattr=+m,+c`; the source is quoted above each.
use kivori_pip_frame_cost::cpu::{div_rem, mark, Machine, Stop, DROM_BASE, IROM_BASE, RAM_BASE};
use kivori_pip_frame_cost::model::CycleModel;
use kivori_pip_frame_cost::rvc;

fn machine_at(base: u32, code: &[u8]) -> Machine {
    let mut m = Machine::new(CycleModel::v1());
    assert!(m.load(base, code));
    m.pc = base;
    m
}

fn run(base: u32, code: &[u8]) -> Machine {
    let mut m = machine_at(base, code);
    m.x[2] = RAM_BASE + 0x1_0000; // sp
    assert_eq!(m.run(10_000), Stop::Ecall, "pc {:#x}", m.pc);
    m
}

// li a0,0x12345678; li a1,-5; add a2,a0,a1; sub a3,a0,a1; slli a4,a1,4; srli a5,a1,28;
// srai a6,a1,28; slt a7,a1,a0; sltu t0,a1,a0; xori t1,a0,-1; lui t2,0x80000; auipc t3,1; ecall
const ALU: &[u8] = &[
    0x37, 0x55, 0x34, 0x12, 0x13, 0x05, 0x85, 0x67, 0x93, 0x05, 0xb0, 0xff, 0x33, 0x06, 0xb5, 0x00,
    0xb3, 0x06, 0xb5, 0x40, 0x13, 0x97, 0x45, 0x00, 0x93, 0xd7, 0xc5, 0x01, 0x13, 0xd8, 0xc5, 0x41,
    0xb3, 0xa8, 0xa5, 0x00, 0xb3, 0xb2, 0xa5, 0x00, 0x13, 0x43, 0xf5, 0xff, 0xb7, 0x03, 0x00, 0x80,
    0x17, 0x1e, 0x00, 0x00, 0x73, 0x00, 0x00, 0x00,
];

#[test]
fn integer_alu_semantics() {
    let m = run(IROM_BASE, ALU);
    assert_eq!(m.x[12], 0x1234_5673); // a2
    assert_eq!(m.x[13], 0x1234_567d); // a3
    assert_eq!(m.x[14], 0xffff_ffb0); // a4 = -5 << 4
    assert_eq!(m.x[15], 0xf); // a5 logical shift
    assert_eq!(m.x[16], 0xffff_ffff); // a6 arithmetic shift
    assert_eq!(m.x[17], 1); // a7 signed less-than
    assert_eq!(m.x[5], 0); // t0 unsigned less-than
    assert_eq!(m.x[6], 0xedcb_a987); // t1
    assert_eq!(m.x[7], 0x8000_0000); // t2
    assert_eq!(m.x[28], IROM_BASE + 48 + 0x1000); // t3 = auipc
}

// li a0,7; li a1,0; div a2; divu a3; rem a4; remu a5 (by zero); li a0,0x80000000; li a1,-1;
// div a6; rem a7 (overflow); li a0,-7; li a1,2; div t0; rem t1; divu t2;
// li a0,a1,-1 (0xffffffff); mulh t3; mulhu t4; mulhsu t5; mul t6; ecall
const MULDIV: &[u8] = &[
    0x13, 0x05, 0x70, 0x00, 0x93, 0x05, 0x00, 0x00, 0x33, 0x46, 0xb5, 0x02, 0xb3, 0x56, 0xb5, 0x02,
    0x33, 0x67, 0xb5, 0x02, 0xb3, 0x77, 0xb5, 0x02, 0x37, 0x05, 0x00, 0x80, 0x93, 0x05, 0xf0, 0xff,
    0x33, 0x48, 0xb5, 0x02, 0xb3, 0x68, 0xb5, 0x02, 0x13, 0x05, 0x90, 0xff, 0x93, 0x05, 0x20, 0x00,
    0xb3, 0x42, 0xb5, 0x02, 0x33, 0x63, 0xb5, 0x02, 0xb3, 0x53, 0xb5, 0x02, 0x13, 0x05, 0xf0, 0xff,
    0x93, 0x05, 0xf0, 0xff, 0x33, 0x1e, 0xb5, 0x02, 0xb3, 0x3e, 0xb5, 0x02, 0x33, 0x2f, 0xb5, 0x02,
    0xb3, 0x0f, 0xb5, 0x02, 0x73, 0x00, 0x00, 0x00,
];

#[test]
fn multiply_and_divide_edge_cases() {
    let m = run(IROM_BASE, MULDIV);
    assert_eq!(m.x[12], u32::MAX, "div by zero is -1");
    assert_eq!(m.x[13], u32::MAX, "divu by zero is all ones");
    assert_eq!(m.x[14], 7, "rem by zero is the dividend");
    assert_eq!(m.x[15], 7, "remu by zero is the dividend");
    assert_eq!(m.x[16], 0x8000_0000, "MIN / -1 overflows to MIN");
    assert_eq!(m.x[17], 0, "MIN % -1 is 0");
    assert_eq!(m.x[5], 0xffff_fffd, "-7 / 2 truncates toward zero");
    assert_eq!(m.x[6], 0xffff_ffff, "-7 % 2 takes the dividend's sign");
    assert_eq!(m.x[7], 0x7fff_fffc, "divu treats -7 as unsigned");
    assert_eq!(m.x[28], 0, "mulh(-1,-1)");
    assert_eq!(m.x[29], 0xffff_fffe, "mulhu(max,max)");
    assert_eq!(m.x[30], 0xffff_ffff, "mulhsu(-1,max)");
    assert_eq!(m.x[31], 1, "mul(-1,-1)");
}

#[test]
fn div_rem_helper_matches_the_spec_table() {
    assert_eq!(div_rem(4, 10, 3), 3);
    assert_eq!(div_rem(6, (-10i32) as u32, 3), (-1i32) as u32);
    assert_eq!(div_rem(7, 10, 0), 10);
    assert_eq!(div_rem(5, 10, 0), u32::MAX);
}

// lui a0,0x3fc80; li a1,0x80ff; sh a1,0(a0); li a1,-2; sb a1,2(a0); lb a2,0(a0); lbu a3,0(a0);
// lh a4,0(a0); lhu a5,0(a0); lb a6,2(a0); sw a1,4(a0); lw a7,4(a0); ecall
const MEM: &[u8] = &[
    0x37, 0x05, 0xc8, 0x3f, 0xb7, 0x85, 0x00, 0x00, 0x93, 0x85, 0xf5, 0x0f, 0x23, 0x10, 0xb5, 0x00,
    0x93, 0x05, 0xe0, 0xff, 0x23, 0x01, 0xb5, 0x00, 0x03, 0x06, 0x05, 0x00, 0x83, 0x46, 0x05, 0x00,
    0x03, 0x17, 0x05, 0x00, 0x83, 0x57, 0x05, 0x00, 0x03, 0x08, 0x25, 0x00, 0x23, 0x22, 0xb5, 0x00,
    0x83, 0x28, 0x45, 0x00, 0x73, 0x00, 0x00, 0x00,
];

#[test]
fn loads_and_stores_sign_extend_correctly() {
    let m = run(IROM_BASE, MEM);
    assert_eq!(m.x[12], 0xffff_ffff, "lb of 0xff");
    assert_eq!(m.x[13], 0xff, "lbu");
    assert_eq!(m.x[14], 0xffff_80ff, "lh of 0x80ff");
    assert_eq!(m.x[15], 0x80ff, "lhu");
    assert_eq!(m.x[16], 0xffff_fffe, "lb of the byte stored at +2");
    assert_eq!(m.x[17], 0xffff_fffe, "sw/lw round trip");
}

// li a0,0; li a1,1; li a2,11; 1: add a0,a0,a1; addi a1,a1,1; bne a1,a2,1b; jal ra,2f;
// li a4,99; 2: auipc a5,0; addi a5,a5,16; jalr ra,0(a5); li a4,98; li a3,7; ecall
const BRANCH: &[u8] = &[
    0x13, 0x05, 0x00, 0x00, 0x93, 0x05, 0x10, 0x00, 0x13, 0x06, 0xb0, 0x00, 0x33, 0x05, 0xb5, 0x00,
    0x93, 0x85, 0x15, 0x00, 0xe3, 0x9c, 0xc5, 0xfe, 0xef, 0x00, 0x80, 0x00, 0x13, 0x07, 0x30, 0x06,
    0x97, 0x07, 0x00, 0x00, 0x93, 0x87, 0x07, 0x01, 0xe7, 0x80, 0x07, 0x00, 0x13, 0x07, 0x20, 0x06,
    0x93, 0x06, 0x70, 0x00, 0x73, 0x00, 0x00, 0x00,
];

#[test]
fn loops_jumps_and_links() {
    let m = run(IROM_BASE, BRANCH);
    assert_eq!(m.x[10], 55, "sum 1..=10 through a backward branch");
    assert_eq!(m.x[14], 0, "jal skipped li a4,99 and jalr skipped li a4,98");
    assert_eq!(m.x[13], 7);
    assert_eq!(m.x[1], IROM_BASE + 44, "jalr links the next instruction");
}

// lui a0,0x3fc80; c.li a1,-3; c.addi a1,5; c.mv a2,a1; c.add a2,a2; c.slli a2,3; c.sw a2,4(a0);
// c.lw a3,4(a0); c.li a4,0x1f; c.srli a4,2; c.li a5,-16; c.srai a5,2; c.andi a5,0xe; c.li s0,12;
// c.li s1,10; c.sub s0,s1; c.xor s1,s0; c.or s1,s0; c.and s1,a4; c.addi4spn a2,sp,16;
// c.addi16sp sp,-32; c.swsp a2,8(sp); c.lwsp a7,8(sp); c.beqz a1,1f; c.j 2f; 1: c.li a7,1;
// 2: c.lui t0,3; c.nop; c.jal 3f; c.li t1,5; ecall; 3: c.jr ra
const RVC: &[u8] = &[
    0x37, 0x05, 0xc8, 0x3f, 0xf5, 0x55, 0x95, 0x05, 0x2e, 0x86, 0x32, 0x96, 0x0e, 0x06, 0x50, 0xc1,
    0x54, 0x41, 0x7d, 0x47, 0x09, 0x83, 0xc1, 0x57, 0x89, 0x87, 0xb9, 0x8b, 0x31, 0x44, 0xa9, 0x44,
    0x05, 0x8c, 0xa1, 0x8c, 0xc1, 0x8c, 0xf9, 0x8c, 0x10, 0x08, 0x3d, 0x71, 0x32, 0xc4, 0xa2, 0x48,
    0x91, 0xc1, 0x11, 0xa0, 0x85, 0x48, 0x8d, 0x62, 0x01, 0x00, 0x21, 0x20, 0x15, 0x43, 0x73, 0x00,
    0x00, 0x00, 0x82, 0x80,
];

#[test]
fn compressed_instructions_execute_like_their_expansions() {
    let m = run(IROM_BASE, RVC);
    let sp0 = RAM_BASE + 0x1_0000;
    assert_eq!(m.x[11], 2, "c.li + c.addi");
    assert_eq!(m.x[13], 32, "c.sw / c.lw through a2 = (2+2)<<3");
    assert_eq!(m.x[14], 7, "c.srli");
    assert_eq!(m.x[15], 12, "c.srai then c.andi: (-16 >> 2) & 0xe");
    assert_eq!(m.x[8], 2, "c.sub");
    assert_eq!(m.x[9], 2, "c.xor, c.or, c.and chain");
    assert_eq!(m.x[2], sp0 - 32, "c.addi16sp");
    assert_eq!(
        m.x[17],
        sp0 + 16,
        "c.addi4spn value through c.swsp / c.lwsp"
    );
    assert_eq!(
        m.x[5], 0x3000,
        "c.lui; c.beqz was not taken and c.j skipped c.li a7,1"
    );
    assert_eq!(m.x[6], 5, "c.jal / c.jr returned to the next instruction");
    assert_eq!(
        m.x[1],
        IROM_BASE + 0x3c,
        "c.jal links the next halfword pair"
    );
}

#[test]
fn compressed_expansion_matches_known_encodings() {
    assert_eq!(rvc::expand(0x8082), Some(0x0000_8067)); // ret
    assert_eq!(rvc::expand(0x1141), Some(0xff01_0113)); // addi sp,sp,-16
    assert_eq!(rvc::expand(0xc606), Some(0x0011_2623)); // sw ra,12(sp)
    assert_eq!(rvc::expand(0x4501), Some(0x0000_0513)); // li a0,0
    assert_eq!(rvc::expand(0x0001), Some(0x0000_0013)); // nop
    assert_eq!(
        rvc::expand(0x0000),
        None,
        "the all-zero halfword is illegal"
    );
    assert_eq!(rvc::expand(0x2000), None, "c.fld does not exist in RV32IMC");
}

#[test]
fn illegal_instruction_and_unmapped_access_stop_the_run() {
    let mut m = machine_at(IROM_BASE, &[0, 0, 0, 0]);
    assert!(matches!(m.run(10), Stop::Illegal(pc, 0) if pc == IROM_BASE));
    // lui a0,0x10000; lw a1,0(a0)  (nothing is mapped there)
    let mut m = machine_at(IROM_BASE, &[0x37, 0x05, 0x00, 0x10, 0x83, 0x25, 0x05, 0x00]);
    assert!(matches!(m.run(10), Stop::Fault(_, 0x1000_0000)));
}

// lui a0,0x60000; li a1,5; sw a1,8(a0); li a1,0; sw a1,0x14(a0)
const MARKER: &[u8] = &[
    0x37, 0x05, 0x00, 0x60, 0x93, 0x05, 0x50, 0x00, 0x23, 0x24, 0xb5, 0x00, 0x93, 0x05, 0x00, 0x00,
    0x23, 0x2a, 0xb5, 0x00,
];

#[test]
fn marker_stores_become_events_and_are_not_program_stores() {
    let mut m = machine_at(IROM_BASE, MARKER);
    assert_eq!(m.run(100), Stop::Exit(0));
    assert_eq!(m.events.len(), 2);
    assert_eq!((m.events[0].kind, m.events[0].value), (mark::ROW_BEGIN, 5));
    assert_eq!(m.counters.stores, 0);
}

fn base_cycles(code: &[u8]) -> u64 {
    run(RAM_BASE + 0x2000, code).counters.base_cycles
}

#[test]
fn cycle_rules_load_use_branch_mul_div() {
    // lui; lw a1; add a2,a1,a1 (uses the load: +1); ecall
    assert_eq!(
        base_cycles(&[
            0x37, 0x05, 0xc8, 0x3f, 0x83, 0x25, 0x05, 0x00, 0x33, 0x86, 0xb5, 0x00, 0x73, 0, 0, 0
        ]),
        5
    );
    // the same without the dependency
    assert_eq!(
        base_cycles(&[
            0x37, 0x05, 0xc8, 0x3f, 0x83, 0x25, 0x05, 0x00, 0x33, 0x86, 0xd6, 0x00, 0x73, 0, 0, 0
        ]),
        4
    );
    // mul (3 cycles), div (35), ecall (1)
    assert_eq!(
        base_cycles(&[0x33, 0x05, 0xa5, 0x02, 0x33, 0x45, 0xa5, 0x02, 0x73, 0, 0, 0]),
        39
    );
}

// beq x0,x0,1f (taken); nop; 1: bne x0,x0,2f (not taken); jal x0,2f; nop; 2: jalr x0,0(ra)
const BRANCHES: &[u8] = &[
    0x63, 0x04, 0x00, 0x00, 0x13, 0x00, 0x00, 0x00, 0x63, 0x16, 0x00, 0x00, 0x6f, 0x00, 0x80, 0x00,
    0x13, 0x00, 0x00, 0x00, 0x67, 0x80, 0x00, 0x00,
];

#[test]
fn taken_branches_and_jumps_cost_extra_and_untaken_do_not() {
    let mut m = machine_at(RAM_BASE + 0x2000, BRANCHES);
    for _ in 0..4 {
        m.step().unwrap();
    }
    // taken beq 1+2, untaken bne 1, jal 1+1, jalr 1+2
    assert_eq!(m.counters.base_cycles, 3 + 1 + 2 + 3);
    assert_eq!(m.counters.taken, 3);
}

// lui a0,0x3c000; lw a1,0(a0); lw a2,4(a0); lw a3,64(a0); ecall
const DLOAD: &[u8] = &[
    0x37, 0x05, 0x00, 0x3c, 0x83, 0x25, 0x05, 0x00, 0x03, 0x26, 0x45, 0x00, 0x83, 0x26, 0x05, 0x04,
    0x73, 0x00, 0x00, 0x00,
];

#[test]
fn flash_data_reads_go_through_the_cache() {
    let mut m = machine_at(RAM_BASE + 0x2000, DLOAD);
    assert!(m.load(DROM_BASE, &[0; 128]));
    assert_eq!(m.run(100), Stop::Ecall);
    // misses: line 0 (first read), then line 2 (offset 64); the read at +4 hits and pays 1
    assert_eq!(m.counters.dmisses, 2);
    assert_eq!(m.counters.flash_loads, 3);
    assert_eq!(m.counters.base_cycles, 1 + 1 + (1 + 1) + 1 + 1);
}

#[test]
fn instruction_fetch_misses_once_per_cache_line() {
    // 56 bytes at the start of the IROM window span two 32-byte lines.
    let m = run(IROM_BASE, ALU);
    assert_eq!(m.counters.imisses, 2);
    assert_eq!(m.counters.instrs, 14);
}
