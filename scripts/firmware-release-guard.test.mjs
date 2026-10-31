import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
  existsSync,
  chmodSync,
} from 'node:fs';
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
  for (const manifest of ['Cargo.toml', 'firmware/pip/Cargo.toml', 'firmware/pip/sim/Cargo.toml']) {
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
    'firmware/pip/Cargo.toml',
    '[workspace]\nresolver = "2"\n[package]\nname = "test-firmware"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\ntest-shared = { path = "../../crates/shared" }\n[build-dependencies]\ntest-compiler = { path = "../../tools/asset-compiler" }\n',
  );
  put(root, 'firmware/pip/src/lib.rs', 'pub const VALUE: u8 = 1;\n');
  // A separate host workspace nested in the firmware crate pins the same path packages in its own lockfile.
  put(
    root,
    'firmware/pip/sim/Cargo.toml',
    '[workspace]\nresolver = "2"\n[package]\nname = "test-sim"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\ntest-firmware = { path = ".." }\ntest-shared = { path = "../../../crates/shared" }\n',
  );
  put(root, 'firmware/pip/sim/src/lib.rs', 'pub const VALUE: u8 = 1;\n');
  put(root, 'products/pip/product.toml', productToml('Pip', 'firmware/pip'));
  put(root, 'products/registry.toml', '[products]\npip = 1\n[boards]\nboard = 1\n');
  put(root, 'assets/mascot.svg', '<svg/>\n');
  locks(root);
  commit(root, 'initial firmware');
  if (tagged) git(root, 'tag', '-a', 'fw-pip-v0.1.0', '-m', 'firmware 0.1.0');
  return root;
}
function rejectWithoutWrites(root, pattern, extra = []) {
  const paths = ['Cargo.toml', 'firmware/pip/Cargo.toml', 'Cargo.lock', 'firmware/pip/Cargo.lock'];
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
  const { paths } = firmwareInputs(root, 'pip');
  for (const path of [
    'firmware/pip',
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
  const { paths, bookkeepingNames } = firmwareInputs(root, 'pip');
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
  const script = readFileSync(join(root, 'firmware/pip/build.rs'), 'utf8');
  const declarations = [...script.matchAll(/cargo:rerun-if-changed=([^"\n]*)/g)];
  assert.ok(declarations.length > 0);
  for (const [, path] of declarations) {
    assert.doesNotMatch(path, /[{}]/, 'dynamic build inputs need an explicit guard inventory');
    const input = relative(root, resolve(root, 'firmware/pip', path))
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
  git(root, 'tag', 'fw-pip-v0.1.0');
  put(root, 'docs/notes.md', 'Docs only.\n');
  commit(root, 'docs: add release notes');
  main(['--app', 'patch', '--dry-run'], root);
  assert.equal(git(root, 'status', '--porcelain'), '');
});

test('after the pala -> pip rename the old tags stay the baseline, and the moved folder needs a firmware bump', (t) => {
  const root = fixture(t, { tagged: false });
  // Released under the earlier id, from the earlier folder.
  git(root, 'mv', 'firmware/pip', 'firmware/pala');
  git(root, 'mv', 'products/pip', 'products/pala');
  put(root, 'products/pala/product.toml', productToml('Pip', 'firmware/pala'));
  put(root, 'products/registry.toml', '[products]\npala = 1\n[boards]\nboard = 1\n');
  commit(root, 'firmware under its codename');
  git(root, 'tag', '-a', 'fw-pala-v0.1.0', '-m', 'firmware 0.1.0');
  git(root, 'mv', 'firmware/pala', 'firmware/pip');
  git(root, 'mv', 'products/pala', 'products/pip');
  put(root, 'products/pip/product.toml', productToml('Pip', 'firmware/pip'));
  put(root, 'products/registry.toml', '[products]\npip = 1\n[boards]\nboard = 1\n');
  commit(root, 'chore: rename pala to pip');
  // The moved inputs compare as changed against fw-pala-v0.1.0, so keeping 0.1.0 is refused...
  rejectWithoutWrites(root, /fw-pala-v0\.1\.0.*resolved firmware version 0\.1\.0 is unchanged/s);
  // ...and a firmware bump continues the lineage under the new prefix.
  main(['--app', 'patch', '--firmware', 'patch', '--dry-run'], root);
  assert.equal(git(root, 'status', '--porcelain'), '');
});

test('changed inputs reject unchanged firmware before versions or outputs are written', async (t) => {
  for (const path of [
    'firmware/pip/src/lib.rs',
    'crates/shared/src/lib.rs',
    'crates/transitive/src/lib.rs',
    'tools/asset-compiler/src/lib.rs',
    'assets/mascot.svg',
    'products/registry.toml',
    'Cargo.lock',
    'firmware/pip/Cargo.lock',
    '.cargo/config.toml',
    'firmware/.cargo/config.toml',
    'rust-toolchain.toml',
    'firmware/pip/rust-toolchain.toml',
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
  put(root, 'firmware/pip/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'fix: firmware behavior');
  rejectWithoutWrites(root, /firmware inputs changed/, ['--firmware-version', '0.1.0']);
  main(['--app', 'patch', '--firmware', 'patch', '--dry-run'], root);
  main(['--app', 'patch', '--firmware', 'none', '--firmware-version', '0.1.1', '--write'], root);
  assert.match(readFileSync(join(root, 'firmware/pip/Cargo.toml'), 'utf8'), /version = "0\.1\.1"/);
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
    'firmware/pip/src/lib.rs',
    'pub const APP_MANIFEST: &str = include_str!("../../../Cargo.toml");\n',
  );
  commit(root, 'firmware embeds workspace metadata');
  git(root, 'tag', 'fw-pip-v0.1.0');
  put(root, 'Cargo.toml', readFileSync(join(root, 'Cargo.toml'), 'utf8').replace('0.1.0', '0.2.0'));
  locks(root);
  commit(root, 'chore: workspace version');
  rejectWithoutWrites(root, /firmware inputs changed/);
});

test('nested independent host packages do not disable firmware bookkeeping exemptions', (t) => {
  const root = fixture(t, { tagged: false });
  for (const directory of ['firmware/pip/host-tool', 'tools/asset-compiler/host-tool']) {
    put(
      root,
      `${directory}/Cargo.toml`,
      '[workspace]\n[package]\nname = "host-tool"\nversion = "0.1.0"\nedition = "2021"\n',
    );
    put(root, `${directory}/src/main.rs`, 'fn main() {}\n');
    put(
      root,
      `${directory}/build.rs`,
      'fn main() { let _ = std::fs::read_to_string("../Cargo.toml"); }\n',
    );
  }
  commit(root, 'add independent host packages');
  git(root, 'tag', 'fw-pip-v0.1.0');
  assert.ok(firmwareInputs(root, 'pip').bookkeepingNames.has('test-shared'));
  put(root, 'Cargo.toml', readFileSync(join(root, 'Cargo.toml'), 'utf8').replace('0.1.0', '0.2.0'));
  locks(root);
  commit(root, 'chore: workspace version');
  main(['--app', 'patch', '--dry-run'], root);
});

test('resolved build scripts reading manifests still disable bookkeeping exemptions', (t) => {
  const root = fixture(t);
  for (const directory of ['firmware/pip', 'tools/asset-compiler']) {
    put(
      root,
      `${directory}/build.rs`,
      'fn main() { let _ = std::fs::read_to_string("../Cargo.toml"); }\n',
    );
    commit(root, `add manifest read in ${directory}`);
    assert.equal(firmwareInputs(root, 'pip').bookkeepingNames.size, 0, directory);
    rmSync(join(root, directory, 'build.rs'));
    commit(root, 'remove manifest read');
  }
});

test('nested packages resolved as build dependencies remain in the bookkeeping scan', (t) => {
  const root = fixture(t);
  put(
    root,
    'firmware/pip/build-helper/Cargo.toml',
    '[workspace]\n[package]\nname = "nested-helper"\nversion = "0.1.0"\nedition = "2021"\n',
  );
  put(
    root,
    'firmware/pip/build-helper/src/lib.rs',
    'pub const MANIFEST: &str = include_str!("../../Cargo.toml");\n',
  );
  put(
    root,
    'firmware/pip/Cargo.toml',
    readFileSync(join(root, 'firmware/pip/Cargo.toml'), 'utf8').replace(
      '[workspace]',
      '[workspace]\nexclude = ["build-helper"]',
    ) + 'nested-helper = { path = "build-helper" }\n',
  );
  locks(root);
  commit(root, 'add nested build dependency reading manifest');
  const { paths, bookkeepingNames } = firmwareInputs(root, 'pip');
  assert.ok(paths.includes('firmware/pip/build-helper'));
  assert.equal(bookkeepingNames.size, 0);
});

test('shared crate reads of workspace manifests also disable bookkeeping exemptions', (t) => {
  const root = fixture(t, { tagged: false });
  put(
    root,
    'crates/shared/src/lib.rs',
    'pub const APP_MANIFEST: &str = include_str!("../../../Cargo.toml");\n',
  );
  commit(root, 'shared crate embeds workspace metadata');
  git(root, 'tag', 'fw-pip-v0.1.0');
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
    'firmware/pip/Cargo.toml',
    readFileSync(join(root, 'firmware/pip/Cargo.toml'), 'utf8').replace(
      '[dependencies]\n',
      '[dependencies]\nnew-dependency = { path = "../../custom/new" }\n',
    ),
  );
  commit(root, 'stale lockfile baseline');
  git(root, 'tag', 'fw-pip-v0.1.0');
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
    readFileSync(join(root, 'firmware/pip/Cargo.toml'), 'utf8').replace(
      '[dependencies]\n',
      '[dependencies]\npatched-dependency = "0.1.0"\n',
    ) + '\n[patch.crates-io]\npatched-dependency = { path = "../../custom/patched" }\n';
  put(root, 'firmware/pip/Cargo.toml', manifest);
  put(
    root,
    'firmware/pip/sim/Cargo.toml',
    readFileSync(join(root, 'firmware/pip/sim/Cargo.toml'), 'utf8') +
      '\n[patch.crates-io]\npatched-dependency = { path = "../../../custom/patched" }\n',
  );
  locks(root);
  commit(root, 'patch dependency baseline');
  git(root, 'tag', 'fw-pip-v0.1.0');
  assert.ok(firmwareInputs(root, 'pip').paths.includes('custom/patched'));
  put(root, 'custom/patched/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'fix: patched dependency behavior');
  rejectWithoutWrites(root, /custom\/patched\/src\/lib\.rs/);
});

test('legacy Cargo config paths overrides are refused before writes', (t) => {
  const root = fixture(t);
  put(root, 'firmware/pip/.cargo/config.toml', 'paths = ["../../crates/shared"]\n');
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
  put(root, 'firmware/pip/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'unmerged firmware release');
  git(root, 'tag', 'fw-pip-v0.1.0');
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
  put(root, 'firmware/pip/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  rejectWithoutWrites(root, /commit firmware inputs before checking/);
});

test('an older existing firmware tag cannot bypass version monotonicity', () => {
  assert.throws(
    () =>
      planRelease({
        appCurrent: '0.1.0',
        firmwareCurrent: '0.1.0',
        product: 'pip',
        appBump: 'patch',
        appExact: '',
        firmwareBump: 'none',
        firmwareExact: '',
        tags: ['v0.1.0', 'fw-pip-v0.1.0', 'fw-pip-v0.2.0'],
      }),
    /below the latest released version 0\.2\.0/,
  );
});

function twoProducts(t, options) {
  const root = fixture(t, options);
  put(root, 'products/registry.toml', '[products]\npip = 1\nlamp = 2\n[boards]\nboard = 1\n');
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
    git(root, 'tag', '-f', 'fw-pip-v0.1.0');
    git(root, 'tag', 'fw-lamp-v0.4.0');
  }
  return root;
}
test('release CLI keeps or bumps literal TOML versions without reformatting other products', (t) => {
  for (const bumpLamp of [false, true]) {
    const root = twoProducts(t);
    const path = 'devices/lamp firmware/Cargo.toml';
    const literal = readFileSync(join(root, path), 'utf8')
      .replace('[package]', '[ package ] # lamp')
      .replace('version = "0.4.0"', "version = '0.4.0' # release version")
      .replaceAll('\n', '\r\n');
    put(root, path, literal);
    const appText = readFileSync(join(root, 'Cargo.toml'), 'utf8')
      .replace('[workspace.package]', '[ workspace . package ] # app')
      .replace('version = "0.1.0"', "version = '0.1.0'");
    put(root, 'Cargo.toml', appText);
    commit(root, 'test: valid TOML formatting');
    git(root, 'tag', '-f', 'fw-pip-v0.1.0');
    git(root, 'tag', '-f', 'fw-lamp-v0.4.0');
    git(root, 'tag', 'v0.1.0');
    const outputs = outputFile(t);
    main(
      [
        '--app',
        'patch',
        '--firmware-overrides',
        JSON.stringify({ pip: 'patch', lamp: bumpLamp ? '0.5.0' : 'none' }),
        '--write',
        '--outputs',
        outputs,
      ],
      root,
    );
    assert.equal(
      readFileSync(join(root, path), 'utf8'),
      bumpLamp ? literal.replace("'0.4.0'", "'0.5.0'") : literal,
    );
    assert.equal(
      readFileSync(join(root, 'Cargo.toml'), 'utf8'),
      appText.replace("'0.1.0'", "'0.1.1'"),
    );
    const notes = info(['--version', '0.1.1', '--prev-tag', 'v0.1.0'], root);
    assert.match(
      notes,
      bumpLamp ? /Lamp firmware 0.5.0.*was 0.4.0/ : /Lamp firmware 0.4.0.*unchanged/,
    );
    assert.deepEqual(JSON.parse(readOutputs(outputs).firmware_versions), {
      pip: '0.1.1',
      lamp: bumpLamp ? '0.5.0' : '0.4.0',
    });
  }
});

test('a multiline TOML decoy fails before release versions or outputs are written', (t) => {
  const root = twoProducts(t);
  const path = 'devices/lamp firmware/Cargo.toml';
  const text = readFileSync(join(root, path), 'utf8');
  put(root, path, '[workspace.metadata]\nnote = """\n[package]\nversion = "0.4.0"\n"""\n' + text);
  commit(root, 'test: multiline table decoy');
  git(root, 'tag', '-f', 'fw-lamp-v0.4.0');
  const outputs = outputFile(t);
  assert.throws(
    () =>
      main(
        [
          '--app',
          'patch',
          '--product',
          'lamp',
          '--firmware',
          'patch',
          '--write',
          '--outputs',
          outputs,
        ],
        root,
      ),
    /Cannot safely edit/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
  assert.equal(existsSync(outputs), false);
});

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
    { product: 'pip', name: 'Pip', version: '0.1.0', tag: 'fw-pip-v0.1.0' },
    { product: 'lamp', name: 'Lamp', version: '0.4.0', tag: 'fw-lamp-v0.4.0' },
  ]);
  assert.deepEqual(JSON.parse(first.firmware_crates), ['firmware/pip', 'devices/lamp firmware']);
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
          'pip',
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
      '{"pip":"patch","lamp":"0.5.0"}',
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
      '{"pip":"patch","lamp":"0.5.0"}',
      '--write',
      '--outputs',
      outputs,
    ],
    root,
  );
  assert.equal(info(['--version', '0.1.1'], root), preview);
  assert.match(readFileSync(join(root, 'firmware/pip/Cargo.toml'), 'utf8'), /version = "0.1.1"/);
  assert.match(
    readFileSync(join(root, 'devices/lamp firmware/Cargo.toml'), 'utf8'),
    /version = "0.5.0"/,
  );
  assert.deepEqual(JSON.parse(readOutputs(outputs).firmware_versions), {
    pip: '0.1.1',
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
          '{"pip":"patch"}',
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
    ['--app', 'patch', '--firmware-overrides', '{"pip":"patch","lamp":"patch"}', '--dry-run'],
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
    ['--firmware-overrides', '{}', '--product', 'pip'],
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
      productToml('Lamp', 'firmware/pip')
        .slice(productToml('Lamp', 'firmware/pip').indexOf('[boards.'))
        .replaceAll('boards.board', 'boards.other'),
  );
  put(
    root,
    'products/registry.toml',
    '[products]\npip = 1\nlamp = 2\n[boards]\nboard = 1\nother = 2\n',
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
  put(root, 'products/lamp/product.toml', productToml('Lamp', 'firmware/pip'));
  commit(root, 'test: shared firmware crate');
  git(root, 'tag', '-d', 'fw-lamp-v0.4.0');
  git(root, 'tag', 'fw-lamp-v0.1.0');
  git(root, 'tag', '-f', 'fw-pip-v0.1.0');
  assert.throws(
    () => main(['--app', 'patch', '--product', 'pip', '--firmware', 'patch', '--write'], root),
    /Conflicting firmware versions/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
  const outputs = outputFile(t);
  main(
    [
      '--app',
      'patch',
      '--firmware-overrides',
      '{"pip":"patch","lamp":"patch"}',
      '--dry-run',
      '--outputs',
      outputs,
    ],
    root,
  );
  assert.deepEqual(JSON.parse(readOutputs(outputs).firmware_crates), ['firmware/pip']);
});

test('planned app version changes cannot bypass shared version reflection', (t) => {
  const root = fixture(t, { consumesVersion: true });
  rejectWithoutWrites(root, /firmware inputs will change/);
  main(['--app', 'patch', '--firmware', 'patch', '--dry-run'], root);
});

test('release notes cover independent firmware histories at custom catalog paths', (t) => {
  const root = twoProducts(t);
  git(root, 'tag', 'v0.1.0');
  put(root, 'firmware/pip/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
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
      '{"pip":"0.1.1","lamp":"0.5.0"}',
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
    '{"pip":"0.1.1","absent":"0.4.0"}',
    '[]',
    '{"pip":3,"lamp":"0.4.0"}',
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
    GITHUB_OUTPUT: outputs,
    ...extra,
  };
  return execFileSync('bash', ['-c', workflowStep(name)], {
    cwd: root,
    env,
    encoding: 'utf8',
    stdio: 'pipe',
  });
}

(process.platform === 'win32' ? test.skip : test)(
  'finalize CLI checks merged PR and exact main CI before tagging the reviewed SHA',
  (t) => {
    const root = twoProducts(t);
    const outputs = outputFile(t);
    const remote = join(dirname(outputs), 'origin.git');
    execFileSync('git', ['init', '--bare', '--quiet', remote]);
    git(root, 'remote', 'add', 'origin', remote);
    git(root, 'push', '--quiet', 'origin', 'main', '--tags');
    main(
      [
        '--app',
        'patch',
        '--firmware-overrides',
        '{"pip":"patch","lamp":"patch"}',
        '--write',
        '--outputs',
        outputs,
      ],
      root,
    );
    runWorkflowStep('Refresh all Cargo.lock files', root, outputs);
    commit(root, 'chore(release): v0.1.1');
    const sha = git(root, 'rev-parse', 'HEAD');
    git(root, 'push', '--quiet', 'origin', 'main');
    const pr = {
      user: { login: 'github-actions[bot]' },
      merged_at: '2026-10-20T00:00:00Z',
      merge_commit_sha: sha,
      title: 'chore(release): v0.1.1',
      head: { ref: 'release/v0.1.1', repo: { full_name: 'test/repo' } },
      base: { ref: 'main', repo: { full_name: 'test/repo' } },
    };
    const good = {
      head_sha: sha,
      head_branch: 'main',
      event: 'push',
      status: 'completed',
      conclusion: 'success',
    };
    const api = {
      pr,
      files: JSON.parse(readOutputs(outputs).release_files),
      runs: { host: [good], firmware: [good], determinism: [good] },
    };
    const bin = join(dirname(outputs), 'bin');
    mkdirSync(bin);
    const apiFile = join(dirname(outputs), 'api.json');
    const gh = join(bin, 'gh');
    writeFileSync(
      gh,
      `#!/usr/bin/env node
const fs = require('node:fs');
const data = JSON.parse(fs.readFileSync(process.env.RELEASE_API_FIXTURE, 'utf8'));
const route = process.argv.find(x => x.startsWith('repos/'));
if (route.includes('/files?')) process.stdout.write(data.files.join('\\n'));
else if (route.includes('/pulls/')) process.stdout.write(JSON.stringify(data.pr));
else {
 const name = /workflows\\/(.+)\\.yml/.exec(route)[1];
 process.stdout.write(JSON.stringify({ workflow_runs: data.runs[name] }));
}
`,
    );
    chmodSync(gh, 0o755);
    const realCargo = execFileSync('which', ['cargo'], { encoding: 'utf8' }).trim();
    const cargo = join(bin, 'cargo');
    writeFileSync(
      cargo,
      `#!/usr/bin/env node
const { execFileSync } = require('node:child_process');
const args = process.argv.slice(2);
const output = execFileSync(process.env.REAL_CARGO, args, { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
if (args[0] === 'metadata') {
 const data = JSON.parse(output);
 data.metadata = { padding: 'x'.repeat(2 * 1024 * 1024) };
 process.stdout.write(JSON.stringify(data));
} else process.stdout.write(output);
`,
    );
    chmodSync(cargo, 0o755);
    const finalize = () =>
      execFileSync(
        'bun',
        [fileURLToPath(new URL('./release-finalize.mjs', import.meta.url)), '--pr', '123'],
        {
          cwd: root,
          encoding: 'utf8',
          stdio: 'pipe',
          env: {
            ...process.env,
            GITHUB_REPOSITORY: 'test/repo',
            RELEASE_API_FIXTURE: apiFile,
            REAL_CARGO: realCargo,
            PATH: `${bin}:${process.env.PATH}`,
          },
        },
      );
    for (const bad of [
      { ...api, pr: { ...pr, merged_at: null } },
      { ...api, runs: { ...api.runs, firmware: [{ ...good, conclusion: 'failure' }] } },
      {
        ...api,
        pr: { ...pr, title: 'chore(release): v9.9.9', head: { ...pr.head, ref: 'release/v9.9.9' } },
      },
    ]) {
      writeFileSync(apiFile, JSON.stringify(bad));
      assert.throws(finalize);
      assert.equal(git(remote, 'tag', '--list', 'v0.1.1', 'fw-pip-v0.1.1', 'fw-lamp-v0.4.1'), '');
    }
    // Newer main work must not change the commit being released.
    git(root, 'checkout', '--quiet', 'main');
    put(root, 'README.md', 'Later main work.\n');
    commit(root, 'docs: later work');
    const newerMain = git(root, 'rev-parse', 'HEAD');
    git(root, 'push', '--quiet', 'origin', 'main');
    writeFileSync(apiFile, JSON.stringify(api));
    finalize();
    for (const tag of ['v0.1.1', 'fw-pip-v0.1.1', 'fw-lamp-v0.4.1'])
      assert.equal(git(remote, 'rev-parse', `${tag}^{commit}`), sha);
    assert.equal(git(remote, 'rev-parse', 'main'), newerMain);
    finalize(); // Same-SHA retry is safe and doesn't move tags.
  },
);

test('version-only validation rejects manifest behavior changes, lock changes and extra files', (t) => {
  const root = fixture(t);
  const base = git(root, 'rev-parse', 'HEAD');
  const script = new URL('./release-finalize.mjs', import.meta.url).href;
  const check = () =>
    execFileSync(
      'bun',
      [
        '--eval',
        `import { requireVersionOnly } from ${JSON.stringify(script)}; requireVersionOnly(process.argv[1], process.argv[2], [{crates:['firmware/pip']}]);`,
        root,
        git(root, 'rev-parse', 'HEAD'),
      ],
      { encoding: 'utf8', stdio: 'pipe' },
    );
  for (const [path, text] of [
    [
      'Cargo.toml',
      readFileSync(join(root, 'Cargo.toml'), 'utf8') + '\n[profile.release]\nopt-level = 0\n',
    ],
    [
      'Cargo.lock',
      readFileSync(join(root, 'Cargo.lock'), 'utf8').replace('version = 4', 'version = 3'),
    ],
    ['surprise.txt', 'Not a version edit.\n'],
  ]) {
    git(root, 'reset', '--hard', base);
    put(root, path, text);
    commit(root, 'chore(release): unsafe edit');
    assert.throws(check);
  }
});

test('version-only validation covers nested workspace lockfiles', (t) => {
  const root = fixture(t);
  const base = git(root, 'rev-parse', 'HEAD');
  const script = new URL('./release-finalize.mjs', import.meta.url).href;
  const check = () =>
    execFileSync(
      'bun',
      [
        '--eval',
        `import { requireVersionOnly } from ${JSON.stringify(script)}; requireVersionOnly(process.argv[1], process.argv[2], [{crates:['firmware/pip']}]);`,
        realpathSync(root), // cargo reports real manifest paths; the tmpdir may be a symlink
        git(root, 'rev-parse', 'HEAD'),
      ],
      { encoding: 'utf8', stdio: 'pipe' },
    );
  const lock = 'firmware/pip/sim/Cargo.lock';
  const text = readFileSync(join(root, lock), 'utf8');
  const bump = (from, to) => put(root, lock, text.replace(from, to));
  // Version-only: the nested lockfile may follow the firmware bump (see the prepare/finalize test for the full flow).
  const firmware = readFileSync(join(root, 'firmware/pip/Cargo.toml'), 'utf8');
  put(root, 'firmware/pip/Cargo.toml', firmware.replace('version = "0.1.0"', 'version = "0.1.1"'));
  bump('name = "test-firmware"\nversion = "0.1.0"', 'name = "test-firmware"\nversion = "0.1.1"');
  execFileSync('cargo', ['update', '--workspace', '--offline'], {
    cwd: join(root, 'firmware/pip'),
    stdio: 'pipe',
  });
  commit(root, 'chore(release): nested lock follows the firmware version');
  check();
  // Anything else in it, or any other nested file, is refused.
  git(root, 'reset', '--hard', base);
  bump('name = "test-sim"\nversion = "0.1.0"', 'name = "test-sim"\nversion = "0.2.0"');
  commit(root, 'chore(release): unrelated nested lock change');
  assert.throws(check, /Non-version lockfile changes in firmware\/pip\/sim\/Cargo\.lock/);
  git(root, 'reset', '--hard', base);
  put(root, 'firmware/pip/sim/src/lib.rs', 'pub const VALUE: u8 = 2;\n');
  commit(root, 'chore(release): nested source change');
  assert.throws(check, /only catalog manifests and lockfiles/);
});

test('prepare refreshes all locks and pushes only a version branch with exact files', (t) => {
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
      '{"pip":"patch","lamp":"0.5.0"}',
      '--write',
      '--outputs',
      outputs,
    ],
    root,
  );
  runWorkflowStep('Refresh all Cargo.lock files', root, outputs);
  for (const crate of ['firmware/pip', 'firmware/pip/sim', 'devices/lamp firmware'])
    assert.match(
      readFileSync(join(root, crate, 'Cargo.lock'), 'utf8'),
      /name = "test-shared"\nversion = "0.1.1"/,
    );
  assert.match(
    readFileSync(join(root, 'firmware/pip/sim/Cargo.lock'), 'utf8'),
    /name = "test-firmware"\nversion = "0.1.1"/,
  );
  assert.ok(JSON.parse(readOutputs(outputs).release_files).includes('firmware/pip/sim/Cargo.lock'));
  runWorkflowStep('Commit version branch', root, outputs);
  assert.equal(git(root, 'status', '--porcelain'), '');
  assert.equal(git(remote, 'rev-parse', 'main'), before);
  assert.equal(git(remote, 'rev-parse', 'release/v0.1.1'), git(root, 'rev-parse', 'HEAD'));
  assert.equal(git(remote, 'tag', '--list'), '');
  const changed = git(root, 'diff-tree', '--no-commit-id', '--name-only', '-r', 'HEAD')
    .split('\n')
    .sort();
  assert.deepEqual(changed, JSON.parse(readOutputs(outputs).release_files).sort());
});

test('prepare refuses a versionless first-release PR without pushing anything', (t) => {
  const root = twoProducts(t, { tagged: false });
  const before = git(root, 'rev-parse', 'HEAD');
  const outputs = outputFile(t);
  const remote = join(dirname(outputs), 'origin.git');
  execFileSync('git', ['init', '--bare', '--quiet', remote]);
  git(root, 'remote', 'add', 'origin', remote);
  git(root, 'push', '--quiet', 'origin', 'main');
  main(['--app', 'none', '--write', '--outputs', outputs], root);
  assert.throws(
    () => runWorkflowStep('Commit version branch', root, outputs),
    /No version changes/,
  );
  assert.equal(git(root, 'rev-parse', 'HEAD'), before);
  assert.equal(git(remote, 'tag', '--list'), '');
  assert.equal(git(remote, 'branch', '--list', 'release/*'), '');
});

test('unexpected lock-refresh edits and an existing release branch are refused', (t) => {
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
      '{"pip":"patch","lamp":"patch"}',
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
  git(remote, 'update-ref', 'refs/heads/release/v0.1.1', before);
  assert.throws(
    () => runWorkflowStep('Commit version branch', root, outputs),
    /release branch already exists/,
  );
  assert.equal(git(remote, 'rev-parse', 'main'), before);
  assert.equal(git(remote, 'tag', '--list', 'v0.1.1', 'fw-pip-v0.1.1'), '');
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
    '[products]\npip = 1\nlamp = 2\n[boards]\nboard = 1\nother = 2\n',
  );
  commit(root, 'feat: alternate lamp board');
  git(root, 'tag', '-f', 'fw-pip-v0.1.0');
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
    'firmware/pip',
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
      'test-firmware = { path = "../../firmware/pip" }\n',
  );
  execFileSync(
    'cargo',
    ['generate-lockfile', '--offline', '--manifest-path', 'devices/lamp firmware/Cargo.toml'],
    { cwd: root, stdio: 'pipe' },
  );
  commit(root, 'test: firmware crate dependency');
  git(root, 'tag', '-f', 'fw-pip-v0.1.0');
  git(root, 'tag', '-f', 'fw-lamp-v0.4.0');
  assert.throws(
    () => main(['--app', 'patch', '--product', 'pip', '--firmware', 'patch', '--write'], root),
    /lamp firmware inputs will change/,
  );
  assert.equal(git(root, 'status', '--porcelain'), '');
});

test('newly cataloged shared-crate products have no invented previous firmware baseline', (t) => {
  const root = twoProducts(t);
  git(root, 'tag', 'v0.1.0', 'HEAD^');
  put(root, 'products/lamp/product.toml', productToml('Lamp', 'firmware/pip'));
  commit(root, 'feat: new product sharing firmware');
  const notes = info(['--version', '0.1.1', '--prev-tag', 'v0.1.0'], root);
  const section = notes.slice(notes.indexOf('### Lamp firmware'));
  assert.match(section, /Lamp firmware 0.1.0 is bundled with this release\./);
  assert.doesNotMatch(section, /unchanged|was 0.1.0/);
});

test('inherited firmware versions and non-root firmware workspaces fail before writes', (t) => {
  const root = fixture(t, { tagged: false });
  const path = join(root, 'firmware/pip/Cargo.toml');
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
            '{"pip":"patch","lamp":"patch"}',
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
    FIRMWARE_OVERRIDES: '{"pip":"patch","lamp":"0.5.0"}',
  });
  assert.deepEqual(JSON.parse(readOutputs(outputs).firmware_versions), {
    pip: '0.1.1',
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
