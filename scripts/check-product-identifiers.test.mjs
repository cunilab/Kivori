import { expect, test } from 'bun:test';
import { execFileSync, spawnSync } from 'node:child_process';
import {
  chmodSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkProductIdentifiers, withoutRustTests } from './check-product-identifiers.mjs';

function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'kivori identifier guard '));
  const put = (path, text) => {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), text);
  };
  execFileSync('git', ['init', '--quiet'], { cwd: root });
  put('products/registry.toml', '[products]\nlada = 1\nreserved = 7\n[boards]\nrp-zero = 2\n');
  put(
    'products/lada/product.toml',
    `short_name = "Beacon"
display_name = "Kivori Beacon"
model = "KV-L2"
[boards.rp-zero]
display_name = "RP Zero"
[boards.rp-zero.firmware]
crate = "firmware/lada"
artifact = "target/test-none-elf/release/device"
[boards.rp-zero.firmware.build]
target = "test-none-elf"
features = []
`,
  );
  put(
    'firmware/lada/Cargo.toml',
    '[workspace]\n[package]\nname = "test-device"\nversion = "0.1.0"\n',
  );
  const policy = (rules) =>
    put('scripts/product-identifier-exceptions.json', JSON.stringify(rules));
  policy([]);
  return { root, put, policy, close: () => rmSync(root, { recursive: true, force: true }) };
}
const usingFixture = (body) => {
  const f = fixture();
  try {
    body(f);
  } finally {
    f.close();
  }
};

test('catalog ids, names, models, boards, reservations and separator/camel variants are caught', () =>
  usingFixture(({ root, put }) => {
    const forms = [
      'lada',
      'PALA',
      'Beacon',
      'KV-L2',
      'kv_l2',
      'KVL2',
      'RP Zero',
      'rp_zero',
      'RPZero',
      'reserved',
      'isLada',
      'ladaBoard',
      'BEACON_ART',
    ];
    for (const text of forms) {
      put('apps/desktop/src/view.ts', `const value = '${text}';\n`);
      const { violations } = checkProductIdentifiers(root);
      expect(violations.length > 0).toBe(text !== 'PALA'); // Terms come from this catalog, not the shipping one.
    }
  }));

test('ordinary pipeline/pipe names and brand-level text stay clear', () =>
  usingFixture(({ root, put }) => {
    put(
      'apps/desktop/src/view.ts',
      'const names = ["Kivori", "RotaryPipeline", "SimPipe", "BrokenPipe", "BROKEN_PIPE", "ladder"];\n',
    );
    expect(checkProductIdentifiers(root).violations).toEqual([]);
  }));

test('untracked production sources and filenames fail, including binary product art names', () =>
  usingFixture(({ root, put }) => {
    put('apps/desktop/src/view.ts', 'const name = "Beacon";\n');
    expect(checkProductIdentifiers(root).violations[0]).toContain('apps/desktop/src/view.ts:1:');
    put('apps/desktop/src/view.ts', 'const name = "neutral";\n');
    put('apps/desktop/src/lada.png', Buffer.from([0, 1, 2]));
    expect(checkProductIdentifiers(root).violations).toEqual([
      expect.stringContaining('filename contains lada'),
    ]);
  }));

test('test paths, vendor, docs, firmware and unrelated worktrees are outside the shared scope', () =>
  usingFixture(({ root, put }) => {
    for (const path of [
      'apps/desktop/src/test/fixture.ts',
      'apps/desktop/src/__tests__/view.tsx',
      'crates/demo/tests/contract.rs',
      'scripts/guard.test.mjs',
      'tools/asset-compiler/src/view.spec.ts',
      'apps/desktop/src-tauri/vendor/bridge.m',
      'docs/notes.md',
      'firmware/lada/src/main.rs',
      '.claude/worktrees/other/src/view.ts',
    ])
      put(path, 'Beacon lada KV-L2');
    put('apps/desktop/src/view.ts', 'const name = "neutral";');
    expect(checkProductIdentifiers(root).violations).toEqual([]);
  }));

