import { test } from 'bun:test';
import assert from 'node:assert/strict';
import {
  evaluate,
  interruptStack,
  matches,
  measureElf,
  num,
  parseDisassembly,
  parseElf,
  worstStack,
} from './firmware-budget.mjs';

// Builds `llvm-objdump -d --no-show-raw-insn` text. Each function is [name, [instruction, ...]]
// and an instruction is "mnemonic operands" (2 bytes apart, which is enough for the parser).
function dis(...fns) {
  let addr = 0x42000000;
  const lines = [];
  for (const [name, body] of fns) {
    lines.push('', `${addr.toString(16)} <${name}>:`);
    for (const text of body) {
      const [op, ...rest] = text.split(' ');
      lines.push(`${addr.toString(16)}:      \t${op}${rest.length ? `\t${rest.join(' ')}` : ''}`);
      addr += 4;
    }
  }
  return lines.join('\n');
}

// The address `dis` gives the function at `index` when every function has `size` instructions.
const addrOf = (index, size) => 0x42000000 + index * size * 4;
const hex = (n) => `0x${n.toString(16)}`;

const find = (parsed, name) => parsed.functions.find((f) => f.name === name);

test('num reads hex, negative hex and decimal operands', () => {
  assert.equal(num('0x10'), 16);
  assert.equal(num('-0x10'), -16);
  assert.equal(num('-16'), -16);
  assert.equal(num('sp'), null);
});

test('frame is the sum of every sp adjustment in the prologue, including lui/addi/sub', () => {
  const parsed = parseDisassembly(
    dis(
      ['small', ['addi sp, sp, -0x10', 'sw ra, 0xc(sp)', 'addi sp, sp, 0x10', 'ret']],
      [
        'big',
        [
          'lui a0, 0x2',
          'addi a0, a0, -0x730',
          'sub sp, sp, a0',
          'sw ra, 0x0(sp)',
          'lui a0, 0x2',
          'addi a0, a0, -0x730',
          'add sp, sp, a0',
          'ret',
        ],
      ],
      ['split', ['addi sp, sp, -0x40', 'addi sp, sp, -0x20', 'ret']],
      ['negative-li', ['li t0, -0x1000', 'add sp, sp, t0', 'ret']],
      ['leaf', ['mv a0, a1', 'ret']],
    ),
  );
  assert.equal(find(parsed, 'small').frame, 16);
  assert.equal(find(parsed, 'big').frame, 0x2000 - 0x730);
  assert.equal(find(parsed, 'split').frame, 0x60);
  assert.equal(find(parsed, 'negative-li').frame, 0x1000);
  assert.equal(find(parsed, 'leaf').frame, 0);
});

test('an sp adjustment after the first call is not part of the prologue', () => {
  const parsed = parseDisassembly(
    dis(
      ['callee', ['ret']],
      [
        'f',
        ['addi sp, sp, -0x10', `jal ${hex(addrOf(0, 1))} <callee>`, 'addi sp, sp, -0x500', 'ret'],
      ],
    ),
  );
  assert.equal(find(parsed, 'f').frame, 16);
});

test('local .L labels do not split a function', () => {
  const parsed = parseDisassembly(
    dis(
      ['trap', ['addi sp, sp, -0x40', 'sw a0, 0x20(sp)']],
      ['.Lpcrel_hi0', ['auipc a0, 0x0', 'ret']],
    ),
  );
  assert.equal(parsed.functions.length, 1);
});

