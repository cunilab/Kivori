import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  checkImageLayout,
  crc32Mpeg2,
  evaluate,
  findObjdump,
  interruptStack,
  matches,
  measureElf,
  num,
  parseDisassembly,
  parseElf,
  worstStack,
} from './firmware-budget.mjs';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');

// Builds `llvm-objdump -d --no-show-raw-insn` text of a Thumb image. Each function is
// [name, [instruction, ...]] and an instruction is "mnemonic operands" (4 bytes apart, which is
// enough for the parser). `.word 0xVALUE` makes a literal-pool word (llvm-objdump prints its bytes).
function dis(...fns) {
  let addr = 0x10000000;
  const lines = [];
  for (const [name, body] of fns) {
    lines.push('', `${addr.toString(16)} <${name}>:`);
    for (const text of body) {
      const [op, ...rest] = text.split(' ');
      if (op === '.word') lines.push(`${addr.toString(16)}: 00 00 00 00  \t.word\t${rest[0]}`);
      else
        lines.push(`${addr.toString(16)}:      \t${op}${rest.length ? `\t${rest.join(' ')}` : ''}`);
      addr += 4;
    }
  }
  return lines.join('\n');
}

// The address `dis` gives the function at `index` when every function has `size` instructions.
const addrOf = (index, size) => 0x10000000 + index * size * 4;
const hex = (n) => `0x${n.toString(16)}`;

const find = (parsed, name) => parsed.functions.find((f) => f.name === name);

test('num reads hex, negative hex and decimal operands', () => {
  assert.equal(num('0x10'), 16);
  assert.equal(num('-0x10'), -16);
  assert.equal(num('-16'), -16);
  assert.equal(num('sp'), null);
});

test('frame is the pushed registers plus every sp allocation in the prologue, including a literal-pool constant', () => {
  const parsed = parseDisassembly(
    dis(
      ['small', ['push {r4, r7, lr}', 'sub sp, #0x10', 'add sp, #0x10', 'pop {r4, r7, pc}']],
      [
        'big',
        [
          'push {r4, r5, r6, r7, lr}',
          // pc-relative load of -0x1000 from the literal below (at 0x10000020)
          'ldr r6, [pc, #0xc] @ 0x10000020 <big+0x10>',
          'add sp, r6',
          'ldr r0, [sp, #0x4]',
          '.word 0xfffff000',
          'pop {r4, r5, r6, r7, pc}',
        ],
      ],
      ['split', ['sub sp, #0x40', 'sub sp, #0x20', 'bx lr']],
      ['shifted', ['movs r3, #0x10', 'lsls r3, r3, #0x8', 'negs r3, r3', 'add sp, r3', 'bx lr']],
      ['leaf', ['mov r0, r1', 'bx lr']],
    ),
  );
  assert.equal(find(parsed, 'small').frame, 12 + 16);
  assert.equal(find(parsed, 'big').frame, 20 + 0x1000);
  assert.equal(find(parsed, 'split').frame, 0x60);
  assert.equal(find(parsed, 'shifted').frame, 0x1000);
  assert.equal(find(parsed, 'leaf').frame, 0);
});

test('an sp adjustment after the first call is not part of the prologue', () => {
  const parsed = parseDisassembly(
    dis(
      ['callee', ['bx lr']],
      ['f', ['sub sp, #0x10', `bl ${hex(addrOf(0, 1))} <callee>`, 'sub sp, #0x50', 'bx lr']],
    ),
  );
  assert.equal(find(parsed, 'f').frame, 16);
});

test('local .L labels do not split a function', () => {
  const parsed = parseDisassembly(
    dis(['trap', ['sub sp, #0x40', 'str r0, [sp, #0x20]']], ['.Lpcrel_hi0', ['nop', 'bx lr']]),
  );
  assert.equal(parsed.functions.length, 1);
});

test('bl is a direct call; blx of a loaded constant is external; blx of a bare register is indirect', () => {
  const parsed = parseDisassembly(
    dis(
      ['callee', ['bx lr', 'bx lr']],
      [
        'caller',
        [
          'sub sp, #0x20',
          `bl ${hex(addrOf(0, 1))} <callee>`,
          // pc of the ldr is 0x10000010; the literal below sits at 0x10000018
          'ldr r3, [pc, #0x4] @ 0x10000018 <caller+0x10>',
          'blx r3',
          '.word 0x00000d1c',
          'blx r2',
          'bx lr',
        ],
      ],
    ),
  );
  const caller = find(parsed, 'caller');
  assert.equal(caller.calls.length, 2);
  assert.equal(caller.calls[0].addr, find(parsed, 'callee').addr);
  assert.equal(caller.external, 1);
  assert.equal(caller.indirect, 1);
});

