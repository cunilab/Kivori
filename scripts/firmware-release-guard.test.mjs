import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { firmwareInputs, normalizeBookkeeping } from './firmware-release-guard.mjs';
import { planRelease } from './release-bump.mjs';

const bumpScript = fileURLToPath(new URL('./release-bump.mjs', import.meta.url));
function main(args, root) {
  return execFileSync(
    'bun',
    [
      '--eval',
      `import { main } from ${JSON.stringify(pathToFileURL(bumpScript).href)}; main(JSON.parse(process.argv[1]), process.argv[2]);`,
      JSON.stringify(args),
      root,
    ],
    { encoding: 'utf8', stdio: 'pipe' },
  );
}

const git = (root, ...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
function put(root, path, text) {
  mkdirSync(dirname(join(root, path)), { recursive: true });
  writeFileSync(join(root, path), text);
}
function commit(root, subject) {
  git(root, 'add', '.');
  git(root, 'commit', '--quiet', '-m', subject);
}
function locks(root) {
  for (const manifest of ['Cargo.toml', 'firmware/pala/Cargo.toml']) {
    execFileSync('cargo', ['generate-lockfile', '--offline', '--manifest-path', manifest], {
      cwd: root,
      stdio: 'pipe',
    });
  }
}

function productToml(name, crate) {
  return `short_name = "${name}"
display_name = "Kivori ${name}"
[boards.board.firmware]
crate = "${crate}"
artifact = "target/riscv32imc-unknown-none-elf/release/device"
[boards.board.firmware.build]
target = "riscv32imc-unknown-none-elf"
features = []
`;
}

function fixture(t, { tagged = true, consumesVersion = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'kivori-release-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  t.mock.method(console, 'log', () => {});
  git(root, 'init', '--quiet', '--initial-branch=main');
  git(root, 'config', 'user.name', 'Release guard test');
  git(root, 'config', 'user.email', 'release-test@example.invalid');
  git(root, 'config', 'commit.gpgsign', 'false');
  git(root, 'config', 'tag.gpgsign', 'false');
  git(root, 'config', 'core.hooksPath', join(root, '.git', 'no-hooks'));
  git(root, 'config', 'core.autocrlf', 'false');
  put(
    root,
    'Cargo.toml',
    '[workspace]\nmembers = ["crates/shared", "crates/transitive", "tools/asset-compiler"]\nexclude = ["firmware"]\nresolver = "2"\n[workspace.package]\nversion = "0.1.0"\n',
  );
  for (const [dir, name] of [
    ['crates/shared', 'test-shared'],
    ['crates/transitive', 'test-transitive'],
    ['tools/asset-compiler', 'test-compiler'],
  ]) {
    put(
      root,
      `${dir}/Cargo.toml`,
      `[package]\nname = "${name}"\nversion.workspace = true\nedition = "2021"\n` +
        (name === 'test-shared'
          ? '[dependencies]\ntest-transitive = { path = "../transitive" }\n'
          : ''),
    );
    put(
      root,
      `${dir}/src/lib.rs`,
      consumesVersion && name === 'test-shared'
        ? `pub const VERSION: &str = env!("${consumesVersion === true ? 'CARGO_PKG_VERSION' : consumesVersion}");\n`
        : 'pub const VALUE: u8 = 1;\n',
    );
  }
  put(
    root,
    'firmware/pala/Cargo.toml',
    '[workspace]\nresolver = "2"\n[package]\nname = "test-firmware"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\ntest-shared = { path = "../../crates/shared" }\n[build-dependencies]\ntest-compiler = { path = "../../tools/asset-compiler" }\n',
  );
  put(root, 'firmware/pala/src/lib.rs', 'pub const VALUE: u8 = 1;\n');
  put(root, 'products/pala/product.toml', productToml('Pip', 'firmware/pala'));
  put(root, 'products/registry.toml', '[products]\npala = 1\n[boards]\nboard = 1\n');
  put(root, 'assets/mascot.svg', '<svg/>\n');
  locks(root);
  commit(root, 'initial firmware');
  if (tagged) git(root, 'tag', '-a', 'fw-pala-v0.1.0', '-m', 'firmware 0.1.0');
  return root;
}
function rejectWithoutWrites(root, pattern, extra = []) {
  const paths = [
    'Cargo.toml',
    'firmware/pala/Cargo.toml',
    'Cargo.lock',
    'firmware/pala/Cargo.lock',
  ];
  const before = paths.map((p) => readFileSync(join(root, p), 'utf8'));
  const outputs = join(root, 'release-outputs');
  assert.throws(
    () => main(['--app', 'patch', '--write', '--outputs', outputs, ...extra], root),
    pattern,
  );
  assert.deepEqual(
    paths.map((p) => readFileSync(join(root, p), 'utf8')),
    before,
  );
  assert.equal(existsSync(outputs), false);
}

test('Cargo input inventory includes path/build dependencies and non-Cargo build inputs', (t) => {
  const root = fixture(t);
  const { paths } = firmwareInputs(root, 'pala');
  for (const path of [
    'firmware/pala',
    'crates/shared',
    'crates/transitive',
    'tools/asset-compiler',
    'products',
    'assets',
    '.cargo',
    'rust-toolchain.toml',
    'Cargo.lock',
  ]) {
    assert.ok(paths.includes(path), path);
  }
});

test('the real firmware graph and its declared build inputs are covered', () => {
  const root = fileURLToPath(new URL('../', import.meta.url));
  const { paths, bookkeepingNames } = firmwareInputs(root, 'pala');
  for (const path of [
    'crates/kivori-model',
    'crates/kivori-protocol',
    'tools/asset-compiler',
    'products',
    'assets',
  ]) {
    assert.ok(paths.includes(path), path);
  }
  assert.ok(
    bookkeepingNames.has('kivori-model'),
    'current firmware permits harmless workspace bookkeeping',
  );
  const script = readFileSync(join(root, 'firmware/pala/build.rs'), 'utf8');
  const declarations = [...script.matchAll(/cargo:rerun-if-changed=([^"\n]*)/g)];
  assert.ok(declarations.length > 0);
  for (const [, path] of declarations) {
    assert.doesNotMatch(path, /[{}]/, 'dynamic build inputs need an explicit guard inventory');
    const input = relative(root, resolve(root, 'firmware/pala', path))
      .split(sep)
      .join('/');
    assert.ok(
      paths.some((p) => input === p || input.startsWith(`${p}/`)),
      input,
    );
  }
});

test('first release and docs-only changes allow unchanged firmware', (t) => {
  const root = fixture(t, { tagged: false });
  main(['--app', 'none', '--dry-run'], root);
  git(root, 'tag', 'fw-pala-v0.1.0');
  put(root, 'docs/notes.md', 'Docs only.\n');
  commit(root, 'docs: add release notes');
  main(['--app', 'patch', '--dry-run'], root);
  assert.equal(git(root, 'status', '--porcelain'), '');
});

test('changed inputs reject unchanged firmware before versions or outputs are written', async (t) => {
  for (const path of [
    'firmware/pala/src/lib.rs',
    'crates/shared/src/lib.rs',
    'crates/transitive/src/lib.rs',
    'tools/asset-compiler/src/lib.rs',
    'assets/mascot.svg',
    'products/registry.toml',
    'Cargo.lock',
    'firmware/pala/Cargo.lock',
    '.cargo/config.toml',
    'firmware/.cargo/config.toml',
    'rust-toolchain.toml',
    'firmware/pala/rust-toolchain.toml',
    '.github/workflows/release.yml',
    'scripts/build-desktop-firmware.ps1',
    'scripts/build-firmware.mjs',
    'scripts/build-firmware-set.mjs',
    'scripts/release-catalog.mjs',
  ]) {
    await t.test(path, (t) => {
      const root = fixture(t);
      const before = existsSync(join(root, path)) ? readFileSync(join(root, path), 'utf8') : '';
      const addition = path.endsWith('rust-toolchain.toml')
        ? '[toolchain]\nchannel = "stable"\n'
        : path === 'firmware/.cargo/config.toml'
          ? '[env]\nKIVORI_GUARD_TEST_VALUE = "1"\n'
          : path.endsWith('.rs')
            ? '// firmware fix\n'
            : '# changed input\n';
      put(root, path, before + addition);
      commit(root, 'chore(release): real input change');
      rejectWithoutWrites(
        root,
        /firmware inputs changed.*resolved firmware version 0\.1\.0 is unchanged/s,
      );
    });
  }
});

test('resolved version honors exact overrides; unchanged overrides cannot bypass the guard', (t) => {
  const root = fixture(t);
  put(root, 'firmware/pala/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'fix: firmware behavior');
  rejectWithoutWrites(root, /firmware inputs changed/, ['--firmware-version', '0.1.0']);
  main(['--app', 'patch', '--firmware', 'patch', '--dry-run'], root);
  main(['--app', 'patch', '--firmware', 'none', '--firmware-version', '0.1.1', '--write'], root);
  assert.match(readFileSync(join(root, 'firmware/pala/Cargo.toml'), 'utf8'), /version = "0\.1\.1"/);
});

test('workspace version bookkeeping is ignored by content, while mixed changes still fail', (t) => {
  const root = fixture(t);
  put(root, 'Cargo.toml', readFileSync(join(root, 'Cargo.toml'), 'utf8').replace('0.1.0', '0.2.0'));
  locks(root);
  commit(root, 'fix: harmless workspace version bookkeeping');
  main(['--app', 'patch', '--dry-run'], root);
  put(root, 'crates/shared/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'chore(release): v0.2.0');
  rejectWithoutWrites(root, /crates\/shared\/src\/lib\.rs/);
});

test('workspace version metadata is not exempt when shared firmware code consumes it', (t) => {
  const root = fixture(t, { consumesVersion: 'CARGO_PKG_VERSION_MAJOR' });
  put(root, 'Cargo.toml', readFileSync(join(root, 'Cargo.toml'), 'utf8').replace('0.1.0', '0.2.0'));
  locks(root);
  commit(root, 'chore: update workspace version');
  rejectWithoutWrites(root, /firmware inputs changed/);
});

test('firmware reads of workspace manifests disable bookkeeping exemptions', (t) => {
  const root = fixture(t, { tagged: false });
  put(
    root,
    'firmware/pala/src/lib.rs',
    'pub const APP_MANIFEST: &str = include_str!("../../../Cargo.toml");\n',
  );
  commit(root, 'firmware embeds workspace metadata');
  git(root, 'tag', 'fw-pala-v0.1.0');
  put(root, 'Cargo.toml', readFileSync(join(root, 'Cargo.toml'), 'utf8').replace('0.1.0', '0.2.0'));
  locks(root);
  commit(root, 'chore: workspace version');
  rejectWithoutWrites(root, /firmware inputs changed/);
});

test('shared crate reads of workspace manifests also disable bookkeeping exemptions', (t) => {
  const root = fixture(t, { tagged: false });
  put(
    root,
    'crates/shared/src/lib.rs',
    'pub const APP_MANIFEST: &str = include_str!("../../../Cargo.toml");\n',
  );
  commit(root, 'shared crate embeds workspace metadata');
  git(root, 'tag', 'fw-pala-v0.1.0');
  put(root, 'Cargo.toml', readFileSync(join(root, 'Cargo.toml'), 'utf8').replace('0.1.0', '0.2.0'));
  locks(root);
  commit(root, 'chore: workspace version');
  rejectWithoutWrites(root, /firmware inputs changed/);
});

test('registry sources/checksums/versions are never normalized as path-package bookkeeping', () => {
  const names = new Set(['test-shared']);
  const block =
    '[[package]]\nname = "test-shared"\nversion = "0.1.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "abc"\n';
  for (const changed of [block, block.replace('0.1.0', '0.2.0'), block.replace('abc', 'def')]) {
    assert.equal(normalizeBookkeeping('Cargo.lock', changed, '0.1.0', names), changed);
  }
});

test('a stale committed lockfile is rejected before writes, rather than repaired after the guard', (t) => {
  const root = fixture(t, { tagged: false });
  put(
    root,
    'custom/new/Cargo.toml',
    '[workspace]\n[package]\nname = "new-dependency"\nversion = "0.1.0"\nedition = "2021"\n',
  );
  put(root, 'custom/new/src/lib.rs', 'pub const VALUE: u8 = 1;\n');
  put(
    root,
    'firmware/pala/Cargo.toml',
    readFileSync(join(root, 'firmware/pala/Cargo.toml'), 'utf8').replace(
      '[dependencies]\n',
      '[dependencies]\nnew-dependency = { path = "../../custom/new" }\n',
    ),
  );
  commit(root, 'stale lockfile baseline');
  git(root, 'tag', 'fw-pala-v0.1.0');
  rejectWithoutWrites(
    root,
    /(?:the lock file .* needs to be updated|cannot update the lock file)/s,
  );
});

test('resolved Cargo patches are included even without a dependency.path declaration', (t) => {
  const root = fixture(t, { tagged: false });
  put(
    root,
    'custom/patched/Cargo.toml',
    '[workspace]\n[package]\nname = "patched-dependency"\nversion = "0.1.0"\nedition = "2021"\n',
  );
  put(root, 'custom/patched/src/lib.rs', 'pub const VALUE: u8 = 1;\n');
  const manifest =
    readFileSync(join(root, 'firmware/pala/Cargo.toml'), 'utf8').replace(
      '[dependencies]\n',
      '[dependencies]\npatched-dependency = "0.1.0"\n',
    ) + '\n[patch.crates-io]\npatched-dependency = { path = "../../custom/patched" }\n';
  put(root, 'firmware/pala/Cargo.toml', manifest);
  locks(root);
  commit(root, 'patch dependency baseline');
  git(root, 'tag', 'fw-pala-v0.1.0');
  assert.ok(firmwareInputs(root, 'pala').paths.includes('custom/patched'));
  put(root, 'custom/patched/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'fix: patched dependency behavior');
  rejectWithoutWrites(root, /custom\/patched\/src\/lib\.rs/);
});

test('legacy Cargo config paths overrides are refused before writes', (t) => {
  const root = fixture(t);
  put(root, 'firmware/pala/.cargo/config.toml', 'paths = ["../../crates/shared"]\n');
  commit(root, 'use legacy path override');
  rejectWithoutWrites(root, /paths overrides are unsupported/);
});

test('intermediate Cargo config paths overrides are refused too', (t) => {
  const root = fixture(t);
  put(root, 'firmware/.cargo/config.toml', 'paths = ["../crates/shared"]\n');
  commit(root, 'use parent-directory path override');
  rejectWithoutWrites(root, /paths overrides are unsupported/);
});

test('tags off HEAD ancestry never become first-release or valid-bump exceptions', (t) => {
  const root = fixture(t, { tagged: false });
  git(root, 'checkout', '--quiet', '-b', 'other-release');
  put(root, 'firmware/pala/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'unmerged firmware release');
  git(root, 'tag', 'fw-pala-v0.1.0');
  git(root, 'checkout', '--quiet', 'main');
  rejectWithoutWrites(root, /not on HEAD's ancestry/);
  rejectWithoutWrites(root, /not on HEAD's ancestry/, ['--firmware-version', '0.2.0']);
});

test('shallow history fails before any mutation, even when the shallow clone has no firmware tags', (t) => {
  const source = fixture(t);
  put(source, 'docs/notes.md', 'One more commit.\n');
  commit(source, 'docs: notes');
  const root = mkdtempSync(join(tmpdir(), 'kivori-shallow-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync('git', ['clone', '--quiet', '--depth=1', pathToFileURL(source).href, root]);
  rejectWithoutWrites(root, /needs full Git history/);
});

test('dirty firmware inputs are not silently omitted from HEAD comparisons', (t) => {
  const root = fixture(t);
  put(root, 'firmware/pala/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  rejectWithoutWrites(root, /commit firmware inputs before checking/);
});

test('an older existing firmware tag cannot bypass version monotonicity', () => {
  assert.throws(
    () =>
      planRelease({
        appCurrent: '0.1.0',
        firmwareCurrent: '0.1.0',
        product: 'pala',
        appBump: 'patch',
        appExact: '',
        firmwareBump: 'none',
        firmwareExact: '',
        tags: ['v0.1.0', 'fw-pala-v0.1.0', 'fw-pala-v0.2.0'],
      }),
    /below the latest released version 0\.2\.0/,
  );
});

function twoProducts(t, options) {
  const root = fixture(t, options);
  put(root, 'products/registry.toml', '[products]\npala = 1\nlamp = 2\n[boards]\nboard = 1\n');
  put(root, 'products/lamp/product.toml', productToml('Lamp', 'devices/lamp firmware'));
  put(
    root,
    'devices/lamp firmware/Cargo.toml',
    '[workspace]\nresolver = "2"\n[package]\nname = "test-lamp"\nversion = "0.4.0"\nedition = "2021"\n[dependencies]\ntest-shared = { path = "../../crates/shared" }\n',
  );
  put(root, 'devices/lamp firmware/src/lib.rs', 'pub const VALUE: u8 = 1;\n');
  execFileSync(
    'cargo',
    ['generate-lockfile', '--offline', '--manifest-path', 'devices/lamp firmware/Cargo.toml'],
    { cwd: root, stdio: 'pipe' },
  );
  commit(root, 'feat: second catalog product');
  if (options?.tagged !== false) {
    git(root, 'tag', '-f', 'fw-pala-v0.1.0');
    git(root, 'tag', 'fw-lamp-v0.4.0');
  }
  return root;
}
function outputFile(t) {
  const dir = mkdtempSync(join(tmpdir(), 'kivori-plan-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return join(dir, 'outputs');
}
function readOutputs(path) {
  return Object.fromEntries(
    readFileSync(path, 'utf8')
      .trim()
      .split('\n')
      .filter(Boolean)
      .map((line) => {
        const at = line.indexOf('=');
        return [line.slice(0, at), line.slice(at + 1)];
      }),
  );
}
function info(args, root) {
  const script = new URL('./release-info.mjs', import.meta.url).href;
  return execFileSync(
    'bun',
    [
      '--eval',
      `import { main } from ${JSON.stringify(script)}; main(JSON.parse(process.argv[1]), process.argv[2]);`,
      JSON.stringify(args),
      root,
    ],
    { encoding: 'utf8', stdio: 'pipe' },
  );
}

test('whole catalog first release and independent per-product overrides preserve wire order', (t) => {
  const root = twoProducts(t, { tagged: false });
  const outputs = outputFile(t);
  main(['--app', 'none', '--dry-run', '--outputs', outputs], root);
  const first = readOutputs(outputs);
  assert.equal(first.commit_needed, 'false');
  assert.deepEqual(JSON.parse(first.firmware), [
    { product: 'pala', name: 'Pip', version: '0.1.0', tag: 'fw-pala-v0.1.0' },
    { product: 'lamp', name: 'Lamp', version: '0.4.0', tag: 'fw-lamp-v0.4.0' },
  ]);
  assert.deepEqual(JSON.parse(first.firmware_crates), ['firmware/pala', 'devices/lamp firmware']);
  const notes = info(
    ['--version', first.app_version, '--firmware-versions', first.firmware_versions],
    root,
  );
  assert.ok(notes.startsWith('Kivori 0.1.0 · Pip firmware 0.1.0 · Lamp firmware 0.4.0\n'));
  assert.match(notes, /Kivori Pip firmware 0.1.0, Kivori Lamp firmware 0.4.0/);
  assert.equal(git(root, 'status', '--porcelain'), '');
});

test('a later product refusing unchanged firmware leaves every version and output untouched', (t) => {
  const root = twoProducts(t);
  put(root, 'devices/lamp firmware/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'fix: lamp behavior');
  const outputs = outputFile(t);
  assert.throws(
    () =>
      main(
        [
          '--app',
          'patch',
          '--product',
          'pala',
          '--firmware',
          'patch',
          '--write',
          '--outputs',
          outputs,
        ],
        root,
      ),
    /lamp firmware inputs changed/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
  assert.equal(existsSync(outputs), false);
  main(
    [
      '--app',
      'patch',
      '--firmware-overrides',
      '{"pala":"patch","lamp":"0.5.0"}',
      '--dry-run',
      '--outputs',
      outputs,
    ],
    root,
  );
  const preview = info(
    ['--version', '0.1.1', '--firmware-versions', readOutputs(outputs).firmware_versions],
    root,
  );
  rmSync(outputs);
  main(
    [
      '--app',
      'patch',
      '--firmware-overrides',
      '{"pala":"patch","lamp":"0.5.0"}',
      '--write',
      '--outputs',
      outputs,
    ],
    root,
  );
  assert.equal(info(['--version', '0.1.1'], root), preview);
  assert.match(readFileSync(join(root, 'firmware/pala/Cargo.toml'), 'utf8'), /version = "0.1.1"/);
  assert.match(
    readFileSync(join(root, 'devices/lamp firmware/Cargo.toml'), 'utf8'),
    /version = "0.5.0"/,
  );
  assert.deepEqual(JSON.parse(readOutputs(outputs).firmware_versions), {
    pala: '0.1.1',
    lamp: '0.5.0',
  });
});

test('shared inputs guard all products, and unlisted overrides mean unchanged', (t) => {
  const root = twoProducts(t);
  put(root, 'crates/shared/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'fix: shared behavior');
  const outputs = outputFile(t);
  assert.throws(
    () =>
      main(
        [
          '--app',
          'patch',
          '--firmware-overrides',
          '{"pala":"patch"}',
          '--write',
          '--outputs',
          outputs,
        ],
        root,
      ),
    /lamp firmware inputs changed/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
  assert.equal(existsSync(outputs), false);
  main(
    ['--app', 'patch', '--firmware-overrides', '{"pala":"patch","lamp":"patch"}', '--dry-run'],
    root,
  );
});

test('catalog selectors and override validation fail before any mutation', (t) => {
  const root = twoProducts(t);
  for (const args of [
    ['--product', 'reserved'],
    ['--firmware', 'patch'],
    ['--firmware-version', '1.0.0'],
    ['--firmware-overrides', '{"reserved":"patch"}'],
    ['--firmware-overrides', '{"lamp":"huge"}'],
    ['--firmware-overrides', '{"lamp":3}'],
    ['--firmware-overrides', '[]'],
    ['--firmware-overrides', '{}', '--product', 'pala'],
  ]) {
    const outputs = outputFile(t);
    assert.throws(() => main(['--app', 'patch', '--write', '--outputs', outputs, ...args], root));
    assert.equal(git(root, 'status', '--porcelain'), '');
    assert.equal(existsSync(outputs), false);
  }
});

test('multi-board products check every crate and reject mismatched versions', (t) => {
  const root = twoProducts(t);
  const original = readFileSync(join(root, 'products/lamp/product.toml'), 'utf8');
  put(
    root,
    'products/lamp/product.toml',
    original +
      productToml('Lamp', 'firmware/pala')
        .slice(productToml('Lamp', 'firmware/pala').indexOf('[boards.'))
        .replaceAll('boards.board', 'boards.other'),
  );
  put(
    root,
    'products/registry.toml',
    '[products]\npala = 1\nlamp = 2\n[boards]\nboard = 1\nother = 2\n',
  );
  commit(root, 'test: second lamp board');
  const outputs = outputFile(t);
  assert.throws(
    () => main(['--app', 'patch', '--write', '--outputs', outputs], root),
    /same firmware version/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
  assert.equal(existsSync(outputs), false);
});

test('shared firmware crate allows consistent lineages and refuses conflicting bumps', (t) => {
  const root = twoProducts(t);
  put(root, 'products/lamp/product.toml', productToml('Lamp', 'firmware/pala'));
  commit(root, 'test: shared firmware crate');
  git(root, 'tag', '-d', 'fw-lamp-v0.4.0');
  git(root, 'tag', 'fw-lamp-v0.1.0');
  git(root, 'tag', '-f', 'fw-pala-v0.1.0');
  assert.throws(
    () => main(['--app', 'patch', '--product', 'pala', '--firmware', 'patch', '--write'], root),
    /Conflicting firmware versions/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
  const outputs = outputFile(t);
  main(
    [
      '--app',
      'patch',
      '--firmware-overrides',
      '{"pala":"patch","lamp":"patch"}',
      '--dry-run',
      '--outputs',
      outputs,
    ],
    root,
  );
  assert.deepEqual(JSON.parse(readOutputs(outputs).firmware_crates), ['firmware/pala']);
});

test('planned app version changes cannot bypass shared version reflection', (t) => {
  const root = fixture(t, { consumesVersion: true });
  rejectWithoutWrites(root, /firmware inputs will change/);
  main(['--app', 'patch', '--firmware', 'patch', '--dry-run'], root);
});

test('release notes cover independent firmware histories at custom catalog paths', (t) => {
  const root = twoProducts(t);
  git(root, 'tag', 'v0.1.0');
  put(root, 'firmware/pala/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'fix(firmware): pip correction');
  put(root, 'devices/lamp firmware/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'feat(firmware): lamp behavior');
  const notes = info(
    [
      '--version',
      '0.2.0',
      '--prev-tag',
      'v0.1.0',
      '--firmware-versions',
      '{"pala":"0.1.1","lamp":"0.5.0"}',
      '--repo',
      'test/repo',
    ],
    root,
  );
  const pip = notes.slice(notes.indexOf('### Pip firmware'), notes.indexOf('### Lamp firmware'));
  const lamp = notes.slice(notes.indexOf('### Lamp firmware'));
  assert.match(pip, /Pip firmware 0.1.1.*was 0.1.0/);
  assert.match(pip, /pip correction/);
  assert.doesNotMatch(pip, /lamp behavior/);
  assert.match(lamp, /Lamp firmware 0.5.0.*was 0.4.0/);
  assert.match(lamp, /lamp behavior/);
  assert.doesNotMatch(lamp, /pip correction/);
  assert.match(lamp, /compare\/v0.1.0...v0.2.0/);
  for (const map of [
    '{"lamp":"0.5.0"}',
    '{"pala":"0.1.1","absent":"0.4.0"}',
    '[]',
    '{"pala":3,"lamp":"0.4.0"}',
  ])
    assert.throws(
      () => info(['--version', '0.2.0', '--firmware-versions', map], root),
      /Invalid catalog firmware version overrides/,
    );
});

function workflowStep(name) {
  const workflow = readFileSync(
    new URL('../.github/workflows/release-start.yml', import.meta.url),
    'utf8',
  );
  const section = workflow.split(`      - name: ${name}\n`)[1].split(/\n {6}- |\n {2}release:/)[0];
  return section
    .split('        run: |\n')[1]
    .split('\n')
    .filter((line) => line.startsWith('          ') || !line.trim())
    .map((line) => line.slice(10))
    .join('\n');
}
function runWorkflowStep(name, root, outputs, extra = {}) {
  const env = {
    ...process.env,
    ...Object.fromEntries(
      Object.entries(readOutputs(outputs)).map(([k, v]) => [k.toUpperCase(), v]),
    ),
    RUNNER_TEMP: dirname(outputs),
    ...extra,
  };
  return execFileSync('bash', ['-c', workflowStep(name)], {
    cwd: root,
    env,
    encoding: 'utf8',
    stdio: 'pipe',
  });
}

test('actual release workflow refreshes all locks, commits exact files and atomically pushes every tag', (t) => {
  const root = twoProducts(t);
  const outputs = outputFile(t);
  const remote = join(dirname(outputs), 'origin.git');
  execFileSync('git', ['init', '--bare', '--quiet', remote]);
  git(root, 'remote', 'add', 'origin', remote);
  git(root, 'push', '--quiet', 'origin', 'main');
  main(
    [
      '--app',
      'patch',
      '--firmware-overrides',
      '{"pala":"patch","lamp":"0.5.0"}',
      '--write',
      '--outputs',
      outputs,
    ],
    root,
  );
  runWorkflowStep('Refresh all Cargo.lock files', root, outputs);
  for (const crate of ['firmware/pala', 'devices/lamp firmware'])
    assert.match(
      readFileSync(join(root, crate, 'Cargo.lock'), 'utf8'),
      /name = "test-shared"\nversion = "0.1.1"/,
    );
  runWorkflowStep('Commit, tag and push', root, outputs);
  assert.equal(git(root, 'status', '--porcelain'), '');
  assert.equal(git(remote, 'rev-parse', 'main'), git(root, 'rev-parse', 'HEAD'));
  for (const tag of ['v0.1.1', 'fw-pala-v0.1.1', 'fw-lamp-v0.5.0'])
    assert.equal(git(remote, 'rev-parse', `${tag}^{commit}`), git(root, 'rev-parse', 'HEAD'));
  const changed = git(root, 'diff-tree', '--no-commit-id', '--name-only', '-r', 'HEAD')
    .split('\n')
    .sort();
  assert.deepEqual(changed, JSON.parse(readOutputs(outputs).release_files).sort());
});

test('actual first-release workflow tags without a version commit', (t) => {
  const root = twoProducts(t, { tagged: false });
  const before = git(root, 'rev-parse', 'HEAD');
  const outputs = outputFile(t);
  const remote = join(dirname(outputs), 'origin.git');
  execFileSync('git', ['init', '--bare', '--quiet', remote]);
  git(root, 'remote', 'add', 'origin', remote);
  git(root, 'push', '--quiet', 'origin', 'main');
  main(['--app', 'none', '--write', '--outputs', outputs], root);
  runWorkflowStep('Commit, tag and push', root, outputs);
  assert.equal(git(root, 'rev-parse', 'HEAD'), before);
  for (const tag of ['v0.1.0', 'fw-pala-v0.1.0', 'fw-lamp-v0.4.0'])
    assert.equal(git(remote, 'rev-parse', `${tag}^{commit}`), before);
});

test('unexpected lock-refresh edits are refused and remote tag races push nothing', (t) => {
  const root = twoProducts(t);
  const outputs = outputFile(t);
  const remote = join(dirname(outputs), 'origin.git');
  execFileSync('git', ['init', '--bare', '--quiet', remote]);
  git(root, 'remote', 'add', 'origin', remote);
  git(root, 'push', '--quiet', 'origin', 'main');
  const before = git(root, 'rev-parse', 'HEAD');
  main(
    [
      '--app',
      'patch',
      '--firmware-overrides',
      '{"pala":"patch","lamp":"patch"}',
      '--write',
      '--outputs',
      outputs,
    ],
    root,
  );
  put(root, 'surprise.txt', 'Unexpected output.\n');
  assert.throws(
    () => runWorkflowStep('Refresh all Cargo.lock files', root, outputs),
    /Unexpected change in surprise.txt/,
  );
  rmSync(join(root, 'surprise.txt'));
  runWorkflowStep('Refresh all Cargo.lock files', root, outputs);
  git(remote, 'update-ref', 'refs/tags/fw-lamp-v0.4.1', before);
  assert.throws(
    () => runWorkflowStep('Commit, tag and push', root, outputs),
    /Push to main rejected/,
  );
  assert.equal(git(remote, 'rev-parse', 'main'), before);
  assert.equal(git(remote, 'tag', '--list', 'v0.1.1', 'fw-pala-v0.1.1'), '');
});

test('every crate of a same-version multi-board product participates in the guard', (t) => {
  const root = twoProducts(t);
  const other = 'boards/alternate';
  put(
    root,
    `${other}/Cargo.toml`,
    '[workspace]\n[package]\nname = "alternate"\nversion = "0.4.0"\nedition = "2021"\n',
  );
  put(root, `${other}/src/lib.rs`, 'pub const VALUE: u8 = 1;\n');
  execFileSync(
    'cargo',
    ['generate-lockfile', '--offline', '--manifest-path', `${other}/Cargo.toml`],
    { cwd: root, stdio: 'pipe' },
  );
  const path = 'products/lamp/product.toml';
  const board = productToml('Lamp', other);
  put(
    root,
    path,
    readFileSync(join(root, path), 'utf8') +
      board.slice(board.indexOf('[boards.')).replaceAll('boards.board', 'boards.other'),
  );
  put(
    root,
    'products/registry.toml',
    '[products]\npala = 1\nlamp = 2\n[boards]\nboard = 1\nother = 2\n',
  );
  commit(root, 'feat: alternate lamp board');
  git(root, 'tag', '-f', 'fw-pala-v0.1.0');
  git(root, 'tag', '-f', 'fw-lamp-v0.4.0');
  put(root, `${other}/src/lib.rs`, 'pub const VALUE: u8 = 2;\n');
  commit(root, 'fix: alternate board');
  const outputs = outputFile(t);
  assert.throws(
    () => main(['--app', 'patch', '--write', '--outputs', outputs], root),
    /boards\/alternate\/src\/lib.rs/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
  assert.equal(existsSync(outputs), false);
  main(
    ['--app', 'patch', '--product', 'lamp', '--firmware', 'patch', '--write', '--outputs', outputs],
    root,
  );
  for (const crate of ['devices/lamp firmware', other])
    assert.match(readFileSync(join(root, crate, 'Cargo.toml'), 'utf8'), /version = "0.4.1"/);
  assert.deepEqual(JSON.parse(readOutputs(outputs).firmware_crates), [
    'firmware/pala',
    'devices/lamp firmware',
    other,
  ]);
});

test('future firmware manifest edits cannot bypass another product dependency guard', (t) => {
  const root = twoProducts(t);
  put(
    root,
    'devices/lamp firmware/Cargo.toml',
    readFileSync(join(root, 'devices/lamp firmware/Cargo.toml'), 'utf8') +
      'test-firmware = { path = "../../firmware/pala" }\n',
  );
  execFileSync(
    'cargo',
    ['generate-lockfile', '--offline', '--manifest-path', 'devices/lamp firmware/Cargo.toml'],
    { cwd: root, stdio: 'pipe' },
  );
  commit(root, 'test: firmware crate dependency');
  git(root, 'tag', '-f', 'fw-pala-v0.1.0');
  git(root, 'tag', '-f', 'fw-lamp-v0.4.0');
  assert.throws(
    () => main(['--app', 'patch', '--product', 'pala', '--firmware', 'patch', '--write'], root),
    /lamp firmware inputs will change/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
});

test('newly cataloged shared-crate products have no invented previous firmware baseline', (t) => {
  const root = twoProducts(t);
  git(root, 'tag', 'v0.1.0', 'HEAD^');
  put(root, 'products/lamp/product.toml', productToml('Lamp', 'firmware/pala'));
  commit(root, 'feat: new product sharing firmware');
  const notes = info(['--version', '0.1.1', '--prev-tag', 'v0.1.0'], root);
  const section = notes.slice(notes.indexOf('### Lamp firmware'));
  assert.match(section, /Lamp firmware 0.1.0 is bundled with this release\./);
  assert.doesNotMatch(section, /unchanged|was 0.1.0/);
});

test('inherited firmware versions and non-root firmware workspaces fail before writes', (t) => {
  const root = fixture(t, { tagged: false });
  const path = join(root, 'firmware/pala/Cargo.toml');
  const original = readFileSync(path, 'utf8');
  for (const text of [
    original.replace('version = "0.1.0"', 'version.workspace = true'),
    original.replace('[workspace]\nresolver = "2"\n', ''),
  ]) {
    writeFileSync(path, text);
    assert.throws(
      () => main(['--app', 'patch', '--write'], root),
      /Explicit X.Y.Z|explicit workspace root/,
    );
    assert.match(readFileSync(join(root, 'Cargo.toml'), 'utf8'), /version = "0.1.0"/);
  }
});

test('single-product CLI preserves the existing first release title and body byte for byte', (t) => {
  const root = fixture(t, { tagged: false });
  assert.equal(
    info(['--version', '0.1.0'], root),
    'Kivori 0.1.0 · Pip firmware 0.1.0\n\n### First release\n\nKivori 0.1.0, with Kivori Pip firmware 0.1.0.\n\nSee the README for what Kivori does.\n\n',
  );
});

test('missing locks fail before first-release or bumped versions and outputs are written', (t) => {
  for (const lock of ['Cargo.lock', 'devices/lamp firmware/Cargo.lock']) {
    const root = twoProducts(t, { tagged: false });
    rmSync(join(root, lock));
    const outputs = outputFile(t);
    assert.throws(
      () =>
        main(
          [
            '--app',
            'patch',
            '--firmware-overrides',
            '{"pala":"patch","lamp":"patch"}',
            '--write',
            '--outputs',
            outputs,
          ],
          root,
        ),
      /lock file|locked/,
    );
    assert.equal(existsSync(outputs), false);
    assert.match(readFileSync(join(root, 'Cargo.toml'), 'utf8'), /version = "0.1.0"/);
    assert.match(
      readFileSync(join(root, 'devices/lamp firmware/Cargo.toml'), 'utf8'),
      /version = "0.4.0"/,
    );
    assert.equal(existsSync(join(root, lock)), false);
  }
});

test('actual compute and preview workflow steps preserve dry-run input and version-map semantics', (t) => {
  const root = twoProducts(t, { tagged: false });
  for (const script of [
    'release-bump.mjs',
    'release-info.mjs',
    'release-catalog.mjs',
    'firmware-release-guard.mjs',
    'build-firmware.mjs',
  ])
    put(root, `scripts/${script}`, readFileSync(new URL(`./${script}`, import.meta.url), 'utf8'));
  commit(root, 'test: release scripts');
  const outputs = outputFile(t);
  writeFileSync(outputs, '');
  const extra = {
    APP_BUMP: 'none',
    APP_VERSION: '',
    PRODUCT: '',
    FIRMWARE_BUMP: 'none',
    FIRMWARE_VERSION: '',
    FIRMWARE_OVERRIDES: '',
    DRY_RUN: 'true',
    GITHUB_OUTPUT: outputs,
    GITHUB_REPOSITORY: 'test/repo',
    GITHUB_STEP_SUMMARY: join(dirname(outputs), 'summary.md'),
  };
  runWorkflowStep('Compute the versions', root, outputs, extra);
  assert.equal(readOutputs(outputs).commit_needed, 'false');
  runWorkflowStep('Release notes preview', root, outputs, {
    DRY_RUN: 'true',
    GITHUB_REPOSITORY: 'test/repo',
    GITHUB_STEP_SUMMARY: extra.GITHUB_STEP_SUMMARY,
  });
  assert.equal(
    readFileSync(join(dirname(outputs), 'release-info/title.txt'), 'utf8'),
    'Kivori 0.1.0 · Pip firmware 0.1.0 · Lamp firmware 0.4.0',
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
  writeFileSync(outputs, '');
  runWorkflowStep('Compute the versions', root, outputs, {
    ...extra,
    APP_BUMP: 'patch',
    FIRMWARE_OVERRIDES: '{"pala":"patch","lamp":"0.5.0"}',
  });
  assert.deepEqual(JSON.parse(readOutputs(outputs).firmware_versions), {
    pala: '0.1.1',
    lamp: '0.5.0',
  });
  runWorkflowStep('Release notes preview', root, outputs, {
    DRY_RUN: 'true',
    GITHUB_REPOSITORY: 'test/repo',
    GITHUB_STEP_SUMMARY: extra.GITHUB_STEP_SUMMARY,
  });
  assert.match(
    readFileSync(join(dirname(outputs), 'release-info/title.txt'), 'utf8'),
    /Pip firmware 0.1.1 · Lamp firmware 0.5.0/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
});
