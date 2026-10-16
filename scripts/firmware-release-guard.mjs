import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, readFileSync, realpathSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, sep } from 'node:path';

const run = (root, command, args) =>
  execFileSync(command, args, { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
const git = (root, ...args) => run(root, 'git', ['--literal-pathspecs', ...args]);

/** Cargo supplies path/build dependencies; build.rs and packaging supply the non-Cargo inputs. */
export function firmwareInputs(root, product) {
  root = realpathSync(root);
  const firmware = join(root, 'firmware', product, 'Cargo.toml');
  const directories = [root];
  for (let directory = dirname(firmware); directory !== root; directory = dirname(directory)) {
    if (dirname(directory) === directory)
      throw new Error('firmware directory is outside the repository');
    directories.push(directory);
  }
  for (const directory of directories) {
    for (const file of ['config', 'config.toml']) {
      const config = join(directory, '.cargo', file);
      if (
        existsSync(config) &&
        /^\s*(paths|"paths"|'paths')\s*=/m.test(readFileSync(config, 'utf8'))
      ) {
        throw new Error(
          `Cargo config paths overrides are unsupported by the firmware guard: ${config}`,
        );
      }
    }
  }
  const metadata = (manifest) =>
    JSON.parse(
      run(dirname(manifest), 'cargo', [
        'metadata',
        '--locked',
        '--all-features',
        '--format-version',
        '1',
        '--manifest-path',
        manifest,
      ]),
    );
  const workspace = metadata(join(root, 'Cargo.toml'));
  const graph = metadata(firmware);
  const packages = new Map(graph.packages.map((pkg) => [pkg.id, pkg]));
  const nodes = new Map(graph.resolve.nodes.map((node) => [node.id, node]));
  // shortcut: non-Cargo inputs are explicit; update this set when build or packaging reads new paths.
  const paths = new Set([
    'Cargo.toml',
    'Cargo.lock',
    '.cargo',
    'rust-toolchain',
    'rust-toolchain.toml',
    'products',
    'assets',
    'justfile',
    'scripts/bundle-macos.sh',
    'scripts/build-desktop-firmware.ps1',
    'scripts/build-firmware.mjs',
    'scripts/build-firmware-set.mjs',
    '.github/workflows/release.yml',
    '.github/workflows/release-start.yml',
    '.github/workflows/firmware.yml',
  ]);
  for (const directory of directories) {
    paths.add(relative(root, join(directory, '.cargo')).split(sep).join('/'));
  }
  const visited = new Set();
  const local = new Set();
  const visit = (id) => {
    if (visited.has(id)) return;
    const pkg = packages.get(id);
    const node = nodes.get(id);
    if (!pkg || !node) throw new Error(`Cargo did not resolve ${id}`);
    visited.add(id);
    const manifest = realpathSync(pkg.manifest_path);
    const path = relative(root, dirname(manifest));
    const inside = path && !isAbsolute(path) && path !== '..' && !path.startsWith(`..${sep}`);
    if (inside) {
      local.add(manifest);
      paths.add(path.split(sep).join('/'));
    } else if (pkg.source === null) {
      throw new Error(`firmware dependency must be inside the repository: ${manifest}`);
    }
    for (const dep of node.deps) {
      if (dep.dep_kinds.some((kind) => kind.kind !== 'dev')) visit(dep.pkg);
    }
  };
  visit(graph.resolve.root);

  // Version reflection makes an app version bump a firmware input, even through build helpers.
  const shared = [...local].filter((p) => p !== firmware).map((p) => relative(root, dirname(p)));
  const uses = (pattern, paths) => {
    if (!paths.length) return false;
    paths = paths.map((path) => path.split(sep).join('/'));
    // Separate Cargo test/example/bench targets do not contribute to the shipped library or build script.
    const exclusions = paths.flatMap((path) =>
      ['tests', 'examples', 'benches'].map((directory) => `:(exclude,literal)${path}/${directory}`),
    );
    const probe = spawnSync(
      'git',
      [
        'grep',
        '-I',
        '-E',
        '-l',
        pattern,
        '--',
        ...paths.map((p) => `:(literal)${p}`),
        ...exclusions,
      ],
      {
        cwd: root,
        encoding: 'utf8',
      },
    );
    if (probe.error || ![0, 1].includes(probe.status)) {
      throw new Error('could not check shared-crate version usage');
    }
    return probe.status === 0;
  };
  const versionHelper = [...visited].some((id) =>
    /^(clap($|_)|vergen($|-)|built$)/.test(packages.get(id).name),
  );
  const bookkeepingNames = new Set();
  if (
    !versionHelper &&
    !uses('CARGO_PKG_|crate_version|vergen|built::|command.*version', shared) &&
    !uses('["\'][^"\']*Cargo\\.(toml|lock)["\']', [relative(root, dirname(firmware)), ...shared])
  ) {
    for (const pkg of workspace.packages) {
      if (
        workspace.workspace_members.includes(pkg.id) &&
        /^version\.workspace\s*=\s*true\s*$/m.test(readFileSync(pkg.manifest_path, 'utf8'))
      )
        bookkeepingNames.add(pkg.name);
    }
  }
  return { paths: [...paths], bookkeepingNames };
}

function workspaceVersion(text) {
  return /\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms.exec(text)?.[1];
}

export function normalizeBookkeeping(path, text, version, names) {
  if (!version || names.size === 0) return text;
  if (path === 'Cargo.toml') {
    return text.replace(
      /(\[workspace\.package\][^[]*?^version\s*=\s*")[^"]+(".*)/ms,
      '$1WORKSPACE_VERSION$2',
    );
  }
  if (!path.endsWith('Cargo.lock')) return text;
  return text
    .split(/(?=^\[\[package\]\])/m)
    .map((block) => {
      const name = /^name = "([^"]+)"$/m.exec(block)?.[1];
      if (!names.has(name) || /^(source|checksum)\s*=/m.test(block)) return block;
      return block.replace(/^version = "([^"]+)"$/m, (line, value) =>
        value === version ? 'version = "WORKSPACE_VERSION"' : line,
      );
    })
    .join('');
}