test('a bl into a far block of the same function is a jump, not a recursive call', () => {
  const parsed = parseDisassembly(
    dis([
      'f',
      ['sub sp, #0x10', `bl ${hex(0x10000010)} <f+0x10>`, 'bx lr', 'bx lr', 'nop', 'bx lr'],
    ]),
  );
  assert.equal(find(parsed, 'f').calls.length, 0);
});

test('a hoisted register is forgotten once something else writes it', () => {
  const parsed = parseDisassembly(
    dis([
      'f',
      [
        'ldr r4, [pc, #0x8] @ 0x1000000c <f+0xc>',
        'ldr r4, [r0]',
        'blx r4',
        '.word 0x00000d1c',
        'bx lr',
      ],
    ]),
  );
  assert.equal(find(parsed, 'f').indirect, 1);
});

test('a jump out of the function is a tail edge; a jump inside it is not', () => {
  const parsed = parseDisassembly(
    dis(
      ['target', ['sub sp, #0x30', 'bx lr']],
      ['tailer', [`beq 0x10000018 <tailer+0x8>`, `b ${hex(addrOf(0, 2))} <target>`, 'bx lr']],
      ['dispatch', ['bx r5']],
    ),
  );
  assert.deepEqual(
    find(parsed, 'tailer').tails.map((t) => t.name),
    ['target'],
  );
  assert.equal(find(parsed, 'dispatch').indirectJumps, 1);
  assert.equal(find(parsed, 'target').tails.length, 0);
});

test('64-bit division libcalls are recorded per calling function', () => {
  const parsed = parseDisassembly(
    dis(
      ['__aeabi_uldivmod', ['bx lr', 'bx lr']],
      ['uses', [`bl ${hex(addrOf(0, 2))} <__aeabi_uldivmod>`, 'bx lr']],
      ['clean', ['bx lr']],
    ),
  );
  assert.deepEqual([...find(parsed, 'uses').div64], ['__aeabi_uldivmod']);
  assert.equal(find(parsed, 'clean').div64.size, 0);
});

// A small graph with real addresses: a -> b (frame 32) -> c (frame 100); a -> d (frame 50).
// c tail-jumps to e (frame 1000): the tail edge adds e but not c's own frame.
function graph() {
  const pad = ['bx lr', 'bx lr', 'bx lr', 'bx lr'];
  const at = (i) => hex(0x10000000 + i * 16);
  return parseDisassembly(
    dis(
      ['e', ['sub sp, #0x1f4', 'sub sp, #0x1f4', 'bx lr', 'bx lr']],
      ['c', ['sub sp, #0x64', 'add sp, #0x64', `b ${at(0)} <e>`, 'bx lr']],
      ['b', ['sub sp, #0x20', `bl ${at(1)} <c>`, 'bx lr', 'bx lr']],
      ['d', ['sub sp, #0x32', ...pad.slice(1)]],
      ['a', ['sub sp, #0x10', `bl ${at(2)} <b>`, `bl ${at(3)} <d>`, 'bx lr']],
    ),
  );
}

test('worstStack follows calls by summed frames and tail jumps without the caller frame', () => {
  const r = worstStack(graph(), 'a');
  assert.equal(r.depth, 0x10 + 0x20 + 0x3e8);
  assert.deepEqual(
    r.chain.map((c) => c.name),
    ['a', 'b', 'c', 'e'],
  );
  assert.equal(r.chain[3].tail, true);
  assert.equal(worstStack(graph(), 'nope'), null);
});

test('worstStack cuts recursion and reports it', () => {
  const parsed = parseDisassembly(
    dis(
      ['x', ['sub sp, #0x10', `bl ${hex(0x10000010)} <y>`, 'bx lr', 'bx lr']],
      ['y', ['sub sp, #0x20', `bl ${hex(0x10000000)} <x>`, 'bx lr', 'bx lr']],
    ),
  );
  const r = worstStack(parsed, 'x');
  assert.equal(r.depth, 0x30);
  assert.deepEqual(r.cycles, ['x']);
});

