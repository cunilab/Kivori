#!/usr/bin/env bun
// Finalize only a reviewed version PR's exact, CI-green main commit. Never push a branch.
import { execFileSync } from 'node:child_process';
import { appendFileSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual, parseArgs } from 'node:util';
import { main as planRelease, tomlVersion } from './release-bump.mjs';
import { nestedLockfiles, releaseCatalog } from './release-catalog.mjs';

const run = (root, command, args) =>
  execFileSync(command, args, {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'pipe'],
  }).trim();
const git = (root, ...args) => run(root, 'git', args);

export function releaseCommit(pr, repository) {
  const version = /^release\/(v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))$/.exec(pr.head?.ref);
  if (
    pr.user?.login !== 'github-actions[bot]' ||
    !pr.merged_at ||
    pr.base?.ref !== 'main' ||
    pr.base?.repo?.full_name !== repository ||
    pr.head?.repo?.full_name !== repository ||
    !version ||
    pr.title !== `chore(release): ${version[1]}` ||
    !/^[a-f0-9]{40}$/.test(pr.merge_commit_sha)
  )
    throw new Error('Finalize requires a merged, same-repository release/vX.Y.Z PR into main.');
  return { sha: pr.merge_commit_sha, tag: version[1] };
}

export function requireMainCi(sha, workflows) {
  for (const name of ['host', 'firmware', 'determinism']) {
    const latest = workflows[name]?.find(
      (r) => r.head_sha === sha && r.head_branch === 'main' && r.event === 'push',
    );
    if (!latest || latest.status !== 'completed' || latest.conclusion !== 'success')
      throw new Error(`${name} push CI must pass on ${sha}; wait or rerun it before finalizing.`);
  }
}

/** Validate the actual squash commit, not the PR API's mutable file list. */
export function requireVersionOnly(root, sha, products) {
  const parents = git(root, 'rev-list', '--parents', '-n', '1', sha).split(' ');
  if (parents.length !== 2) throw new Error('A version PR must be squash-merged with one parent.');
  const parent = parents[1];
  const manifestPaths = [
    'Cargo.toml',
    ...new Set(products.flatMap((p) => p.crates.map((c) => `${c}/Cargo.toml`))),
  ];
  const allowed = new Set([
    ...manifestPaths.flatMap((path) => [path, path.replace(/Cargo\.toml$/, 'Cargo.lock')]),
    ...nestedLockfiles(root, parent, [...new Set(products.flatMap((p) => p.crates))]),
  ]);
  const files = git(root, 'diff', '--name-only', '-z', parent, sha).split('\0').filter(Boolean);
  if (!files.length || files.some((file) => !allowed.has(file)))
    throw new Error('A version PR may change only catalog manifests and lockfiles.');
  const read = (ref, path) => Bun.TOML.parse(git(root, 'show', `${ref}:${path}`));
  const oldRoot = read(parent, 'Cargo.toml');
  for (const path of manifestPaths) {
    const before = read(parent, path);
    const after = read(sha, path);
    const table = path === 'Cargo.toml' ? 'workspace.package' : 'package';
    const get = (value) => table.split('.').reduce((value, key) => value[key], value);
    get(before).version = get(after).version;
    if (!isDeepStrictEqual(before, after))
      throw new Error(`Non-version manifest changes in ${path}.`);
  }
  const changes = [];
  const seen = new Set();
  for (const cwd of ['.', ...new Set(products.flatMap((p) => p.crates))]) {
    const metadata = JSON.parse(
      run(join(root, cwd), 'cargo', [
        'metadata',
        '--locked',
        '--all-features',
        '--format-version',
        '1',
      ]),
    );
    for (const pkg of metadata.packages) {
      if (pkg.source || seen.has(pkg.manifest_path)) continue;
      seen.add(pkg.manifest_path);
      const path = relative(root, pkg.manifest_path).split(sep).join('/');
      if (path.startsWith('../'))
        throw new Error('Release path dependency is outside the checkout.');
      const before = read(parent, path).package;
      const old =
        typeof before.version === 'string' ? before.version : oldRoot.workspace.package.version;
      if (old !== pkg.version) changes.push({ name: pkg.name, old, next: pkg.version });
    }
  }
  for (const path of files.filter((path) => path.endsWith('Cargo.lock'))) {
    const before = read(parent, path);
    const after = read(sha, path);
    for (const pkg of before.package ?? []) {
      for (const change of changes) {
        if (!pkg.source && pkg.name === change.name && pkg.version === change.old)
          pkg.version = change.next;
        pkg.dependencies = pkg.dependencies?.map((dep) =>
          dep === `${change.name} ${change.old}` ? `${change.name} ${change.next}` : dep,
        );
        if (pkg.dependencies === undefined) delete pkg.dependencies;
      }
    }
    if (!isDeepStrictEqual(before, after))
      throw new Error(`Non-version lockfile changes in ${path}.`);
  }
}

