#!/usr/bin/env bun
// Hardware-free resource check of the catalog firmware: reads the release ELF and checks it against
// the committed budget (`resource-budget.json` in the firmware crate). No board is needed, so CI can catch a stack,
// RAM, size or 64-bit-division regression that used to surface only on the board itself.
//
//   bun scripts/firmware-budget.mjs [--elf PATH] [--budget PATH] [--json] [--disasm FILE]
//
// Without --elf/--budget the catalog-selected firmware is used (KIVORI_PRODUCT + KIVORI_BOARD when
// several exist, as for `just fw-build`): its release artifact and `resource-budget.json` beside it.
// Build first with `just fw-build`, or run `just fw-budget`.
//
// What it measures, all from the ELF:
//   * app image bytes (loadable sections) and the asset blob inside it (vs its cap in internals.rs)
//   * static RAM (`_stack_end` - start of data RAM) and the stack region (`_stack_start` - `_stack_end`)
//   * the stack frame of every function, summed over the whole prologue (`push {..}`, `sub sp, #N`,
//     and a big frame is `ldr rX, [pc, ..]` of a negative constant then `add sp, rX`)
//   * the worst stack depth over the direct call graph from the entry function. Calls through a
//     register (trait objects, fn pointers) and interrupts cannot be followed; they are reported and
//     covered by documented allowances in the budget.
//   * which functions call the 64-bit division libcalls (a software loop on the Cortex-M0+ core).
// Exit status 1 when the budget is violated, 2 when the ELF cannot be analysed.

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const DATA_RAM_START = 0x20000000;
// The prologue is straight-line code before the first call; this bounds the scan for a function that
// has none.
const PROLOGUE_SCAN_LIMIT = 80;
const DIV64_LIBCALLS = new Set(['__aeabi_uldivmod', '__aeabi_ldivmod']);

// ---------------------------------------------------------------------------------------------
// Disassembly parsing

/** Parses an objdump integer operand: `0x1f`, `-0x1f` or decimal. */
export function num(text) {
  const s = String(text).trim();
  const m = /^(-?)(0x[0-9a-fA-F]+|\d+)$/.exec(s);
  if (!m) return null;
  const v = Number(m[2]);
  return m[1] ? -v : v;
}