test('interruptStack sums the frames down the path and the last function whole', () => {
  const parsed = graph();
  assert.equal(interruptStack(parsed, ['a', 'b']).bytes, 0x10 + (0x20 + 0x3e8));
  assert.deepEqual(interruptStack(parsed, ['a', 'missing']).missing, ['missing']);
});

test('matches supports * and escapes regex characters', () => {
  assert.ok(matches('a::b<*>::c', 'a::b<X, Y>::c'));
  assert.ok(matches('exact', 'exact'));
  assert.ok(!matches('exact', 'exact2'));
  assert.ok(!matches('a.b', 'axb'));
  assert.ok(matches('x(*)', 'x(1)'));
});

// ---- budget evaluation ----

const BUDGET = {
  entry: 'a',
  limits: {
    app_image_bytes: { max: 1000 },
    asset_blob_bytes: { max: 500 },
    static_ram_bytes: { max: 4000 },
    stack_region_bytes: { min: 1000 },
    default_frame_bytes: { max: 100 },
    worst_stack_bytes: { max: 2000 },
    indirect_allowance_bytes: { value: 64 },
    interrupt_margin_bytes: { value: 16 },
    stack_margin_bytes: { value: 100 },
  },
  interrupt_path: ['d'],
  large_frames: [{ fn: 'e', max: 1000 }],
  int64_division_allowlist: [],
};
const MEASURED = {
  appImageBytes: 900,
  assetBlobBytes: 400,
  blobCapBytes: 450,
  staticRamBytes: 3000,
  stackRegionBytes: 2500,
};

test('a budget that fits reports no violations', () => {
  const r = evaluate({ measured: MEASURED, parsed: graph(), budget: BUDGET });
  assert.deepEqual(r.violations, []);
  assert.equal(r.stack.directBytes, 1048);
  assert.equal(r.stack.peakBytes, 1048 + 64 + 0x32 + 16);
});

test('each limit is enforced', () => {
  const run = (measured, budget = BUDGET) =>
    evaluate({ measured: { ...MEASURED, ...measured }, parsed: graph(), budget }).violations.join(
      '\n',
    );
  assert.match(run({ appImageBytes: 1001 }), /app image/);
  assert.match(run({ assetBlobBytes: 501 }), /asset blob/);
  assert.match(run({ assetBlobBytes: 460 }), /cap in internals\.rs/);
  assert.match(run({ staticRamBytes: 4001 }), /static RAM/);
  assert.match(run({ stackRegionBytes: 999 }), /below the budget minimum/);
  // peak 1178 B + 100 B margin = 1278 B
  assert.match(run({ stackRegionBytes: 1277 }), /exceeds the 1277 B stack region/);
  assert.equal(run({ stackRegionBytes: 1278 }), '');
  const tight = { ...BUDGET, limits: { ...BUDGET.limits, worst_stack_bytes: { max: 1000 } } };
  assert.match(run({}, tight), /direct worst-case stack/);
});

test('a big frame must be listed and stay under its ceiling', () => {
  const unlisted = { ...BUDGET, large_frames: [] };
  const r1 = evaluate({ measured: MEASURED, parsed: graph(), budget: unlisted });
  assert.match(r1.violations.join('\n'), /stack frame: e uses 1000 B, above the 100 B default/);
  const low = { ...BUDGET, large_frames: [{ fn: 'e', max: 900 }] };
  const r2 = evaluate({ measured: MEASURED, parsed: graph(), budget: low });
  assert.match(r2.violations.join('\n'), /above its budget of 900 B/);
});

test('a missing interrupt path function is a violation', () => {
  const budget = { ...BUDGET, interrupt_path: ['d', 'gone'] };
  const r = evaluate({ measured: MEASURED, parsed: graph(), budget });
  assert.match(r.violations.join('\n'), /interrupt_path function not found.*gone/);
});

