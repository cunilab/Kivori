//! RV32IMC instruction-set simulator with the ESP32-C3 memory map and cycle accounting.
//!
//! Executes the bench ELF exactly (real ISA semantics); time is accounted by [`CycleModel`]. Cycle
//! counts are split into `base_cycles` (everything the pipeline spends) and flash-cache misses
//! (`imisses` + `dmisses`), so the scale and miss penalty can be changed after the fact.
use crate::cache::Cache;
use crate::model::CycleModel;
use crate::rvc;

/// Memory regions, as the C3 maps them (TRM chapter 3).
pub const IROM_BASE: u32 = 0x4200_0000;
/// Read-only data window of the flash cache.
pub const DROM_BASE: u32 = 0x3C00_0000;
/// Internal SRAM, data bus.
pub const RAM_BASE: u32 = 0x3FC8_0000;
/// Mask ROM (libgcc and libc routines the product links against).
pub const ROM_BASE: u32 = 0x4000_0000;
/// Marker window the bench writes to; handled by the simulator and free of charge.
pub const MARK_BASE: u32 = 0x6000_0000;
const MARK_SIZE: u32 = 0x1000;

/// Marker register offsets (shared with the bench).
pub mod mark {
    /// Start of a scenario (value: id); name pointer/length registers hold its name.
    pub const SCENARIO: u32 = 0x00;
    /// Start of a frame.
    pub const FRAME_BEGIN: u32 = 0x04;
    /// Start of a tile row (or the whole frame when the renderer has no per-row call).
    pub const ROW_BEGIN: u32 = 0x08;
    /// End of a tile row.
    pub const ROW_END: u32 = 0x0C;
    /// End of a frame.
    pub const FRAME_END: u32 = 0x10;
    /// Program finished (value: exit code).
    pub const EXIT: u32 = 0x14;
    /// Address of the scenario name.
    pub const NAME_PTR: u32 = 0x20;
    /// Length of the scenario name.
    pub const NAME_LEN: u32 = 0x24;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Ram,
    Rom,
    FlashCode,
    FlashData,
}

struct Region {
    base: u32,
    kind: Kind,
    data: Vec<u8>,
}

/// Everything counted while running.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Counters {
    /// Instructions retired.
    pub instrs: u64,
    /// Cycles excluding flash-cache miss stalls.
    pub base_cycles: u64,
    /// Instruction-fetch cache misses.
    pub imisses: u64,
    /// Data-read cache misses.
    pub dmisses: u64,
    /// Loads.
    pub loads: u64,
    /// Loads served by the flash cache window (DROM).
    pub flash_loads: u64,
    /// Stores.
    pub stores: u64,
    /// Multiplies.
    pub muls: u64,
    /// Divides and remainders.
    pub divs: u64,
    /// Taken branches and jumps.
    pub taken: u64,
}

impl Counters {
    /// Misses of either kind.
    pub fn misses(&self) -> u64 {
        self.imisses + self.dmisses
    }

    /// Total cycles under `model`.
    pub fn cycles(&self, model: &CycleModel) -> u64 {
        model.cycles(self.base_cycles, self.misses())
    }

    /// Field-wise `self - earlier`.
    pub fn since(&self, earlier: &Counters) -> Counters {
        Counters {
            instrs: self.instrs - earlier.instrs,
            base_cycles: self.base_cycles - earlier.base_cycles,
            imisses: self.imisses - earlier.imisses,
            dmisses: self.dmisses - earlier.dmisses,
            loads: self.loads - earlier.loads,
            flash_loads: self.flash_loads - earlier.flash_loads,
            stores: self.stores - earlier.stores,
            muls: self.muls - earlier.muls,
            divs: self.divs - earlier.divs,
            taken: self.taken - earlier.taken,
        }
    }
}

/// One marker write, with the counters at that instant.
#[derive(Clone, Debug)]
pub struct Event {
    /// Register offset (see [`mark`]).
    pub kind: u32,
    /// Value written.
    pub value: u32,
    /// Scenario name for [`mark::SCENARIO`], empty otherwise.
    pub name: String,
    /// Counters when it happened.
    pub at: Counters,
}