// Conditional branches (Thumb-1 has no wide conditional form on this core).
const BRANCHES = new Set(
  [
    'eq',
    'ne',
    'cs',
    'hs',
    'cc',
    'lo',
    'mi',
    'pl',
    'vs',
    'vc',
    'hi',
    'ls',
    'ge',
    'lt',
    'gt',
    'le',
  ].map((c) => `b${c}`),
);
// Instructions whose first operand is not a destination register.
const NO_REGISTER_WRITE = new Set([
  'str', 'strh', 'strb', 'stm', 'stmia', 'push', 'cmp', 'cmn', 'tst', 'b', 'bl', 'blx', 'bx',
  'nop', 'wfi', 'wfe', 'sev', 'yield', 'dsb', 'dmb', 'isb', 'msr', 'svc', 'bkpt', 'udf',
  ...BRANCHES,
]); // prettier-ignore
/** A call clobbers the caller-saved registers; r4 and up survive it. */
function forgetCallClobbered(known) {
  for (const r of ['r0', 'r1', 'r2', 'r3', 'r12']) delete known[r];
}
/** An immediate operand: `#0x18`, `#-0x4` or `#12`. */
const imm = (text) => num(String(text).replace(/^#/, ''));

/**
 * Parses `llvm-objdump -d --no-show-raw-insn -C` text of a Thumb-1 (thumbv6m) image into functions:
 * start address, name, stack frame, and the control transfers that leave the function (direct calls,
 * tail jumps, indirect calls). Tail jumps are resolved against function ranges once the whole text is
 * read. Literal pools (`.word` lines) are read first: a large frame is `ldr rX, [pc, ..]` of a
 * negative constant followed by `add sp, rX`, and a call through a hoisted address is `blx rX`.
 * @returns {{functions: Array<object>}}
 */
export function parseDisassembly(text) {
  const functions = [];
  const lines = text.split('\n');
  const words = new Map(); // literal pool: address -> 32-bit value
  for (const line of lines) {
    const w = /^\s*([0-9a-f]+):\s+(?:[0-9a-f]{2}\s+)+\.word\s+(0x[0-9a-f]+)/.exec(line);
    if (w) words.set(parseInt(w[1], 16), Number(w[2]) >>> 0);
  }
  let cur = null;
  let scanned = 0;
  let prologueDone = false;
  let known = {}; // registers holding a constant, for frames and for resolving `blx` targets
  const pending = []; // jumps whose target is resolved after all functions are known
  for (const line of lines) {
    const header = /^([0-9a-f]+) <(.+)>:$/.exec(line);
    // `.L` labels sit inside a function; they are not functions.
    if (header && header[2].startsWith('.L')) continue;
    if (header) {
      cur = {
        addr: parseInt(header[1], 16),
        name: header[2],
        end: 0,
        frame: 0,
        calls: [], // direct calls: {addr, name}
        tails: [], // jumps out of the function: {addr}
        indirect: 0, // calls through a register whose value is not a known constant
        indirectJumps: 0, // `bx rX` that is not a return (jump tables, indirect tail calls)
        div64: new Set(),
      };
      functions.push(cur);
      scanned = 0;
      prologueDone = false;
      known = {};
      continue;
    }
    if (!cur) continue;
    if (/^\s*[0-9a-f]+:\s+(?:[0-9a-f]{2}\s+)+\.word/.test(line)) {
      // A literal pool word belongs to the function it follows.
      cur.end = parseInt(line, 16) + 4;
      continue;
    }
    const ins = /^\s*([0-9a-f]+):\s+([\w.]+)\s*(.*)$/.exec(line);
    if (!ins) continue;
    const pc = parseInt(ins[1], 16);
    cur.end = pc + 2;
    const op = ins[2];
    let rest = ins[3].trim().replace(/\s*@\s*imm\s*=.*$/, '');
    // `ldr rX, [pc, #n] @ 0xADDR <sym+off>`: the literal's address.
    const literal = /@\s*(0x[0-9a-f]+)/.exec(rest);
    rest = rest.replace(/\s*@.*$/, '');
    let sym = null;
    const symMatch = /\s*<(.+)>$/.exec(rest);
    if (symMatch) {
      sym = symMatch[1].replace(/\+0x[0-9a-f]+$/, '');
      rest = rest.slice(0, symMatch.index).trim();
    }
    const a = rest.split(/,(?![^{]*})/).map((x) => x.trim());
    scanned += 1;

    // Prologue frame: pushes and every stack allocation before the first call.
    if (!prologueDone) {
      if (op === 'push') cur.frame += 4 * rest.replace(/[{}]/g, '').split(',').length;
      else if (op === 'sub' && a[0] === 'sp' && a.length === 2) cur.frame += Math.max(0, imm(a[1]));
      else if (op === 'add' && a[0] === 'sp' && known[a[1]] !== undefined)
        cur.frame += Math.max(0, -(known[a[1]] | 0));
    }

    const target = (t) => num(t.replace(/\s.*/, ''));
    if (op === 'bl') {
      prologueDone = true;
      call(cur, target(a[0]), sym);
      forgetCallClobbered(known);
    } else if (op === 'blx') {
      prologueDone = true;
      // The target is known when the register holds a constant (an address loaded from a literal pool).
      if (known[a[0]] === undefined) cur.indirect += 1;
      else call(cur, known[a[0]] & ~1, sym);
      forgetCallClobbered(known);
    } else if (op === 'b' || BRANCHES.has(op)) {
      pending.push({ from: cur, target: target(a[0]) });
    } else if (op === 'bx') {
      if (known[a[0]] !== undefined) pending.push({ from: cur, target: known[a[0]] & ~1 });
      else if (a[0] !== 'lr') cur.indirectJumps += 1;
    } else if (op === 'ldr' && literal && /^\[pc/.test(a[1] ?? '')) {
      const value = words.get(parseInt(literal[1], 16));
      if (value === undefined) delete known[a[0]];
      else known[a[0]] = value;
    } else if ((op === 'movs' || op === 'mov') && a[1]?.startsWith('#')) {
      known[a[0]] = imm(a[1]) >>> 0;
    } else if (op === 'mov' && known[a[1]] !== undefined) {
      known[a[0]] = known[a[1]];
    } else if (op === 'lsls' && a.length === 3 && known[a[1]] !== undefined) {
      known[a[0]] = (known[a[1]] << imm(a[2])) >>> 0;
    } else if ((op === 'adds' || op === 'subs') && known[a[a.length === 3 ? 1 : 0]] !== undefined) {
      const v = a.length === 3 ? imm(a[2]) : imm(a[1]);
      known[a[0]] = (known[a[a.length === 3 ? 1 : 0]] + (op === 'adds' ? v : -v)) >>> 0;
    } else if (op === 'mvns' && known[a[1]] !== undefined) {
      known[a[0]] = ~known[a[1]] >>> 0;
    } else if (
      (op === 'negs' && known[a[1]] !== undefined) ||
      (op === 'rsbs' && known[a[1]] !== undefined && imm(a[2]) === 0)
    ) {
      known[a[0]] = -known[a[1]] >>> 0;
    } else if (!NO_REGISTER_WRITE.has(op)) {
      delete known[a[0]];
    }
    if (scanned >= PROLOGUE_SCAN_LIMIT) prologueDone = true;
  }

  // Thumb-1 reaches a far block of the same function with `bl` (a conditional branch cannot): that is
  // a jump inside the function, not a recursive call.
  for (const f of functions) f.calls = f.calls.filter((c) => !(c.addr > f.addr && c.addr < f.end));
  const byAddr = new Map(functions.map((f) => [f.addr, f]));
  const sorted = [...functions].sort((x, y) => x.addr - y.addr);
  const containing = (addr) => {
    let lo = 0;
    let hi = sorted.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (sorted[mid].addr <= addr) lo = mid + 1;
      else hi = mid - 1;
    }
    return hi >= 0 && addr < sorted[hi].end ? sorted[hi] : null;
  };
  for (const f of functions)
    f.external = f.calls.filter(
      (c) => c.addr !== null && c.name === null && !byAddr.has(c.addr),
    ).length;
  for (const { from, target } of pending) {
    if (target === null) continue;
    const to = containing(target);
    if (to && to !== from) from.tails.push({ addr: to.addr, name: to.name });
  }
  return { functions, byAddr };

  function call(from, addr, name) {
    from.calls.push({ addr, name });
    if (DIV64_LIBCALLS.has(name)) from.div64.add(name);
  }
}

// ---------------------------------------------------------------------------------------------
// Stack estimate over the direct call graph

/**
 * Longest stack path (summed frames) from `entryName`, following direct calls and tail jumps. A tail
 * jump leaves the caller's frame already popped, so it adds the callee but not the caller.
 * Recursion cannot be bounded: a back edge is cut and reported in `cycles`.
 */
export function worstStack(parsed, entryName) {
  const { functions, byAddr } = parsed;
  const byName = new Map();
  for (const f of functions) if (!byName.has(f.name)) byName.set(f.name, f);
  const entry = byName.get(entryName);
  if (!entry) return null;
  const resolve = (edge) => byAddr.get(edge.addr) ?? byName.get(edge.name);
  const memo = new Map(); // fn -> {depth, next, viaTail}
  const onStack = new Set();
  const cycles = new Set();
  const visit = (f) => {
    if (memo.has(f)) return memo.get(f);
    onStack.add(f);
    let best = { depth: f.frame, next: null, viaTail: false };
    for (const edge of f.calls) {
      const g = resolve(edge);
      if (!g) continue;
      if (onStack.has(g)) {
        cycles.add(g.name);
        continue;
      }
      const depth = f.frame + visit(g).depth;
      if (depth > best.depth) best = { depth, next: g, viaTail: false };
    }
    for (const edge of f.tails) {
      const g = byAddr.get(edge.addr);
      if (!g) continue;
      if (onStack.has(g)) {
        cycles.add(g.name);
        continue;
      }
      const depth = visit(g).depth;
      if (depth > best.depth) best = { depth, next: g, viaTail: true };
    }
    onStack.delete(f);
    memo.set(f, best);
    return best;
  };
  visit(entry);
  const chain = [];
  const seen = new Set();
  let viaTail = false; // whether the previous hop was a tail jump into `f`
  for (let f = entry; f && !seen.has(f); f = memo.get(f).next) {
    seen.add(f);
    chain.push({ name: f.name, frame: f.frame, tail: viaTail });
    viaTail = memo.get(f).viaTail;
  }
  const reached = new Set();
  const reach = (f) => {
    if (reached.has(f)) return;
    reached.add(f);
    for (const e of f.calls) {
      const g = resolve(e);
      if (g) reach(g);
    }
    for (const e of f.tails) {
      const g = byAddr.get(e.addr);
      if (g) reach(g);
    }
  };
  reach(entry);
  return {
    entry: entryName,
    depth: memo.get(entry).depth,
    chain,
    cycles: [...cycles],
    reached,
    indirectOnChain: chain.reduce((n, c) => n + (byName.get(c.name)?.indirect ?? 0), 0),
  };
}

/**
 * Stack an interrupt can add: `path` names the functions an interrupt runs through, outermost first
 * (trap entry, dispatcher, handler). The hops between them are indirect calls, so each function's own
 * frame is summed and the last one contributes its whole direct chain.
 */
export function interruptStack(parsed, path) {
  const missing = [];
  let bytes = 0;
  path.forEach((name, i) => {
    const f = parsed.functions.find((x) => x.name === name);
    if (!f) return missing.push(name);
    bytes += i < path.length - 1 ? f.frame : (worstStack(parsed, name)?.depth ?? f.frame);
  });
  return { bytes, missing };
}

/** Shortens the long generic names for messages. */
export const short = (name, n = 150) => (name.length > n ? `${name.slice(0, n - 3)}...` : name);

/** `*` is the only wildcard. */
export function matches(pattern, name) {
  if (!pattern.includes('*')) return pattern === name;
  const re = new RegExp(
    `^${pattern
      .split('*')
      .map((s) => s.replace(/[.+?^${}()|[\]\\]/g, '\\$&'))
      .join('.*')}$`,
  );
  return re.test(name);
}

// ---------------------------------------------------------------------------------------------
// ELF facts

/** Reads the facts the budget needs straight from an ELF32 little-endian image. */
export function parseElf(buf) {
  if (buf.readUInt32BE(0) !== 0x7f454c46 || buf[4] !== 1 || buf[5] !== 1)
    throw new Error('not an ELF32 little-endian file');
  const shoff = buf.readUInt32LE(32);
  const shentsize = buf.readUInt16LE(46);
  const shnum = buf.readUInt16LE(48);
  const shstrndx = buf.readUInt16LE(50);
  const sections = [];
  for (let i = 0; i < shnum; i++) {
    const o = shoff + i * shentsize;
    sections.push({
      nameOff: buf.readUInt32LE(o),
      type: buf.readUInt32LE(o + 4),
      flags: buf.readUInt32LE(o + 8),
      addr: buf.readUInt32LE(o + 12),
      offset: buf.readUInt32LE(o + 16),
      size: buf.readUInt32LE(o + 20),
      link: buf.readUInt32LE(o + 24),
    });
  }
  const cstr = (off) => buf.toString('latin1', off, buf.indexOf(0, off));
  const shstr = sections[shstrndx];
  for (const s of sections) s.name = cstr(shstr.offset + s.nameOff);
  const symtab = sections.find((s) => s.type === 2);
  const symbols = new Map();
  if (symtab) {
    const strtab = sections[symtab.link];
    for (let o = symtab.offset; o < symtab.offset + symtab.size; o += 16) {
      symbols.set(cstr(strtab.offset + buf.readUInt32LE(o)), buf.readUInt32LE(o + 4));
    }
  }
  const SHT_PROGBITS = 1;
  const SHF_ALLOC = 2;
  // What the flash image holds: the sections that are both loaded and have bytes in the file.
  const imageBytes = sections
    .filter((s) => s.type === SHT_PROGBITS && s.flags & SHF_ALLOC)
    .reduce((n, s) => n + s.size, 0);
  return { sections, symbols, imageBytes, assetBlobBytes: findAssetBlob(buf, sections) };
}

/** Finds the compiled asset blob (`KASS` header, 16 + manifest + pool bytes) in the read-only data. */
function findAssetBlob(buf, sections) {
  const ro = sections.find((s) => s.name === '.rodata');
  if (!ro) return null;
  for (let o = ro.offset; o + 16 <= ro.offset + ro.size; o += 4) {
    if (buf.readUInt32LE(o) !== 0x5353414b) continue;
    const total = 16 + buf.readUInt32LE(o + 8) + buf.readUInt32LE(o + 12);
    if (o + total <= ro.offset + ro.size) return total;
  }
  return null;
}

export function measureElf(elf) {
  const stackStart = elf.symbols.get('_stack_start');
  const stackEnd = elf.symbols.get('_stack_end');
  if (stackStart === undefined || stackEnd === undefined)
    throw new Error('ELF has no _stack_start/_stack_end symbols');
  return {
    appImageBytes: elf.imageBytes,
    assetBlobBytes: elf.assetBlobBytes,
    staticRamBytes: stackEnd - DATA_RAM_START,
    stackRegionBytes: stackStart - stackEnd,
  };
}

// ---------------------------------------------------------------------------------------------
// Budget check

/**
 * Evaluates every limit. `measured` is `measureElf` output plus `blobCapBytes`; `parsed` the
 * disassembly. Returns the figures the report prints and the list of violations/warnings.
 */
export function evaluate({ measured, parsed, budget }) {
  const limits = budget.limits;
  const violations = [];
  const warnings = [];
  const over = (label, value, max) => {
    if (value > max) violations.push(`${label}: ${value} B exceeds the budget of ${max} B`);
  };

  over('app image', measured.appImageBytes, limits.app_image_bytes.max);
  if (measured.assetBlobBytes === null) warnings.push('asset blob not found in the ELF');
  else {
    over('asset blob', measured.assetBlobBytes, limits.asset_blob_bytes.max);
    if (measured.blobCapBytes !== null)
      over('asset blob vs its cap in internals.rs', measured.assetBlobBytes, measured.blobCapBytes);
  }
  over('static RAM', measured.staticRamBytes, limits.static_ram_bytes.max);
  if (measured.stackRegionBytes < limits.stack_region_bytes.min)
    violations.push(
      `stack region: ${measured.stackRegionBytes} B is below the budget minimum of ${limits.stack_region_bytes.min} B`,
    );

  // Frames: anything above the default ceiling must be listed with its own ceiling.
  const frames = parsed.functions.filter((f) => f.frame > 0).sort((x, y) => y.frame - x.frame);
  const frameRules = budget.large_frames ?? [];
  for (const f of frames) {
    if (f.frame <= limits.default_frame_bytes.max) break;
    const rule = frameRules.find((r) => matches(r.fn, f.name));
    if (!rule)
      violations.push(
        `stack frame: ${short(f.name)} uses ${f.frame} B, above the ${limits.default_frame_bytes.max} B default (add it to large_frames with a reason if intended)`,
      );
    else if (f.frame > rule.max)
      violations.push(
        `stack frame: ${short(f.name)} uses ${f.frame} B, above its budget of ${rule.max} B`,
      );
  }
  for (const rule of frameRules)
    if (
      !parsed.functions.some(
        (f) => matches(rule.fn, f.name) && f.frame > limits.default_frame_bytes.max,
      )
    )
      warnings.push(`large_frames entry no longer needed: ${rule.fn}`);

  // Stack estimate.
  const entry = worstStack(parsed, budget.entry ?? 'main');
  let stack = null;
  if (!entry)
    violations.push(`entry function ${budget.entry ?? 'main'} not found in the disassembly`);
  else {
    // Handlers at one priority do not nest, so the deepest single path is the allowance.
    const isrs = (budget.interrupt_paths ?? [budget.interrupt_path ?? []]).map((path) =>
      interruptStack(parsed, path),
    );
    const isr = {
      bytes: Math.max(0, ...isrs.map((x) => x.bytes)),
      missing: isrs.flatMap((x) => x.missing),
    };
    if (isr.missing.length)
      violations.push(`interrupt_path function not found in the ELF: ${isr.missing.join(', ')}`);
    const interruptBytes = isr.bytes + limits.interrupt_margin_bytes.value;
    const indirectBytes = limits.indirect_allowance_bytes.value;
    const peak = entry.depth + indirectBytes + interruptBytes;
    stack = {
      directBytes: entry.depth,
      indirectAllowanceBytes: indirectBytes,
      interruptBytes,
      interruptPathBytes: isr.bytes,
      peakBytes: peak,
      regionBytes: measured.stackRegionBytes,
      headroomBytes: measured.stackRegionBytes - peak,
      chain: entry.chain,
      cycles: entry.cycles,
      indirectCallsOnChain: entry.indirectOnChain,
      reachableFunctions: entry.reached.size,
    };
    over('direct worst-case stack', entry.depth, limits.worst_stack_bytes.max);
    if (peak + limits.stack_margin_bytes.value > measured.stackRegionBytes)
      violations.push(
        `stack: estimated peak ${peak} B + margin ${limits.stack_margin_bytes.value} B exceeds the ${measured.stackRegionBytes} B stack region`,
      );
    if (entry.cycles.length)
      warnings.push(
        `recursion cut in the call graph (depth is a lower bound): ${entry.cycles.join(', ')}`,
      );
  }

  // 64-bit division libcalls.
  const allow = budget.int64_division_allowlist ?? [];
  const callers = parsed.functions.filter((f) => f.div64.size);
  for (const f of callers)
    if (!allow.some((r) => matches(r.fn, f.name)))
      violations.push(
        `64-bit division: ${short(f.name)} calls ${[...f.div64].join(', ')} (a software loop on this core; use 32-bit math or add it to int64_division_allowlist with a reason)`,
      );
  for (const r of allow)
    if (!callers.some((f) => matches(r.fn, f.name)))
      warnings.push(`int64_division_allowlist entry no longer calls a 64-bit division: ${r.fn}`);

  return {
    measured,
    stack,
    topFrames: frames.slice(0, 12).map((f) => ({ name: f.name, bytes: f.frame })),
    int64Callers: callers.map((f) => ({ name: f.name, calls: [...f.div64] })),
    indirect: {
      callSites: parsed.functions.reduce((n, f) => n + f.indirect, 0),
      jumpSites: parsed.functions.reduce((n, f) => n + f.indirectJumps, 0),
    },
    violations,
    warnings,
  };
}

// ---------------------------------------------------------------------------------------------
// Report

const kib = (n) => `${(n / 1024).toFixed(1)} KiB`;
const fmt = (n) => n.toLocaleString('en-US');

export function renderReport(r, budget, elfPath) {
  const m = r.measured;
  const L = budget.limits;
  const out = [`firmware resource check: ${elfPath}`, ''];
  const row = (label, value, max, unit = 'B') =>
    out.push(
      `  ${label.padEnd(26)}${`${fmt(value)} ${unit}`.padStart(14)}   budget ${fmt(max)} ${unit}`,
    );
  row('app image', m.appImageBytes, L.app_image_bytes.max);
  if (m.assetBlobBytes !== null)
    row(
      'asset blob (in image)',
      m.assetBlobBytes,
      Math.min(L.asset_blob_bytes.max, m.blobCapBytes ?? Infinity),
    );
  row('static RAM', m.staticRamBytes, L.static_ram_bytes.max);
  out.push(
    `  ${'stack region'.padEnd(26)}${`${fmt(m.stackRegionBytes)} B`.padStart(14)}   budget min ${fmt(L.stack_region_bytes.min)} B`,
  );
  if (r.stack) {
    const s = r.stack;
    out.push('', 'stack estimate (direct call graph + documented allowances)');
    out.push(
      `  direct worst chain        ${fmt(s.directBytes).padStart(10)} B  (${kib(s.directBytes)})   budget ${fmt(L.worst_stack_bytes.max)} B`,
    );
    out.push(`  indirect-call allowance   ${fmt(s.indirectAllowanceBytes).padStart(10)} B`);
    out.push(
      `  interrupt allowance       ${fmt(s.interruptBytes).padStart(10)} B  (trap entry to handler ${s.interruptPathBytes} + margin ${L.interrupt_margin_bytes.value})`,
    );
    out.push(
      `  estimated peak            ${fmt(s.peakBytes).padStart(10)} B  (${kib(s.peakBytes)})`,
    );
    out.push(
      `  stack region              ${fmt(s.regionBytes).padStart(10)} B  headroom ${fmt(s.headroomBytes)} B (required margin ${fmt(L.stack_margin_bytes.value)} B)`,
    );
    out.push(
      '',
      `worst chain from ${s.chain[0].name}  (${s.reachableFunctions} functions reachable)`,
    );
    let depth = 0;
    for (const c of s.chain) {
      out.push(
        `  ${String(c.frame).padStart(6)} B  ${'  '.repeat(Math.min(depth, 6))}${short(c.name, 110)}${c.tail ? '  [tail jump]' : ''}`,
      );
      if (!c.tail) depth += 1;
    }
    out.push(
      `  (${s.indirectCallsOnChain} call sites on this chain go through a register and are not followed; ${r.indirect.callSites} in the whole image)`,
    );
  }
  out.push('', 'largest stack frames');
  for (const f of r.topFrames)
    out.push(`  ${String(f.bytes).padStart(6)} B  ${short(f.name, 110)}`);
  out.push(
    '',
    `functions calling 64-bit division (${r.int64Callers.length}, allowlisted unless listed below)`,
  );
  for (const c of r.int64Callers) out.push(`  ${short(c.name, 110)}  -> ${c.calls.join(', ')}`);
  if (r.warnings.length) out.push('', ...r.warnings.map((w) => `warning: ${w}`));
  out.push('');
  if (r.violations.length)
    out.push(
      ...r.violations.map((v) => `FAIL ${v}`),
      '',
      `${r.violations.length} budget violation(s)`,
    );
  else out.push('OK: within budget');
  return out.join('\n');
}

// ---------------------------------------------------------------------------------------------
// CLI

export function findObjdump() {
  const candidates = [];
  if (process.env.LLVM_OBJDUMP) candidates.push(process.env.LLVM_OBJDUMP);
  try {
    const sysroot = execFileSync('rustc', ['--print', 'sysroot'], { encoding: 'utf8' }).trim();
    const host = /host: (\S+)/.exec(execFileSync('rustc', ['-vV'], { encoding: 'utf8' }))?.[1];
    if (host) candidates.push(join(sysroot, 'lib/rustlib', host, 'bin/llvm-objdump'));
  } catch {
    // no rustc on PATH: fall through to the other candidates
  }
  candidates.push('/opt/homebrew/opt/llvm/bin/llvm-objdump', 'llvm-objdump');
  for (const c of candidates) {
    if (!c.includes('/') || existsSync(c)) {
      try {
        execFileSync(c, ['--version'], { stdio: 'ignore' });
        return c;
      } catch {
        // try the next candidate
      }
    }
  }
  throw new Error('llvm-objdump not found: `rustup component add llvm-tools` or set LLVM_OBJDUMP');
}

/** The asset blob cap the firmware enforces, from its source; null when the crate has none. */
function readBlobCap(crate) {
  try {
    const source = readFileSync(join(crate, 'src/internals.rs'), 'utf8');
    const m = /ASSET_BLOB_CAP_BYTES:\s*u32\s*=\s*([\d_]+)/.exec(source);
    return m ? Number(m[1].replaceAll('_', '')) : null;
  } catch {
    return null;
  }
}

export async function main(argv) {
  const opt = { elf: null, budget: null, json: false, disasm: null };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--json') opt.json = true;
    else if (argv[i] === '--elf') opt.elf = resolve(argv[++i]);
    else if (argv[i] === '--budget') opt.budget = resolve(argv[++i]);
    else if (argv[i] === '--disasm') opt.disasm = resolve(argv[++i]);
    else throw new Error(`unknown argument: ${argv[i]}`);
  }
  // The catalog (needs Bun's TOML parser) names the firmware crate and its artifact.
  let crate = null;
  if (!opt.elf || !opt.budget) {
    const { resolveFirmware } = await import('./build-firmware.mjs');
    const plan = resolveFirmware(repo, process.env.KIVORI_PRODUCT, process.env.KIVORI_BOARD);
    crate = plan.cwd;
    opt.elf ??= plan.artifact;
    opt.budget ??= join(plan.cwd, 'resource-budget.json');
  }
  if (!existsSync(opt.elf)) throw new Error(`ELF not found: ${opt.elf} (build: just fw-build)`);
  const budget = JSON.parse(readFileSync(opt.budget, 'utf8'));
  const measured = {
    ...measureElf(parseElf(readFileSync(opt.elf))),
    blobCapBytes: crate ? readBlobCap(crate) : null,
  };
  const text = opt.disasm
    ? readFileSync(opt.disasm, 'utf8')
    : execFileSync(findObjdump(), ['-d', '--no-show-raw-insn', '-C', opt.elf], {
        encoding: 'utf8',
        maxBuffer: 1 << 30,
      });
  const result = evaluate({ measured, parsed: parseDisassembly(text), budget });
  if (opt.json) {
    console.log(JSON.stringify(result, null, 2));
  } else console.log(renderReport(result, budget, opt.elf));
  return result.violations.length ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (e) {
    console.error(`firmware-budget: ${e.message}`);
    process.exitCode = 2;
  }
}