test('the deepest of several interrupt paths sets the interrupt allowance, whatever their order', () => {
  // graph(): d is 0x32 B; a then b is a's frame 0x10 plus b's whole chain (0x20 + c 0x64... + e),
  // 0x10 + 0x20 + 0x3e8 = 1048 B. The deeper one must win from either position.
  const shallow = 0x32;
  const deep = 0x10 + 0x20 + 0x3e8;
  assert.equal(interruptStack(graph(), ['d']).bytes, shallow);
  assert.equal(interruptStack(graph(), ['a', 'b']).bytes, deep);
  for (const interrupt_paths of [
    [['d'], ['a', 'b']],
    [['a', 'b'], ['d']],
  ]) {
    const budget = { ...BUDGET, interrupt_path: undefined, interrupt_paths };
    const r = evaluate({ measured: MEASURED, parsed: graph(), budget });
    assert.equal(r.stack.interruptPathBytes, deep);
    // The allowance is the deepest path plus the margin: paths at one priority do not add up.
    assert.equal(r.stack.interruptBytes, deep + BUDGET.limits.interrupt_margin_bytes.value);
  }
  // A path that is not found is reported even when another path is deeper.
  const budget = { ...BUDGET, interrupt_path: undefined, interrupt_paths: [['a', 'b'], ['gone']] };
  const r = evaluate({ measured: MEASURED, parsed: graph(), budget });
  assert.match(r.violations.join('\n'), /interrupt_path function not found.*gone/);
  assert.equal(r.stack.interruptPathBytes, deep);
});

test('a new 64-bit division caller fails; an allowlisted one passes; a stale entry warns', () => {
  const parsed = parseDisassembly(
    dis(
      ['__aeabi_uldivmod', ['bx lr', 'bx lr', 'bx lr']],
      ['old_user', [`bl ${hex(0x10000000)} <__aeabi_uldivmod>`, 'bx lr']],
      ['new_user', [`bl ${hex(0x10000000)} <__aeabi_uldivmod>`, 'bx lr']],
      ['a', ['sub sp, #0x10', 'bx lr']],
    ),
  );
  const budget = {
    ...BUDGET,
    interrupt_path: [],
    large_frames: [],
    int64_division_allowlist: [{ fn: 'old_user' }, { fn: 'long_gone' }],
  };
  const r = evaluate({ measured: MEASURED, parsed, budget });
  assert.equal(r.violations.length, 1);
  assert.match(r.violations[0], /64-bit division: new_user calls __aeabi_uldivmod/);
  assert.match(r.warnings.join('\n'), /long_gone/);
});

// ---- ELF facts ----

// A minimal ELF32-LE: .rodata (holding an asset blob), .symtab/.strtab with the stack symbols, and
// section names. Only what parseElf reads.
function tinyElf({ stackStart, stackEnd, blobPayload }) {
  const manifest = 8;
  const blob = Buffer.alloc(16 + manifest + blobPayload);
  blob.writeUInt32LE(0x5353414b, 0);
  blob.writeUInt16LE(2, 4);
  blob.writeUInt32LE(manifest, 8);
  blob.writeUInt32LE(blobPayload, 12);
  const rodata = Buffer.concat([Buffer.alloc(8), blob]);
  const names = Buffer.from('\0.rodata\0.symtab\0.strtab\0.shstrtab\0');
  const nameOf = (n) => names.indexOf(`${n}\0`);
  const strtab = Buffer.from('\0_stack_start\0_stack_end\0');
  const sym = (nameOff, value) => {
    const b = Buffer.alloc(16);
    b.writeUInt32LE(nameOff, 0);
    b.writeUInt32LE(value, 4);
    return b;
  };
  const symtab = Buffer.concat([sym(0, 0), sym(1, stackStart), sym(14, stackEnd)]);
  const header = Buffer.alloc(52);
  header.writeUInt32BE(0x7f454c46, 0);
  header[4] = 1;
  header[5] = 1;
  const parts = [header, rodata, symtab, strtab, names];
  const offsets = [];
  let off = 0;
  for (const p of parts) {
    offsets.push(off);
    off += p.length;
  }
  const sections = [
    { name: '', type: 0, flags: 0, addr: 0, off: 0, size: 0, link: 0 },
    {
      name: '.rodata',
      type: 1,
      flags: 2,
      addr: 0x3c000000,
      off: offsets[1],
      size: rodata.length,
      link: 0,
    },
    { name: '.symtab', type: 2, flags: 0, addr: 0, off: offsets[2], size: symtab.length, link: 3 },
    { name: '.strtab', type: 3, flags: 0, addr: 0, off: offsets[3], size: strtab.length, link: 0 },
    { name: '.shstrtab', type: 3, flags: 0, addr: 0, off: offsets[4], size: names.length, link: 0 },
  ];
  const table = Buffer.alloc(sections.length * 40);
  sections.forEach((s, i) => {
    const o = i * 40;
    table.writeUInt32LE(s.name ? nameOf(s.name) : 0, o);
    table.writeUInt32LE(s.type, o + 4);
    table.writeUInt32LE(s.flags, o + 8);
    table.writeUInt32LE(s.addr, o + 12);
    table.writeUInt32LE(s.off, o + 16);
    table.writeUInt32LE(s.size, o + 20);
    table.writeUInt32LE(s.link, o + 24);
  });
  header.writeUInt32LE(off, 32); // e_shoff
  header.writeUInt16LE(40, 46);
  header.writeUInt16LE(sections.length, 48);
  header.writeUInt16LE(4, 50);
  return Buffer.concat([...parts, table]);
}

