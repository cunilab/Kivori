import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { firmwareInputs, normalizeBookkeeping } from './firmware-release-guard.mjs';
import { main, planRelease } from './release-bump.mjs';

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
  put(root, 'products/pala/product.toml', 'short_name = "Pip"\n');
  put(root, 'products/registry.toml', '[products]\npala = 1\n');
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