/// Why the simulation stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    /// The program wrote its exit marker (code).
    Exit(u32),
    /// `ecall` executed.
    Ecall,
    /// `ebreak` executed at pc.
    Ebreak(u32),
    /// Undecodable instruction (pc, word).
    Illegal(u32, u32),
    /// Access to unmapped memory (pc, address).
    Fault(u32, u32),
    /// Instruction budget exhausted.
    Budget,
}

/// The simulated machine.
pub struct Machine {
    /// Registers.
    pub x: [u32; 32],
    /// Program counter.
    pub pc: u32,
    regions: Vec<Region>,
    cache: Cache,
    model: CycleModel,
    /// Running counters.
    pub counters: Counters,
    /// Marker writes in order.
    pub events: Vec<Event>,
    csr: Vec<u32>,
    last_fetch_line: u32,
    /// Destination of the previous instruction when it was a load (0 = none).
    last_load_rd: u32,
    name_ptr: u32,
    name_len: u32,
    exit: Option<u32>,
    /// Cycles a data access of the current instruction adds (flash-cache hit latency).
    pending_cycles: u64,
    /// Most recent `(pc, instruction)` pairs, newest last, for post-mortems.
    pub recent: std::collections::VecDeque<(u32, u32)>,
    /// Base cycles per code halfword of IROM when profiling (see [`Machine::enable_profile`]).
    pub profile: Option<Vec<u64>>,
}

type R<T> = Result<T, Stop>;

impl Machine {
    /// A machine with the C3 memory map, empty memory and an empty cache.
    pub fn new(model: CycleModel) -> Self {
        let region = |base, kind, size: usize| Region {
            base,
            kind,
            data: vec![0; size],
        };
        Machine {
            x: [0; 32],
            pc: 0,
            regions: vec![
                region(RAM_BASE, Kind::Ram, 512 * 1024),
                region(IROM_BASE, Kind::FlashCode, 4 << 20),
                region(DROM_BASE, Kind::FlashData, 4 << 20),
                region(ROM_BASE, Kind::Rom, 384 * 1024),
            ],
            cache: Cache::new(model.cache),
            model,
            counters: Counters::default(),
            events: Vec::new(),
            csr: vec![0; 4096],
            last_fetch_line: u32::MAX,
            last_load_rd: 0,
            name_ptr: 0,
            name_len: 0,
            exit: None,
            pending_cycles: 0,
            recent: std::collections::VecDeque::new(),
            profile: None,
        }
    }

    /// Copies `bytes` to `addr` (loader; no cost). `false` if it does not fit a region.
    pub fn load(&mut self, addr: u32, bytes: &[u8]) -> bool {
        for r in &mut self.regions {
            let end = r.base as u64 + r.data.len() as u64;
            if addr >= r.base && addr as u64 + bytes.len() as u64 <= end {
                let off = (addr - r.base) as usize;
                r.data[off..off + bytes.len()].copy_from_slice(bytes);
                return true;
            }
        }
        false
    }

    fn region(&self, addr: u32) -> Option<usize> {
        self.regions
            .iter()
            .position(|r| addr >= r.base && ((addr - r.base) as usize) < r.data.len())
    }

    fn byte(&self, addr: u32) -> Option<u8> {
        self.region(addr)
            .map(|i| self.regions[i].data[(addr - self.regions[i].base) as usize])
    }

