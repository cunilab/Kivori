import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { listFirmwarePlans, resolveFirmware } from './build-firmware.mjs';
import { buildFirmwareSet, writeFirmwareManifest } from './build-firmware-set.mjs';

function fixture(run) {
  const root = mkdtempSync(join(tmpdir(), 'kivori set '));
  const put = (path, text) => {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), text);
  };
  const board = (id, feature, target = 'test-none-elf') =>
    `[boards.${id}.firmware]\ncrate = 'firmware/shared'\nartifact = 'target/${target}/release/device'\n[boards.${id}.firmware.build]\ntarget = '${target}'\nfeatures = ['${feature}']\n`;
  put(
    'products/registry.toml',
    '[products]\nzeta = 41\nalpha = 42\nreserved = 43\n[boards]\na = 9\nb = 10\n',
  );
  put('products/zeta/product.toml', board('a', 'first') + board('b', 'second'));
  put('products/alpha/product.toml', board('b', 'third'));
  put('firmware/shared/Cargo.toml', '[package]\nname="device"\nversion="0.1.0"\n');
  const plans = () => listFirmwarePlans(root);
  const out = join(root, 'images');
  const manifest = join(root, 'manifest.json');
  let calls = 0;
  const cargo = (command, args, options) => {
    const plan = plans()[calls++];
    assert.equal(command, 'cargo');
    assert.deepEqual(args, plan.args);
    assert.equal(options.env.CARGO_TARGET_DIR, plan.targetDir);
    if (calls > 1) {
      const previous = plans()[calls - 2];
      assert.equal(
        readFileSync(
          join(out, `firmware-${previous.productWire}-${previous.boardWire}.elf`),
          'utf8',
        ),
        previous.args.at(-1),
      );
    }
    mkdirSync(dirname(plan.artifact), { recursive: true });
    writeFileSync(plan.artifact, plan.args.at(-1));
    return { status: 0 };
  };
  try {
    run({ root, put, board, plans, out, manifest, cargo, calls: () => calls });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('plans use all active pairs in wire order, including multiple boards; selectors stay explicit', () =>
  fixture(({ root, plans }) => {
    assert.deepEqual(
      plans().map((p) => [p.product, p.board, p.productWire, p.boardWire]),
      [
        ['zeta', 'a', 41, 9],
        ['zeta', 'b', 41, 10],
        ['alpha', 'b', 42, 10],
      ],
    );
    assert.throws(() => resolveFirmware(root), /Select a catalog/);
    assert.equal(resolveFirmware(root, 'alpha', 'b').productWire, 42);
  }));

test('same-artifact builds are snapshotted immediately; manifest contains absolute ordered images', () =>
  fixture(({ root, plans, out, manifest, cargo, calls }) => {
    assert.equal(buildFirmwareSet(root, out, manifest, cargo), manifest);
    assert.equal(calls(), 3);
    const images = JSON.parse(readFileSync(manifest, 'utf8'));
    assert.ok(images.every(isAbsolute));
    assert.deepEqual(
      images.map((file) => readFileSync(file, 'utf8')),
      ['first', 'second', 'third'],
    );
    assert.equal(new Set(plans().map((p) => p.artifact)).size, 1);
    assert.equal(readdirSync(out).length, 3, 'manifest stays outside transported files');
  }));

test('invalid full catalogs fail before Cargo or output, even when selecting a different pair', () =>
  fixture(({ root, put, plans, out, manifest }) => {
    put(
      'products/alpha/product.toml',
      "[boards.b.firmware]\ncrate='firmware/shared'\nartifact='bad'\n",
    );
    assert.throws(plans, /explicit build/);
    assert.throws(() => resolveFirmware(root, 'zeta', 'a'), /explicit build/);
    assert.throws(
      () => buildFirmwareSet(root, out, manifest, () => assert.fail('must not build')),
      /explicit build/,
    );
    assert.ok(!existsSync(out));
    assert.ok(!existsSync(manifest));
  }));

test('empty catalogs and invalid or duplicate wire pairs cannot produce a set', () =>
  fixture(({ root, put, plans }) => {
    for (const number of ['0', '65536', '1.5']) {
      put('products/registry.toml', `[products]\nzeta=${number}\nalpha=42\n[boards]\na=9\nb=10\n`);
      assert.throws(plans, /wire|Unregistered/);
    }
    put('products/registry.toml', '[products]\nzeta=41\nalpha=41\n[boards]\na=9\nb=10\n');
    assert.throws(plans, /Duplicate/);
    put(
      'products/registry.toml',
      '[products]\nzeta=41\nalpha=42\nreserved=41\n[boards]\na=9\nb=10\n',
    );
    assert.throws(plans, /Duplicate/);
    put('products/registry.toml', '[products]\nzeta=41\nalpha=42\n[boards]\na=9\nb=10\n');
    rmSync(join(root, 'products/alpha'), { recursive: true });
    rmSync(join(root, 'products/zeta'), { recursive: true });
    assert.throws(plans, /no active/);
  }));

test('later build failures never publish a manifest or reuse stale directories', () =>
  fixture(({ root, out, manifest, cargo, calls }) => {
    assert.throws(
      () =>
        buildFirmwareSet(root, out, manifest, (...args) =>
          calls() === 1 ? { status: 1 } : cargo(...args),
        ),
      /build failed/,
    );
    assert.equal(calls(), 1);
    assert.ok(!existsSync(manifest));
    assert.equal(readdirSync(out).length, 1);
    assert.throws(
      () => buildFirmwareSet(root, out, manifest, () => assert.fail('must not build')),
      /EEXIST/,
    );
    writeFileSync(manifest, 'stale');
    assert.throws(
      () =>
        buildFirmwareSet(root, join(root, 'other'), manifest, () => assert.fail('must not build')),
      /already exists/,
    );
    assert.equal(readFileSync(manifest, 'utf8'), 'stale');
  }));

test('transport recreates host paths and refuses missing, extra, empty and nonregular entries', () =>
  fixture(({ root, out, manifest, cargo }) => {
    buildFirmwareSet(root, out, manifest, cargo);
    const files = readdirSync(out);
    const receiver = (name) => {
      const dir = join(root, name);
      mkdirSync(dir);
      for (const file of files) copyFileSync(join(out, file), join(dir, file));
      return dir;
    };
    const received = receiver('download with spaces');
    const localManifest = join(root, 'receiver.json');
    writeFirmwareManifest(root, received, localManifest);
    assert.deepEqual(
      JSON.parse(readFileSync(localManifest, 'utf8')).map((p) => readFileSync(p, 'utf8')),
      ['first', 'second', 'third'],
    );
    assert.ok(
      JSON.parse(readFileSync(localManifest, 'utf8')).every((p) =>
        p.includes('download with spaces'),
      ),
    );
    const mutations = [
      (dir) => rmSync(join(dir, files[0])),
      (dir) => writeFileSync(join(dir, 'extra.elf'), 'x'),
      (dir) => writeFileSync(join(dir, 'manifest.json'), 'foreign paths'),
      (dir) => {
        rmSync(join(dir, files[0]));
        mkdirSync(join(dir, files[0]));
      },
      (dir) => writeFileSync(join(dir, files[0]), ''),
    ];
    if (process.platform !== 'win32')
      mutations.push((dir) => {
        rmSync(join(dir, files[0]));
        symlinkSync(join(out, files[0]), join(dir, files[0]));
      });
    mutations.forEach((mutate, index) => {
      const dir = receiver(`bad${index}`);
      mutate(dir);
      const output = join(root, `bad${index}.json`);
      assert.throws(() => writeFirmwareManifest(root, dir, output));
      assert.ok(!existsSync(output));
    });
    assert.throws(
      () => writeFirmwareManifest(root, received, join(received, 'inside.json')),
      /outside/,
    );
  }));

test('set CLI ignores single-pair selectors; conflicting flags fail without stdout', () => {
  const script = fileURLToPath(new URL('./build-firmware-set.mjs', import.meta.url));
  const root = resolve(dirname(script), '..');
  const result = spawnSync(process.execPath, [script, '--targets'], {
    encoding: 'utf8',
    env: { ...process.env, KIVORI_PRODUCT: 'wrong', KIVORI_BOARD: 'wrong' },
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    result.stdout.trim(),
    [...new Set(listFirmwarePlans(root).map((p) => p.target))].sort().join('\n'),
  );
  for (const args of [
    ['--targets', '--from-dir', '/tmp'],
    ['--out-dir', '/tmp', '--from-dir', '/tmp'],
    ['--product', 'zeta'],
  ]) {
    const invalid = spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });
    assert.equal(invalid.status, 1);
    assert.equal(invalid.stdout, '');
  }
});