test('parseElf and measureElf read the image, asset blob, static RAM and stack region', () => {
  const elf = parseElf(tinyElf({ stackStart: 0x20040000, stackEnd: 0x20024000, blobPayload: 100 }));
  const m = measureElf(elf);
  assert.equal(m.assetBlobBytes, 16 + 8 + 100);
  assert.equal(m.staticRamBytes, 0x20024000 - 0x20000000);
  assert.equal(m.stackRegionBytes, 0x20040000 - 0x20024000);
  assert.equal(m.appImageBytes, 8 + 16 + 8 + 100);
});

test('parseElf rejects a file that is not ELF32 little-endian', () => {
  assert.throws(() => parseElf(Buffer.alloc(64)), /not an ELF32/);
});

// ---- the committed budget and the real firmware ----

const FIRMWARE = join(repo, 'firmware/pip');
const ELF = join(FIRMWARE, 'target/thumbv6m-none-eabi/release/kivori-firmware');
const committed = () => JSON.parse(readFileSync(join(FIRMWARE, 'resource-budget.json'), 'utf8'));

test('the committed budget names the RP2040 interrupt handlers and sets every limit the check reads', () => {
  const budget = committed();
  assert.equal(budget.entry, 'main');
  // Two handlers at one priority: the rotary GPIO interrupt and USB. All must be traced.
  assert.deepEqual(budget.interrupt_paths, [['IO_IRQ_BANK0'], ['USBCTRL_IRQ']]);
  for (const key of [
    'app_image_bytes',
    'asset_blob_bytes',
    'static_ram_bytes',
    'stack_region_bytes',
    'default_frame_bytes',
    'worst_stack_bytes',
    'indirect_allowance_bytes',
    'interrupt_margin_bytes',
    'stack_margin_bytes',
  ])
    assert.ok(budget.limits[key], `limit ${key}`);
  const L = budget.limits;
  // The flash is 2 MB and the SRAM 264 KB; a limit beyond them could never trip.
  assert.ok(L.app_image_bytes.max < 2 * 1024 * 1024);
  assert.ok(L.static_ram_bytes.max < 264 * 1024);
  assert.ok(L.stack_margin_bytes.value > 0);
});

test('measurements over the committed budget fail, each by name', () => {
  const budget = committed();
  const L = budget.limits;
  const ok = {
    appImageBytes: L.app_image_bytes.max,
    assetBlobBytes: L.asset_blob_bytes.max,
    blobCapBytes: null,
    staticRamBytes: L.static_ram_bytes.max,
    stackRegionBytes: L.stack_region_bytes.min,
  };
  // The stack numbers come from a parsed disassembly; a trivial one with the entry and every
  // handler keeps this about the measured limits.
  const parsed = parseDisassembly(
    dis(
      ['main', ['sub sp, #0x10', 'bx lr']],
      ['IO_IRQ_BANK0', ['sub sp, #0x20', 'bx lr']],
      ['USBCTRL_IRQ', ['sub sp, #0x40', 'bx lr']],
    ),
  );
  const run = (over) => evaluate({ measured: { ...ok, ...over }, parsed, budget });
  assert.deepEqual(run({}).violations, []);
  assert.equal(run({}).stack.interruptPathBytes, 0x40, 'the deeper handler counts');
  assert.match(run({ appImageBytes: ok.appImageBytes + 1 }).violations.join(), /app image/);
  assert.match(run({ assetBlobBytes: ok.assetBlobBytes + 1 }).violations.join(), /asset blob/);
  assert.match(run({ staticRamBytes: ok.staticRamBytes + 1 }).violations.join(), /static RAM/);
  assert.match(
    run({ stackRegionBytes: ok.stackRegionBytes - 1 }).violations.join(),
    /stack region/,
  );
});

