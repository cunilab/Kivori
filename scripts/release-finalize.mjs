#!/usr/bin/env bun
// Finalize only a reviewed version PR's exact, CI-green main commit. Never push a branch.
import { execFileSync } from 'node:child_process';
import { appendFileSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { main as planRelease, tomlVersion } from './release-bump.mjs';
import { releaseCatalog } from './release-catalog.mjs';

const run = (root, command, args) =>
  execFileSync(command, args, {
    cwd: root,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  }).trim();
const git = (root, ...args) => run(root, 'git', args);

export function releaseCommit(pr, repository) {
  const version = /^release\/(v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))$/.exec(pr.head?.ref);
  if (
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
  for (const tag of missing) git(root, 'tag', '-a', tag, '-m', tag);
  if (missing.length)
    git(root, 'push', '--atomic', 'origin', ...missing.map((tag) => `refs/tags/${tag}`));
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
  const allowed = new Set([
    'Cargo.toml',
    'Cargo.lock',
    ...products.flatMap((p) => p.crates.flatMap((c) => [`${c}/Cargo.toml`, `${c}/Cargo.lock`])),
  ]);
  const files = run(root, 'gh', [
    'api',
    '--paginate',
    `repos/${repository}/pulls/${opt.pr}/files?per_page=100`,
    '--jq',
    '.[].filename',
  ])
    .split('\n')
    .filter(Boolean);
  if (!files.length || files.some((file) => !allowed.has(file)))
    throw new Error('A version PR may change only catalog manifests and lockfiles.');
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
      git(root, 'config', 'user.name', 'github-actions[bot]');
      git(root, 'config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com');
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