test('balanced Rust test modules are excluded, and production code after each remains checked', () =>
  usingFixture(({ root, put }) => {
    const source = `#[cfg(test)]
mod tests {
    const NAME: &str = "Beacon";
    fn nested() { let brace = '}'; }
}
fn neutral() {}
#[cfg(test)]
pub(crate) mod more_tests { const NAME: &str = "lada"; }
fn isLada() {}
`;
    const masked = withoutRustTests(source);
    expect(masked.split('\n').length).toBe(source.split('\n').length);
    expect(masked).not.toContain('"Beacon"');
    expect(masked).toContain('fn isLada()');
    put('apps/desktop/src-tauri/src/view.rs', source);
    const { violations } = checkProductIdentifiers(root);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toContain(':9: lada:');
  }));

test('Rust comments, raw/byte strings, character literals and lifetimes cannot spoof module bounds', () => {
  const source = `const TEXT: &str = r###"
#[cfg(test)]
mod fake {
    Beacon
}
"###;
#[cfg(test)]
mod tests {
    // } marker in a comment
    /* } /* nested } */ { */
    const RAW: &[u8] = br##" } \\" { "##;
    const STR: &str = "escaped \\" }";
    fn lifetime<'a>(value: &'a str) { let braces = ('{', '}'); }
    const NAME: &str = "lada";
}
fn isBeacon() {}
`;
  const masked = withoutRustTests(source);
  expect(masked).toContain('mod fake'); // A marker inside a production raw string is still scanned.
  expect(masked).toContain('    Beacon');
  expect(masked).not.toContain('const NAME');
  expect(masked).toContain('fn isBeacon()');
});

test('cfg(test) functions are not mistaken for an excluded module, and malformed bounds fail closed', () =>
  usingFixture(({ root, put }) => {
    put('crates/demo/src/lib.rs', '#[cfg(test)]\nfn helper() {}\nfn isLada() {}\n');
    expect(checkProductIdentifiers(root).violations).toHaveLength(1);
    for (const text of [
      '#[cfg(test)]\nmod tests {',
      '/* unfinished',
      'const RAW: &str = r##"unterminated',
      'const STR: &str = "unfinished',
    ])
      expect(() => withoutRustTests(text)).toThrow();
  }));

test('exact exceptions permit only their reviewed line count and reject stale/duplicate/unreasoned rules', () =>
  usingFixture(({ root, put, policy }) => {
    const path = 'apps/desktop/src/legacy.ts';
    const text = 'const legacy = "lada";';
    const rule = { path, text, count: 1, reason: 'Fixed legacy identity fixture.' };
    put(path, text);
    policy([rule]);
    expect(checkProductIdentifiers(root).violations).toEqual([]);
    put(path, `${text}\n${text}\n`);
    expect(checkProductIdentifiers(root).violations[0]).toContain(
      'expected 1 occurrence(s), found 2',
    );
    put(path, 'const legacy = "neutral";');
    expect(checkProductIdentifiers(root).violations[0]).toContain('found 0');
    policy([rule, rule]);
    expect(() => checkProductIdentifiers(root)).toThrow('Duplicate identifier exception');
    policy([{ ...rule, reason: '' }]);
    expect(() => checkProductIdentifiers(root)).toThrow('reason');
    policy([{ path, count: 1, reason: 'Cannot whitelist a file.' }]);
    expect(() => checkProductIdentifiers(root)).toThrow('exact line');
  }));

test('a reviewed legacy line never exempts an added leak elsewhere in that file or on that line', () =>
  usingFixture(({ root, put, policy }) => {
    const path = 'apps/desktop/src/legacy.ts';
    const text = 'const legacy = "lada";';
    policy([{ path, text, count: 1, reason: 'Legacy identity.' }]);
    put(path, `${text}\nconst other = "Beacon";`);
    expect(checkProductIdentifiers(root).violations).toHaveLength(1);
    put(path, text + ' const other = "Beacon";');
    expect(checkProductIdentifiers(root).violations).toHaveLength(2); // Leak plus stale allowance.
  }));

test('positive control: the actual CLI exits nonzero on an injected leak', () =>
  usingFixture(({ root, put }) => {
    put('apps/desktop/src/view.ts', 'const product = "Beacon";');
    const script = new URL('./check-product-identifiers.mjs', import.meta.url).href;
    const result = spawnSync(
      process.execPath,
      ['--eval', `import { main } from ${JSON.stringify(script)}; main(process.argv[1]);`, root],
      { encoding: 'utf8' },
    );
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('apps/desktop/src/view.ts:1:');
    expect(result.stderr).toContain('Product identifier guard FAILED');
  }));