/** Refuse stale firmware before the caller writes versions, outputs, commits or tags. */
export function checkFirmwareRelease(root, product, previous, firmwareVersion, tags) {
  if (git(root, 'rev-parse', '--is-shallow-repository').trim() !== 'false') {
    throw new Error('firmware guard needs full Git history and tags; fetch with fetch-depth: 0');
  }
  const prefix = `fw-${product}-v`;
  const productTags = tags.filter((tag) => tag.startsWith(prefix));
  for (const tag of productTags) {
    if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(tag.slice(prefix.length))) {
      throw new Error(`unsupported firmware tag ${tag}; cannot establish a release baseline`);
    }
    const ancestor = spawnSync(
      'git',
      ['merge-base', '--is-ancestor', `refs/tags/${tag}^{commit}`, 'HEAD'],
      {
        cwd: root,
        encoding: 'utf8',
      },
    );
    if (ancestor.error || ancestor.status !== 0) {
      throw new Error(
        `firmware tag ${tag} is not on HEAD's ancestry; resolve divergent or missing history`,
      );
    }
  }
  if (!productTags.length) return 'first release (no firmware tags)';
  const baseline = `${prefix}${previous}`;
  if (!previous || !productTags.includes(baseline)) throw new Error('missing firmware baseline');
  if (firmwareVersion !== previous)
    return `new firmware version ${firmwareVersion} after ${baseline}`;

  const { paths, bookkeepingNames } = firmwareInputs(root, product);
  const dirty = git(root, 'status', '--porcelain', '--untracked-files=all', '--', ...paths).trim();
  if (dirty)
    throw new Error(`commit firmware inputs before checking an unchanged release:\n${dirty}`);
  const changed = git(
    root,
    'diff',
    '--no-renames',
    '--name-only',
    '-z',
    baseline,
    'HEAD',
    '--',
    ...paths,
  )
    .split('\0')
    .filter(Boolean);
  const beforeRoot = git(root, 'show', `${baseline}:Cargo.toml`);
  const afterRoot = git(root, 'show', 'HEAD:Cargo.toml');
  const offending = changed.filter((path) => {
    if (
      path !== 'Cargo.toml' &&
      path !== 'Cargo.lock' &&
      path !== `firmware/${product}/Cargo.lock`
    ) {
      return true;
    }
    const before = spawnSync('git', ['show', `${baseline}:${path}`], {
      cwd: root,
      encoding: 'utf8',
    });
    const after = spawnSync('git', ['show', `HEAD:${path}`], { cwd: root, encoding: 'utf8' });
    if (before.status !== 0 || after.status !== 0) return true;
    const mode = git(root, 'diff', '--raw', '--no-abbrev', baseline, 'HEAD', '--', path);
    if (!mode.startsWith(':100644 100644 ')) return true;
    return (
      normalizeBookkeeping(path, before.stdout, workspaceVersion(beforeRoot), bookkeepingNames) !==
      normalizeBookkeeping(path, after.stdout, workspaceVersion(afterRoot), bookkeepingNames)
    );
  });
  if (offending.length) {
    const commits = git(
      root,
      'log',
      '--format=%h %s',
      `${baseline}..HEAD`,
      '--',
      ...offending,
    ).trim();
    throw new Error(
      `${product} firmware inputs changed since ${baseline}, but resolved firmware version ${firmwareVersion} is unchanged. Choose a firmware bump or a newer exact version.\n` +
        `Inputs:\n${offending.map((p) => `  ${p}`).join('\n')}\nCommits:\n${commits}`,
    );
  }
  return `inputs unchanged since ${baseline} (version bookkeeping excluded)`;
}