test('auipc+jalr is a direct call, a hoisted ROM address is external, a bare register is indirect', () => {
  const parsed = parseDisassembly(
    dis(
      ['callee', ['ret', 'ret']],
      [
        'caller',
        [
          'addi sp, sp, -0x20',
          // pc of this auipc is 0x42000008 + 4; target = pc + 0 + off lands on callee at 0x42000000
          'auipc ra, 0x0',
          'jalr -0xc(ra) <callee>',
          'lui s3, 0x40000',
          'addi s1, s3, 0x68',
          'jalr s1',
          'jalr a0',
          'ret',
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

test('a hoisted register is forgotten once something else writes it', () => {
  const parsed = parseDisassembly(
    dis(['f', ['lui s1, 0x40000', 'lw s1, 0x0(a0)', 'jalr s1', 'ret']]),
  );
  assert.equal(find(parsed, 'f').indirect, 1);
});

test('a jump out of the function is a tail edge; a jump inside it is not', () => {
  const parsed = parseDisassembly(
    dis(
      ['target', ['addi sp, sp, -0x30', 'ret']],
      ['tailer', ['beqz a0, 0x42000010 <tailer+0x8>', `j ${hex(addrOf(0, 2))} <target>`, 'ret']],
      ['viareg', ['auipc t1, 0x0', `jr -0x14(t1) <target>`]],
      ['dispatch', ['jr a5']],
    ),
  );
  assert.deepEqual(
    find(parsed, 'tailer').tails.map((t) => t.name),
    ['target'],
  );
  assert.deepEqual(
    find(parsed, 'viareg').tails.map((t) => t.name),
    ['target'],
  );
  assert.equal(find(parsed, 'dispatch').indirectJumps, 1);
  assert.equal(find(parsed, 'target').tails.length, 0);
});

test('64-bit division libcalls are recorded per calling function', () => {
  const parsed = parseDisassembly(
    dis(
      ['__udivdi3', ['ret', 'ret']],
      ['uses', ['auipc ra, 0x0', 'jalr -0x8(ra) <__udivdi3>', 'ret']],
      ['clean', ['ret']],
    ),
  );
  assert.deepEqual([...find(parsed, 'uses').div64], ['__udivdi3']);
  assert.equal(find(parsed, 'clean').div64.size, 0);
});

// A small graph with real addresses: a -> b (frame 32) -> c (frame 100); a -> d (frame 50).
// c tail-jumps to e (frame 1000): the tail edge adds e but not c's own frame.
function graph() {
  const pad = ['ret', 'ret', 'ret', 'ret'];
  const at = (i) => hex(0x42000000 + i * 16);
  return parseDisassembly(
    dis(
      ['e', ['addi sp, sp, -0x3e8', 'ret', 'ret', 'ret']],
      ['c', ['addi sp, sp, -0x64', 'addi sp, sp, 0x64', `j ${at(0)} <e>`, 'ret']],
      ['b', ['addi sp, sp, -0x20', `jal ${at(1)} <c>`, 'ret', 'ret']],
      ['d', ['addi sp, sp, -0x32', ...pad.slice(1)]],
      ['a', ['addi sp, sp, -0x10', `jal ${at(2)} <b>`, `jal ${at(3)} <d>`, 'ret']],
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
      ['x', ['addi sp, sp, -0x10', `jal ${hex(0x42000010)} <y>`, 'ret', 'ret']],
      ['y', ['addi sp, sp, -0x20', `jal ${hex(0x42000000)} <x>`, 'ret', 'ret']],
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

test('a new 64-bit division caller fails; an allowlisted one passes; a stale entry warns', () => {
  const parsed = parseDisassembly(
    dis(
      ['__udivdi3', ['ret', 'ret', 'ret']],
      ['old_user', ['auipc ra, 0x0', 'jalr -0x4(ra) <__udivdi3>', 'ret']],
      ['new_user', ['auipc ra, 0x0', 'jalr -0x10(ra) <__udivdi3>', 'ret']],
      ['a', ['addi sp, sp, -0x10', 'ret']],
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
  assert.match(r.violations[0], /64-bit division: new_user calls __udivdi3/);
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
  const elf = parseElf(tinyElf({ stackStart: 0x3fcce400, stackEnd: 0x3fca6a7c, blobPayload: 100 }));
  const m = measureElf(elf);
  assert.equal(m.assetBlobBytes, 16 + 8 + 100);
  assert.equal(m.staticRamBytes, 0x3fca6a7c - 0x3fc80000);
  assert.equal(m.stackRegionBytes, 0x3fcce400 - 0x3fca6a7c);
  assert.equal(m.appImageBytes, 8 + 16 + 8 + 100);
});

test('parseElf rejects a file that is not ELF32 little-endian', () => {
  assert.throws(() => parseElf(Buffer.alloc(64)), /not an ELF32/);
});