    /// Reads `bytes` bytes little-endian without cost (fetch and loader use).
    fn peek(&self, addr: u32, bytes: u32) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..bytes {
            v |= u32::from(self.byte(addr.wrapping_add(i))?) << (8 * i);
        }
        Some(v)
    }

    /// Reads a NUL-free string (marker support).
    fn peek_string(&self, addr: u32, len: u32) -> String {
        (0..len)
            .filter_map(|i| self.byte(addr.wrapping_add(i)))
            .map(char::from)
            .collect()
    }

    fn fault(&self, addr: u32) -> Stop {
        Stop::Fault(self.pc, addr)
    }

    fn data_read(&mut self, addr: u32, bytes: u32) -> R<u32> {
        self.counters.loads += 1;
        if (MARK_BASE..MARK_BASE + MARK_SIZE).contains(&addr) {
            return Ok(0);
        }
        let Some(i) = self.region(addr) else {
            return Err(self.fault(addr));
        };
        if self.regions[i].kind == Kind::FlashData || self.regions[i].kind == Kind::FlashCode {
            self.counters.flash_loads += 1;
            let hit = self.cache.access(addr);
            if hit {
                self.pending_cycles += u64::from(self.model.flash_load_hit);
            } else {
                self.counters.dmisses += 1;
            }
            // A read crossing into the next line touches it as well.
            let last = addr.wrapping_add(bytes - 1);
            if self.cache.line_of(last) != self.cache.line_of(addr) && !self.cache.access(last) {
                self.counters.dmisses += 1;
            }
        }
        self.peek(addr, bytes).ok_or_else(|| self.fault(addr))
    }

    fn data_write(&mut self, addr: u32, bytes: u32, value: u32) -> R<()> {
        self.counters.stores += 1;
        if (MARK_BASE..MARK_BASE + MARK_SIZE).contains(&addr) {
            return self.marker(addr - MARK_BASE, value);
        }
        let Some(i) = self.region(addr) else {
            return Err(self.fault(addr));
        };
        if self.regions[i].kind != Kind::Ram {
            return Err(self.fault(addr));
        }
        let r = &mut self.regions[i];
        let off = (addr - r.base) as usize;
        if off + bytes as usize > r.data.len() {
            return Err(Stop::Fault(self.pc, addr));
        }
        for b in 0..bytes as usize {
            r.data[off + b] = (value >> (8 * b)) as u8;
        }
        Ok(())
    }

    fn marker(&mut self, offset: u32, value: u32) -> R<()> {
        // A marker store is a simulator call, not an instruction of the program.
        self.counters.stores -= 1;
        match offset {
            mark::NAME_PTR => self.name_ptr = value,
            mark::NAME_LEN => self.name_len = value,
            mark::SCENARIO..=mark::EXIT => {
                let name = if offset == mark::SCENARIO {
                    self.peek_string(self.name_ptr, self.name_len)
                } else {
                    String::new()
                };
                self.events.push(Event {
                    kind: offset,
                    value,
                    name,
                    at: self.counters,
                });
                if offset == mark::EXIT {
                    self.exit = Some(value);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Charges the instruction fetch of `len` bytes at `pc`.
    fn fetch_cost(&mut self, pc: u32, len: u32) {
        let Some(i) = self.region(pc) else { return };
        if self.regions[i].kind != Kind::FlashCode && self.regions[i].kind != Kind::FlashData {
            return;
        }
        for addr in [pc, pc + len - 1] {
            let line = self.cache.line_of(addr);
            if line != self.last_fetch_line {
                self.last_fetch_line = line;
                if !self.cache.access(addr) {
                    self.counters.imisses += 1;
                }
            }
        }
    }

    fn csr_read(&self, csr: u32) -> u32 {
        let cycles = self.counters.cycles(&self.model);
        match csr {
            0xB00 | 0xC00 => cycles as u32,
            0xB80 | 0xC80 => (cycles >> 32) as u32,
            0xB02 | 0xC02 => self.counters.instrs as u32,
            0xB82 | 0xC82 => (self.counters.instrs >> 32) as u32,
            _ => self.csr[csr as usize & 0xfff],
        }
    }

    /// Starts attributing base cycles to code addresses in IROM.
    pub fn enable_profile(&mut self) {
        self.profile = Some(vec![0; 2 << 20]);
    }

    /// Runs until a stop condition or `budget` instructions.
    pub fn run(&mut self, budget: u64) -> Stop {
        for _ in 0..budget {
            if let Err(stop) = self.step() {
                return stop;
            }
            if let Some(code) = self.exit {
                return Stop::Exit(code);
            }
        }
        Stop::Budget
    }

    /// Executes one instruction.
    pub fn step(&mut self) -> R<()> {
        let pc = self.pc;
        let Some(low) = self.peek(pc, 2) else {
            return Err(Stop::Fault(pc, pc));
        };
        let (word, len) = if low & 3 == 3 {
            let Some(w) = self.peek(pc, 4) else {
                return Err(Stop::Fault(pc, pc));
            };
            (w, 4)
        } else {
            match rvc::expand(low as u16) {
                Some(w) => (w, 2),
                None => return Err(Stop::Illegal(pc, low)),
            }
        };
        self.fetch_cost(pc, len);
        if self.recent.len() == 24 {
            self.recent.pop_front();
        }
        self.recent.push_back((pc, word));
        self.exec(word, len)
    }

    fn exec(&mut self, w: u32, len: u32) -> R<()> {
        let pc = self.pc;
        let opcode = w & 0x7f;
        let rd = (w >> 7) & 31;
        let f3 = (w >> 12) & 7;
        let rs1 = (w >> 15) & 31;
        let rs2 = (w >> 20) & 31;
        let f7 = w >> 25;
        let (a, b) = (self.x[rs1 as usize], self.x[rs2 as usize]);
        let imm_i = (w as i32) >> 20;
        let m = self.model;

        // Which source registers this instruction reads (for the load-use bubble).
        let uses_rs1 = !matches!(opcode, 0x37 | 0x17 | 0x6f);
        let uses_rs2 = matches!(opcode, 0x33 | 0x63 | 0x23);
        let mut cycles = 1u64;
        if self.last_load_rd != 0
            && ((uses_rs1 && rs1 == self.last_load_rd) || (uses_rs2 && rs2 == self.last_load_rd))
        {
            cycles += u64::from(m.load_use_stall);
        }
        self.last_load_rd = 0;
        let mut next = pc.wrapping_add(len);
        let mut write: Option<u32> = None;

        match opcode {
            0x37 => write = Some(w & 0xffff_f000),
            0x17 => write = Some(pc.wrapping_add(w & 0xffff_f000)),
            0x6f => {
                let imm = (((w as i32) >> 31) << 20) as u32
                    | (w & 0xff000)
                    | ((w >> 9) & 0x800)
                    | ((w >> 20) & 0x7fe);
                write = Some(next);
                next = pc.wrapping_add(imm);
                cycles += u64::from(m.jal);
                self.counters.taken += 1;
            }
            0x67 if f3 == 0 => {
                write = Some(next);
                next = a.wrapping_add(imm_i as u32) & !1;
                cycles += u64::from(m.jalr);
                self.counters.taken += 1;
            }
            0x63 => {
                let imm = (((w as i32) >> 31) << 12) as u32
                    | ((w << 4) & 0x800)
                    | ((w >> 20) & 0x7e0)
                    | ((w >> 7) & 0x1e);
                let take = match f3 {
                    0 => a == b,
                    1 => a != b,
                    4 => (a as i32) < (b as i32),
                    5 => (a as i32) >= (b as i32),
                    6 => a < b,
                    7 => a >= b,
                    _ => return Err(Stop::Illegal(pc, w)),
                };
                if take {
                    next = pc.wrapping_add(imm);
                    cycles += u64::from(m.branch_taken);
                    self.counters.taken += 1;
                }
            }
            0x03 => {
                let addr = a.wrapping_add(imm_i as u32);
                let v = match f3 {
                    0 => self.data_read(addr, 1)? as i8 as i32 as u32,
                    1 => self.data_read(addr, 2)? as i16 as i32 as u32,
                    2 => self.data_read(addr, 4)?,
                    4 => self.data_read(addr, 1)?,
                    5 => self.data_read(addr, 2)?,
                    _ => return Err(Stop::Illegal(pc, w)),
                };
                write = Some(v);
                self.last_load_rd = rd;
            }
            0x23 => {
                let imm = ((((w as i32) >> 25) << 5) as u32) | ((w >> 7) & 0x1f);
                let addr = a.wrapping_add(imm);
                match f3 {
                    0 => self.data_write(addr, 1, b)?,
                    1 => self.data_write(addr, 2, b)?,
                    2 => self.data_write(addr, 4, b)?,
                    _ => return Err(Stop::Illegal(pc, w)),
                }
            }
            0x13 => {
                let shamt = rs2;
                write = Some(match f3 {
                    0 => a.wrapping_add(imm_i as u32),
                    2 => u32::from((a as i32) < imm_i),
                    3 => u32::from(a < imm_i as u32),
                    4 => a ^ imm_i as u32,
                    6 => a | imm_i as u32,
                    7 => a & imm_i as u32,
                    1 if f7 == 0 => a << shamt,
                    5 if f7 == 0 => a >> shamt,
                    5 if f7 == 0x20 => ((a as i32) >> shamt) as u32,
                    _ => return Err(Stop::Illegal(pc, w)),
                });
            }
            0x33 if f7 == 1 => {
                write = Some(match f3 {
                    0..=3 => {
                        self.counters.muls += 1;
                        cycles += u64::from(m.mul);
                        match f3 {
                            0 => a.wrapping_mul(b),
                            1 => ((i64::from(a as i32) * i64::from(b as i32)) >> 32) as u32,
                            2 => ((i64::from(a as i32) * i64::from(b)) >> 32) as u32,
                            _ => ((u64::from(a) * u64::from(b)) >> 32) as u32,
                        }
                    }
                    _ => {
                        self.counters.divs += 1;
                        cycles += u64::from(m.div);
                        div_rem(f3, a, b)
                    }
                });
            }
            0x33 => {
                write = Some(match (f3, f7) {
                    (0, 0) => a.wrapping_add(b),
                    (0, 0x20) => a.wrapping_sub(b),
                    (1, 0) => a << (b & 31),
                    (2, 0) => u32::from((a as i32) < (b as i32)),
                    (3, 0) => u32::from(a < b),
                    (4, 0) => a ^ b,
                    (5, 0) => a >> (b & 31),
                    (5, 0x20) => ((a as i32) >> (b & 31)) as u32,
                    (6, 0) => a | b,
                    (7, 0) => a & b,
                    _ => return Err(Stop::Illegal(pc, w)),
                });
            }
            0x0f => {} // fence / fence.i
            0x73 => match f3 {
                0 => {
                    self.counters.instrs += 1;
                    self.counters.base_cycles += 1;
                    return Err(if w >> 20 == 1 {
                        Stop::Ebreak(pc)
                    } else {
                        Stop::Ecall
                    });
                }
                _ => {
                    let csr = w >> 20;
                    let old = self.csr_read(csr);
                    let operand = if f3 >= 5 { rs1 } else { a };
                    let new = match f3 & 3 {
                        1 => Some(operand),
                        2 if rs1 != 0 => Some(old | operand),
                        3 if rs1 != 0 => Some(old & !operand),
                        _ => None,
                    };
                    if let Some(v) = new {
                        self.csr[csr as usize & 0xfff] = v;
                    }
                    write = Some(old);
                }
            },
            _ => return Err(Stop::Illegal(pc, w)),
        }
        if let Some(v) = write {
            if rd != 0 {
                self.x[rd as usize] = v;
            }
        }
        self.pc = next;
        self.counters.instrs += 1;
        let spent = cycles + std::mem::take(&mut self.pending_cycles);
        self.counters.base_cycles += spent;
        if let Some(profile) = &mut self.profile {
            if let Some(slot) = profile.get_mut((pc.wrapping_sub(IROM_BASE) / 2) as usize) {
                *slot += spent;
            }
        }
        Ok(())
    }
}

/// `div`/`divu`/`rem`/`remu` with the RISC-V results for zero divisors and overflow.
pub fn div_rem(f3: u32, a: u32, b: u32) -> u32 {
    let (sa, sb) = (a as i32, b as i32);
    match f3 {
        4 if b == 0 => u32::MAX,
        4 if sa == i32::MIN && sb == -1 => a,
        4 => (sa / sb) as u32,
        5 if b == 0 => u32::MAX,
        5 => a / b,
        6 if b == 0 => a,
        6 if sa == i32::MIN && sb == -1 => 0,
        6 => (sa % sb) as u32,
        _ if b == 0 => a,
        _ => a % b,
    }
}