export function pushReleaseTags(root, appTag, firmware) {
  const sha = git(root, 'rev-parse', 'HEAD');
  const tags = [appTag, ...firmware.map((p) => p.tag).filter(Boolean)];
  const missing = [];
  // Validate every existing tag before creating or pushing any missing ones.
  for (const tag of tags) {
    if (!/^(v|fw-[a-z0-9-]+-v)(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(tag))
      throw new Error(`Invalid release tag: ${tag}`);
    try {
      if (git(root, 'rev-parse', '--verify', `refs/tags/${tag}^{commit}`) !== sha)
        throw new Error(`Tag ${tag} already identifies another commit; tags are immutable.`);
    } catch (error) {
      if (error.status === undefined) throw error;
      missing.push(tag);
    }
  }
  for (const tag of missing)
    git(
      root,
      '-c',
      'user.name=github-actions[bot]',
      '-c',
      'user.email=41898282+github-actions[bot]@users.noreply.github.com',
      'tag',
      '-a',
      tag,
      '-m',
      tag,
    );
  git(root, 'push', '--atomic', 'origin', ...tags.map((tag) => `refs/tags/${tag}`));
}

export function main(argv, root = process.cwd()) {
  const { values: opt } = parseArgs({
    args: argv,
    options: { pr: { type: 'string' }, outputs: { type: 'string' } },
  });
  if (!/^[1-9]\d*$/.test(opt.pr ?? '')) throw new Error('--pr must name the merged version PR.');
  const repository =
    process.env.GITHUB_REPOSITORY ||
    run(root, 'gh', ['repo', 'view', '--json', 'nameWithOwner', '--jq', '.nameWithOwner']);
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error('Invalid repository.');
  if (git(root, 'status', '--porcelain')) throw new Error('Finalize requires a clean checkout.');
  const pr = JSON.parse(run(root, 'gh', ['api', `repos/${repository}/pulls/${opt.pr}`]));
  const { sha, tag } = releaseCommit(pr, repository);
  git(root, 'fetch', 'origin', 'main', '--tags');
  git(root, 'merge-base', '--is-ancestor', sha, 'origin/main');
  const workflows = {};
  for (const name of ['host', 'firmware', 'determinism']) {
    workflows[name] = JSON.parse(
      run(root, 'gh', [
        'api',
        `repos/${repository}/actions/workflows/${name}.yml/runs?head_sha=${sha}&event=push&per_page=100`,
      ]),
    ).workflow_runs;
  }
  requireMainCi(sha, workflows);
  git(root, 'checkout', '--detach', sha);
  if (
    `v${tomlVersion(readFileSync(join(root, 'Cargo.toml'), 'utf8'), 'workspace.package')}` !== tag
  )
    throw new Error('The release PR branch version does not match its merged workspace version.');
  const products = releaseCatalog(root);
  requireVersionOnly(root, sha, products);
  const directory = mkdtempSync(join(tmpdir(), 'kivori-finalize-'));
  try {
    let existing = '';
    try {
      existing = git(root, 'rev-parse', '--verify', `refs/tags/${tag}^{commit}`);
    } catch {
      /* first finalize */
    }
    if (existing) {
      if (existing !== sha) throw new Error(`Tag ${tag} already identifies another commit.`);
      // A retry rebuilds the same draft; every current firmware lineage must already be tagged.
      for (const product of products)
        git(root, 'merge-base', '--is-ancestor', `fw-${product.product}-v${product.version}`, sha);
    } else {
      const output = join(directory, 'plan');
      planRelease(['--app', 'none', '--firmware', 'none', '--dry-run', '--outputs', output], root);
      const plan = Object.fromEntries(
        readFileSync(output, 'utf8')
          .trim()
          .split('\n')
          .map((line) => {
            const i = line.indexOf('=');
            return [line.slice(0, i), line.slice(i + 1)];
          }),
      );
      if (plan.app_tag !== tag || plan.commit_needed !== 'false')
        throw new Error('The merged release still needs version edits.');
      pushReleaseTags(root, tag, JSON.parse(plan.firmware));
    }
    if (opt.outputs) appendFileSync(opt.outputs, `tag=${tag}\n`);
    console.log(`Finalized ${tag} at ${sha}; publication remains manual.`);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(`release-finalize: ${error.message}`);
    process.exitCode = 1;
  }
}