const elfBuilt = existsSync(ELF);

test.skipIf(!elfBuilt)('the built firmware is within its committed budget', () => {
  const measured = { ...measureElf(parseElf(readFileSync(ELF))), blobCapBytes: null };
  const text = execFileSync(findObjdump(), ['-d', '--no-show-raw-insn', '-C', ELF], {
    encoding: 'utf8',
    maxBuffer: 1 << 30,
  });
  const result = evaluate({ measured, parsed: parseDisassembly(text), budget: committed() });
  assert.deepEqual(result.violations, []);
});

test.skipIf(!elfBuilt)('the built firmware has the layout the RP2040 boot ROM needs', () => {
  const { facts, violations } = checkImageLayout(readFileSync(ELF));
  assert.deepEqual(violations, []);
  assert.equal(facts.boot2Addr, 0x10000000);
  assert.equal(facts.vectorTableAddr, 0x10000100);
  assert.ok(facts.dataLma >= 0x10000100 && facts.dataLma < 0x10200000);
});

// ---- image layout ----

test('crc32Mpeg2 is the boot ROM checksum', () => {
  // The standard check value of CRC-32/MPEG-2.
  assert.equal(crc32Mpeg2(Buffer.from('123456789')), 0x0376e6e7);
  assert.equal(crc32Mpeg2(Buffer.alloc(0)), 0xffffffff);
});

// An ELF32-LE with what checkImageLayout reads: sections .boot2, .vector_table and .data, symbols
// `_stack_start` and `__sidata`, an entry point and PT_LOAD program headers.
function layoutElf({
  boot2Addr = 0x10000000,
  boot2Size = 256,
  corruptBoot2 = false,
  vectorAddr = 0x10000100,
  sp = 0x20042000,
  reset = 0x100001c1,
  entry = reset,
  stackStart = 0x20042000,
  dataAddr = 0x20000000,
  dataSize = 0x40,
  sidata = 0x10040000,
  loads = [
    { vaddr: 0x10000000, paddr: 0x10000000, filesz: 0x100, memsz: 0x100 },
    { vaddr: 0x10000100, paddr: 0x10000100, filesz: 0x2000, memsz: 0x2000 },
    { vaddr: 0x20000000, paddr: 0x10040000, filesz: 0x40, memsz: 0x40 },
  ],
} = {}) {
  const boot2 = Buffer.alloc(boot2Size);
  boot2.fill(0x5a);
  if (boot2Size >= 4)
    boot2.writeUInt32LE(crc32Mpeg2(boot2.subarray(0, boot2Size - 4)), boot2Size - 4);
  if (corruptBoot2) boot2[10] ^= 1;
  const vectors = Buffer.alloc(8);
  vectors.writeUInt32LE(sp, 0);
  vectors.writeUInt32LE(reset, 4);
  const data = Buffer.alloc(dataSize);
  const names = Buffer.from('\0.boot2\0.vector_table\0.data\0.symtab\0.strtab\0.shstrtab\0');
  const nameOf = (n) => names.indexOf(`${n}\0`);
  const strtab = Buffer.from('\0_stack_start\0__sidata\0');
  const sym = (nameOff, value) => {
    const b = Buffer.alloc(16);
    b.writeUInt32LE(nameOff, 0);
    b.writeUInt32LE(value, 4);
    return b;
  };
  const symtab = Buffer.concat([sym(0, 0), sym(1, stackStart), sym(14, sidata)]);
  const header = Buffer.alloc(52);
  header.writeUInt32BE(0x7f454c46, 0);
  header[4] = 1;
  header[5] = 1;
  header.writeUInt32LE(entry, 24);
  const phdrs = Buffer.alloc(loads.length * 32);
  loads.forEach((l, i) => {
    const o = i * 32;
    phdrs.writeUInt32LE(1, o);
    phdrs.writeUInt32LE(l.vaddr, o + 8);
    phdrs.writeUInt32LE(l.paddr, o + 12);
    phdrs.writeUInt32LE(l.filesz, o + 16);
    phdrs.writeUInt32LE(l.memsz, o + 20);
  });
  const parts = [header, phdrs, boot2, vectors, data, symtab, strtab, names];
  const offsets = [];
  let off = 0;
  for (const p of parts) {
    offsets.push(off);
    off += p.length;
  }
  const sections = [
    { name: '', type: 0, addr: 0, off: 0, size: 0, link: 0 },
    { name: '.boot2', type: 1, addr: boot2Addr, off: offsets[2], size: boot2.length, link: 0 },
    { name: '.vector_table', type: 1, addr: vectorAddr, off: offsets[3], size: 8, link: 0 },
    { name: '.data', type: 1, addr: dataAddr, off: offsets[4], size: dataSize, link: 0 },
    { name: '.symtab', type: 2, addr: 0, off: offsets[5], size: symtab.length, link: 5 },
    { name: '.strtab', type: 3, addr: 0, off: offsets[6], size: strtab.length, link: 0 },
    { name: '.shstrtab', type: 3, addr: 0, off: offsets[7], size: names.length, link: 0 },
  ];
  const table = Buffer.alloc(sections.length * 40);
  sections.forEach((s, i) => {
    const o = i * 40;
    table.writeUInt32LE(s.name ? nameOf(s.name) : 0, o);
    table.writeUInt32LE(s.type, o + 4);
    table.writeUInt32LE(2, o + 8); // SHF_ALLOC
    table.writeUInt32LE(s.addr, o + 12);
    table.writeUInt32LE(s.off, o + 16);
    table.writeUInt32LE(s.size, o + 20);
    table.writeUInt32LE(s.link, o + 24);
  });
  header.writeUInt32LE(header.length, 28); // e_phoff: right after the header
  header.writeUInt32LE(off, 32); // e_shoff
  header.writeUInt16LE(32, 42);
  header.writeUInt16LE(loads.length, 44);
  header.writeUInt16LE(40, 46);
  header.writeUInt16LE(sections.length, 48);
  header.writeUInt16LE(sections.length - 1, 50);
  return Buffer.concat([...parts, table]);
}