test('child logs stay on stderr and failed set builds print no manifest', () =>
  fixture(({ root, plans, out, manifest, put }) => {
    const plan = plans()[0];
    put('firmware/shared/target/test-none-elf/release/device', 'fixture');
    const code = `import { spawnSync } from 'node:child_process';
    import { buildFirmwareSet } from ${JSON.stringify(new URL('./build-firmware-set.mjs', import.meta.url).href)};
    let n = 0;
    try { console.log(buildFirmwareSet(${JSON.stringify(root)}, ${JSON.stringify(out)}, ${JSON.stringify(manifest)}, (_c,_a,o) => ++n === 2 ? {status: 1} : spawnSync(process.execPath, ['-e','console.log("child cargo output")'], o))); }
    catch (error) { console.error(error.message); process.exitCode = 1; }`;
    assert.ok(plan);
    const child = spawnSync(process.execPath, ['-e', code], { encoding: 'utf8' });
    assert.equal(child.status, 1);
    assert.equal(child.stdout, '');
    assert.match(child.stderr, /child cargo output/);
    assert.ok(!existsSync(manifest));
  }));

test('Unix desktop recipe isolates firmware inputs, checks helper output and cleans up on failure', () => {
  if (process.platform === 'win32') return; // The Windows leg executes the PowerShell contract.
  fixture(({ root }) => {
    const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
    const recipe = readFileSync(join(repo, 'justfile'), 'utf8')
      .split('\napp:\n')[1]
      .split('\n# DEV ONLY')[0]
      .replace(/^ {4}/gm, '')
      .trim();
    const bin = join(root, 'bin');
    mkdirSync(bin);
    const stub = `#!${process.execPath}
const fs = require('node:fs');
const path = require('node:path');
const name = path.basename(process.argv[1]);
const args = process.argv.slice(2);
fs.appendFileSync(process.env.KIVORI_TEST_TRACE, JSON.stringify({name, legacy:process.env.KIVORI_FIRMWARE_PATH ?? null, manifest:process.env.KIVORI_FIRMWARE_MANIFEST ?? null})+'\\n');
if(name==='bun') {
 const manifest = args[args.indexOf('--manifest-out')+1];
 fs.writeFileSync(manifest,'[]');
 fs.writeFileSync(process.env.KIVORI_TEST_MANIFEST,manifest);
 console.log(manifest);
 if(process.env.KIVORI_TEST_MODE==='multiple-lines') console.log(manifest);
 if(process.env.KIVORI_TEST_MODE==='helper-failure') process.exit(2);
} else {
 if(!fs.existsSync(process.env.KIVORI_FIRMWARE_MANIFEST)) process.exit(9);
 if(process.env.KIVORI_TEST_MODE==='cargo-failure') process.exit(3);
}
`;
    // Executable stubs replace commands; this never starts the desktop app.
    for (const name of ['bun', 'cargo']) writeFileSync(join(bin, name), stub, { mode: 0o755 });
    for (const mode of ['success', 'helper-failure', 'multiple-lines', 'cargo-failure']) {
      const trace = join(root, `${mode}.jsonl`);
      const manifestRecord = join(root, `${mode}.txt`);
      const result = spawnSync('bash', ['-c', recipe], {
        cwd: repo,
        encoding: 'utf8',
        env: {
          ...process.env,
          PATH: `${bin}:${process.env.PATH}`,
          KIVORI_TEST_MODE: mode,
          KIVORI_TEST_TRACE: trace,
          KIVORI_TEST_MANIFEST: manifestRecord,
          KIVORI_FIRMWARE_PATH: 'previous image',
          KIVORI_FIRMWARE_MANIFEST: 'previous manifest',
        },
      });
      assert.equal(result.status === 0, mode === 'success', result.stderr);
      const calls = readFileSync(trace, 'utf8').trim().split('\n').map(JSON.parse);
      assert.deepEqual(
        calls.map((c) => c.name),
        mode === 'success' || mode === 'cargo-failure' ? ['bun', 'cargo'] : ['bun'],
      );
      assert.ok(calls.every((c) => c.legacy === null));
      const manifest = readFileSync(manifestRecord, 'utf8');
      if (calls[1]) assert.equal(calls[1].manifest, manifest);
      assert.ok(!existsSync(dirname(manifest)), 'owned staging directory is cleaned up');
    }
  });
});

