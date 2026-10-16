import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, symlinkSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildFirmware, resolveFirmware } from './build-firmware.mjs';

function fixture(run) {
  const root = mkdtempSync(join(tmpdir(), 'kivori build '));
  const put = (path, text) => {
    mkdirSync(join(root, path, '..'), { recursive: true });
    writeFileSync(join(root, path), text);
  };
  const product = (id, board, target = 'test-none-elf', features = ['physical']) => {
    put(
      `products/${id}/product.toml`,
      `[boards.${board}.firmware]\ncrate = 'firmware/${id}'\nartifact = 'target/${target}/release/device'\n[boards.${board}.firmware.build]\ntarget = '${target}'\nfeatures = ${JSON.stringify(features)}\n`,
    );
    put(`firmware/${id}/Cargo.toml`, '[package]\nname="device"\nversion="0.1.0"\n');
  };
  put(
    'products/registry.toml',
    '[products]\nfirst = 41\nsecond = 42\n[boards]\nboard-a = 9\nboard-b = 10\n',
  );
  product('first', 'board-a');
  try {
    run({ root, put, product });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('sole active pair is selected; reserved registry entries are not builds', () =>
  fixture(({ root }) => {
    const plan = resolveFirmware(root);
    assert.equal(plan.product, 'first');
    assert.equal(plan.board, 'board-a');
    assert.deepEqual(plan.args, [
      'build',
      '--locked',
      '--release',
      '--target',
      'test-none-elf',
      '--no-default-features',
      '--bin',
      'device',
      '--features',
      'physical',
    ]);
  }));

test('second product uses its own recipe; ambiguous or partial selection fails', () =>
  fixture(({ root, product }) => {
    product('second', 'board-b', 'x86_64-unknown-linux-gnu', ['input', 'screen']);
    assert.throws(() => resolveFirmware(root), /Select a catalog/);
    assert.throws(() => resolveFirmware(root, 'second'), /both/);
    assert.throws(() => resolveFirmware(root, 'unknown', 'board-b'), /Select a catalog/);
    const plan = resolveFirmware(root, 'second', 'board-b');
    assert.equal(plan.cwd, join(root, 'firmware/second'));
    assert.equal(plan.artifact, join(plan.cwd, 'target/x86_64-unknown-linux-gnu/release/device'));
    assert.equal(plan.args.at(-1), 'input,screen');
  }));

test('missing recipe, invalid tokens and mismatched artifact paths fail before cargo', () =>
  fixture(({ root, put, product }) => {
    put(
      'products/first/product.toml',
      "[boards.board-a.firmware]\ncrate = 'firmware/first'\nartifact = 'anything'\n",
    );
    assert.throws(() => resolveFirmware(root), /explicit build/);
    for (const [target, features] of [
      ['--bad', ['x']],
      ['test-none-elf', ['x,--all-features']],
      ['test-none-elf', ['']],
    ]) {
      product('first', 'board-a', target, features);
      assert.throws(() => resolveFirmware(root), /explicit build/);
    }
    product('first', 'board-a');
    // A valid target must not select a stale artifact from a different target.
    put(
      'products/first/product.toml',
      "[boards.board-a.firmware]\ncrate = 'firmware/first'\nartifact = 'target/old-none-elf/release/device'\n[boards.board-a.firmware.build]\ntarget = 'test-none-elf'\nfeatures = []\n",
    );
    assert.throws(() => resolveFirmware(root), /artifact must/);
  }));

test('recipe types, unknown fields and duplicate features fail before cargo', () =>
  fixture(({ root, put, product }) => {
    const path = 'products/first/product.toml';
    const source = readFileSync(join(root, path), 'utf8');
    for (const invalid of [
      source.replace("target = 'test-none-elf'", "target = ['test-none-elf']"),
      source + 'default_features = true\n',
    ]) {
      put(path, invalid);
      assert.throws(() => resolveFirmware(root), /explicit build/);
    }
    product('first', 'board-a', 'test-none-elf', ['physical', 'physical']);
    assert.throws(() => resolveFirmware(root), /explicit build/);
  }));

test('crate paths cannot leave the repository', () =>
  fixture(({ root, put }) => {
    put(
      'products/first/product.toml',
      "[boards.board-a.firmware]\ncrate = '../'\nartifact = 'target/test-none-elf/release/device'\n[boards.board-a.firmware.build]\ntarget = 'test-none-elf'\nfeatures = []\n",
    );
    assert.throws(() => resolveFirmware(root), /Catalog path/);
  }));

test('build pins target directory, propagates cargo failures and refuses missing output', () =>
  fixture(({ root, put }) => {
    const plan = resolveFirmware(root);
    const run = (status) => (command, args, options) => {
      assert.equal(command, 'cargo');
      assert.deepEqual(args, plan.args);
      assert.equal(options.cwd, plan.cwd);
      assert.equal(options.env.CARGO_TARGET_DIR, join(plan.cwd, 'target'));
      assert.deepEqual(options.stdio, ['inherit', 2, 2]);
      return { status };
    };
    assert.throws(() => buildFirmware(plan, run(1)), /build failed/);
    assert.throws(
      () => buildFirmware(plan, () => ({ status: null, signal: 'SIGTERM' })),
      /SIGTERM/,
    );
    assert.throws(
      () => buildFirmware(plan, () => ({ error: new Error('cargo unavailable') })),
      /unavailable/,
    );
    assert.throws(() => buildFirmware(plan, run(0)), /not produced/);
    put('firmware/first/target/test-none-elf/release/device', 'fixture');
    assert.equal(buildFirmware(plan, run(0)), plan.artifact);
    // A stale file cannot turn an unsuccessful build into success.
    assert.throws(() => buildFirmware(plan, run(2)), /build failed/);
  }));

test('symlinked output outside the crate is refused', () =>
  fixture(({ root, put }) => {
    if (process.platform === 'win32') return; // Windows symlink creation needs privileges; containment is exercised on Unix.
    const plan = resolveFirmware(root);
    put('outside', 'fixture');
    mkdirSync(join(plan.cwd, 'target/test-none-elf/release'), { recursive: true });
    symlinkSync(join(root, 'outside'), plan.artifact);
    assert.throws(() => buildFirmware(plan, () => ({ status: 0 })), /escapes/);
  }));

test('inherited Rust flags cannot replace the target linker configuration', () =>
  fixture(({ root }) => {
    const plan = resolveFirmware(root);
    for (const key of ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS']) {
      const previous = process.env[key];
      process.env[key] = '-Dwarnings';
      try {
        assert.throws(
          () =>
            buildFirmware(plan, () => {
              throw new Error('must not invoke cargo');
            }),
          /Unset RUSTFLAGS/,
        );
      } finally {
        if (previous === undefined) delete process.env[key];
        else process.env[key] = previous;
      }
    }
  }));

test('empty selectors fail and partial CLI flags never borrow environment selectors', () =>
  fixture(({ root }) => {
    assert.throws(() => resolveFirmware(root, '', ''), /nonempty/);
    assert.throws(() => resolveFirmware(root, 'first', ''), /nonempty/);
    const result = spawnSync(
      process.execPath,
      [fileURLToPath(new URL('./build-firmware.mjs', import.meta.url)), '--product', 'first'],
      {
        env: { ...process.env, KIVORI_BOARD: 'board-a' },
        encoding: 'utf8',
      },
    );
    assert.equal(result.status, 1);
    assert.equal(result.stdout, '');
    assert.match(result.stderr, /Select both/);
  }));

test('real child stdout goes to stderr; only the artifact path reaches stdout', () =>
  fixture(({ root, put }) => {
    const plan = resolveFirmware(root);
    put('firmware/first/target/test-none-elf/release/device', 'fixture');
    const code = `import { spawnSync } from 'node:child_process';
    import { buildFirmware } from ${JSON.stringify(new URL('./build-firmware.mjs', import.meta.url).href)};
    const plan = ${JSON.stringify(plan)};
    console.log(buildFirmware(plan, (command, args, options) => spawnSync(process.execPath, ['-e', 'console.log("build output")'], options)));`;
    const result = spawnSync(process.execPath, ['-e', code], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim(), plan.artifact);
    assert.match(result.stderr, /build output/);
  }));