const layoutProblems = (options) => checkImageLayout(layoutElf(options)).violations.join('\n');

test('a correct RP2040 image layout has no violations', () => {
  const { facts, violations } = checkImageLayout(layoutElf());
  assert.deepEqual(violations, []);
  assert.equal(facts.dataLma, 0x10040000);
  assert.equal(facts.entry, 0x100001c1);
});

test('boot2 must be 256 bytes at the start of flash with a valid CRC', () => {
  assert.match(layoutProblems({ corruptBoot2: true }), /boot2 CRC/);
  assert.match(layoutProblems({ boot2Addr: 0x10000100 }), /\.boot2 is 256 B at 0x10000100/);
  assert.match(layoutProblems({ boot2Size: 252 }), /\.boot2 is 252 B/);
});

test('the vector table must follow boot2 and point the stack and reset into RAM and flash', () => {
  assert.match(layoutProblems({ vectorAddr: 0x10000000 }), /vector table is at 0x10000000/);
  assert.match(layoutProblems({ sp: 0x20042004, stackStart: 0x20042000 }), /not _stack_start/);
  assert.match(layoutProblems({ sp: 0x30000000, stackStart: 0x30000000 }), /outside RAM/);
  assert.match(layoutProblems({ reset: 0x100001c0 }), /lacks the Thumb bit/);
  assert.match(
    layoutProblems({ reset: 0x20000101, entry: 0x20000101 }),
    /reset vector .* not in flash/,
  );
  assert.match(layoutProblems({ entry: 0x100002c1 }), /entry .* is not the reset vector/);
});

test('.data must be stored in flash and __sidata must say where', () => {
  // Stored in RAM (an image with no data in flash): the initial values would be lost at reset.
  const inRam = [
    { vaddr: 0x10000000, paddr: 0x10000000, filesz: 0x100, memsz: 0x100 },
    { vaddr: 0x20000000, paddr: 0x20000000, filesz: 0x40, memsz: 0x40 },
  ];
  assert.match(layoutProblems({ loads: inRam }), /outside the 2 MB flash/);
  assert.match(layoutProblems({ loads: inRam }), /\.data .* not loaded from flash/);
  // Past the end of the 2 MB flash.
  assert.match(
    layoutProblems({
      sidata: 0x101fffe0,
      loads: [{ vaddr: 0x20000000, paddr: 0x101fffe0, filesz: 0x40, memsz: 0x40 }],
    }),
    /outside the 2 MB flash/,
  );
  assert.match(layoutProblems({ sidata: 0x10050000 }), /__sidata 0x10050000 is not/);
});