test('catalog discovery matches native rules for stray files, hidden directories and links', () =>
  fixture(({ root, put, plans }) => {
    put('products/.keep', 'ignored');
    assert.equal(plans().length, 3);
    put('products/stray.txt', 'not a product');
    assert.throws(plans, /Unexpected catalog/);
    rmSync(join(root, 'products/stray.txt'));
    mkdirSync(join(root, 'products/.hidden'));
    assert.throws(plans, /Unregistered/);
    rmSync(join(root, 'products/.hidden'), { recursive: true });
    if (process.platform !== 'win32') {
      symlinkSync(join(root, 'products/zeta'), join(root, 'products/linked'));
      assert.throws(plans, /Unexpected catalog/);
      rmSync(join(root, 'products/linked'));
      const registry = join(root, 'products/registry.toml');
      copyFileSync(registry, join(root, 'registry-backup.toml'));
      rmSync(registry);
      symlinkSync(join(root, 'registry-backup.toml'), registry);
      assert.throws(plans, /regular file/);
    }
  }));

test('invalid build environment leaves no staging directory or manifest', () =>
  fixture(({ root, out, manifest }) => {
    for (const key of ['KIVORI_FIRMWARE_PATH', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS']) {
      const previous = process.env[key];
      process.env[key] = 'inherited';
      try {
        assert.throws(
          () => buildFirmwareSet(root, out, manifest, () => assert.fail('must not invoke cargo')),
          /Unset/,
        );
        assert.ok(!existsSync(out));
        assert.ok(!existsSync(manifest));
      } finally {
        if (previous === undefined) delete process.env[key];
        else process.env[key] = previous;
      }
    }
  }));