test('the real repository is scanned and clean with explicit reviewed exceptions', () => {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const result = checkProductIdentifiers(root);
  expect(result.checked).toBeGreaterThan(100);
  expect(result.terms).toBeGreaterThan(5);
  expect(result.exceptions).toBeGreaterThan(0);
  expect(result.violations).toEqual([]);
});

test(
  'offline dependency guard visits every catalog workspace and scans a custom second crate',
  { skip: process.platform === 'win32' },
  () =>
    usingFixture(({ root, put }) => {
      put('products/registry.toml', '[products]\nlada = 1\njahe = 2\n[boards]\nrp-zero = 2\n');
      put(
        'products/jahe/product.toml',
        readFileSync(join(root, 'products/lada/product.toml'), 'utf8').replace(
          'firmware/lada',
          'devices/second firmware',
        ),
      );
      put(
        'devices/second firmware/Cargo.toml',
        '[workspace]\n[package]\nname = "test-second"\nversion = "0.1.0"\n',
      );
      put('firmware/lada/src/lib.rs', 'pub const VALUE: u8 = 1;\n');
      put('devices/second firmware/src/lib.rs', 'pub const VALUE: u8 = 1;\n');
      for (const name of ['build-firmware.mjs', 'check-offline-deps.sh'])
        put(`scripts/${name}`, readFileSync(new URL(`./${name}`, import.meta.url), 'utf8'));
      put(
        'stubs/cargo',
        `#!${process.execPath}
import { appendFileSync } from 'node:fs';
appendFileSync(process.env.CARGO_GUARD_LOG, JSON.stringify({ cwd: process.cwd(), args: process.argv.slice(2) }) + "\\n");
if (process.argv[2] === 'metadata') console.log(JSON.stringify({ packages: [{ name: 'test-device', dependencies: [] }] }));
`,
      );
      chmodSync(join(root, 'stubs/cargo'), 0o755);
      const env = {
        ...process.env,
        PATH: `${join(root, 'stubs')}${delimiter}${process.env.PATH}`,
        CARGO_GUARD_LOG: join(root, 'cargo-calls.jsonl'),
      };
      const run = () =>
        spawnSync('bash', ['scripts/check-offline-deps.sh'], { cwd: root, env, encoding: 'utf8' });
      const clean = run();
      expect(clean.stderr).toBe('');
      expect(clean.status).toBe(0);
      const calls = readFileSync(env.CARGO_GUARD_LOG, 'utf8')
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line));
      expect(
        calls
          .filter((call) => call.args[0] === 'metadata')
          .map((call) => relative(realpathSync(root), realpathSync(call.cwd)))
          .sort(),
      ).toEqual(['', 'devices/second firmware', 'firmware/lada']);
      put('devices/second firmware/src/lib.rs', 'use std::net::TcpStream;\n');
      const leak = run();
      expect(leak.status).toBe(1);
      expect(leak.stdout).toContain('devices/second firmware/src/lib.rs');
      expect(leak.stdout).toContain('std::net::TcpStream');
      put('products/jahe/product.toml', 'broken = [');
      expect(run().status).not.toBe(0); // Catalog discovery failure cannot silently become an empty workspace list.
    }),
);

test('Unicode customer names and compound board words derive without adding bare chip tokens', () =>
  usingFixture(({ root, put }) => {
    const path = 'products/lada/product.toml';
    put(
      path,
      readFileSync(join(root, path), 'utf8')
        .replaceAll('Beacon', 'Écho')
        .replace('RP Zero"', 'RP ZeroWing"'),
    );
    for (const value of ['Écho', 'isÉcho', 'isE\u0301cho', 'zero_wing', 'ZeroWing', 'zerowing']) {
      put('apps/desktop/src/view.ts', `const value = '${value}';`);
      expect(checkProductIdentifiers(root).violations.length).toBeGreaterThan(0);
    }
    put('apps/desktop/src/view.ts', 'const chip = "RP";');
    expect(checkProductIdentifiers(root).violations).toEqual([]);
  }));
