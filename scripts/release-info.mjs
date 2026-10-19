#!/usr/bin/env bun
// Release title and notes for an app release (ADR-0014), built from the conventional commits since the
// previous app tag. Used by .github/workflows/release.yml and, for the dry-run preview, by
// .github/workflows/release-start.yml.
//
//   bun scripts/release-info.mjs --version 0.2.0 [--prev-tag v0.1.0] [--to HEAD] \
//        [--firmware-versions JSON_MAP] [--repo cunilab/Kivori] [--out-dir DIR]
//
// Writes DIR/title.txt, DIR/notes.md (the release body) and DIR/notes-short.md (the same without the
// compare link, for latest.json); without --out-dir it prints title and notes. The firmware version
// comes from every catalog firmware crate (or a complete --firmware-versions map for preview) and the names from products/<product>/product.toml; nothing is hardcoded. Without a
// previous app tag (the first release) the body is a short "First release" note naming both versions
// and pointing to the README, not the whole history.
//
// Grouping: feat -> Features, fix -> Fixes, perf -> Performance, `!` or a BREAKING CHANGE footer ->
// Breaking changes; other types (chore, docs, test, refactor, ci, build, style, revert) and non
// conventional subjects are left out. A commit is listed for the app when it touches a file outside
// the firmware, hardware, website and docs folders and its scope is not `web`; it is listed
// under Firmware when it touches the product's catalog crate paths and the version changed since the
// previous app tag. The owner reviews (and can edit) the draft before publishing it.
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { releaseCatalog } from './release-catalog.mjs';
import { tomlVersion } from './release-bump.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/** Paths that never make a commit an app change on their own. */
export const APP_IGNORED_PATHS = [
  'firmware/',
  'hardware/',
  'apps/web/',
  'docs/',
  '.github/',
  'README.md',
];

/** Scopes that name something other than the app (the website), left out of the app list. */
export const APP_IGNORED_SCOPES = ['web'];

/** Earlier homes of a product's firmware crate, so history before a folder move still counts. */
export const LEGACY_FIRMWARE_DIRS = { pala: ['firmware/desk-buddy/', 'firmware/esp32-c3/'] };

export const firmwareDirs = (product) => [
  `firmware/${product}/`,
  ...(LEGACY_FIRMWARE_DIRS[product] ?? []),
];

/** The `git log` format parseGitLog reads (use with --name-only). */
export const GIT_LOG_FORMAT = '%x1e%h%x1f%s%x1f%b%x1f';

/** [{ hash, subject, body, files }] from `git log --format=GIT_LOG_FORMAT --name-only`. */
export function parseGitLog(raw) {
  return raw
    .split('\x1e')
    .filter((r) => r.trim())
    .map((record) => {
      const [hash, subject = '', body = '', files = ''] = record.split('\x1f');
      return {
        hash: hash.trim(),
        subject: subject.trim(),
        body: body.trim(),
        files: files
          .split('\n')
          .map((f) => f.trim())
          .filter(Boolean),
      };
    });
}

const HEADER = /^(\w+)(?:\(([^)]*)\))?(!)?:\s*(.+)$/;

/** { type, scope, breaking, description } of a conventional commit, or null. */
export function parseConventional({ subject, body = '' }) {
  const m = HEADER.exec(subject);
  if (!m) return null;
  return {
    type: m[1].toLowerCase(),
    scope: m[2] ?? '',
    breaking: Boolean(m[3]) || /^BREAKING[ -]CHANGE:/m.test(body),
    description: m[4].trim(),
  };
}

const SECTIONS = [
  ['breaking', 'Breaking changes'],
  ['feat', 'Features'],
  ['fix', 'Fixes'],
  ['perf', 'Performance'],
];

/** Milestone scopes (`m2.7`) say nothing to a user, so they are dropped from the line. */
function entryLine({ scope, description }) {
  const shown = scope && !/^m\d+(\.\d+)*$/i.test(scope) ? `${scope}: ` : '';
  return `- ${shown}${description}`;
}

/**
 * Markdown sections for the listed commits, oldest first, `heading` hashes deep (### by default).
 * '' when nothing is listed.
 */
export function groupCommits(commits, heading = '###') {
  const groups = new Map(SECTIONS.map(([key]) => [key, []]));
  const seen = new Set();
  for (const commit of commits) {
    const c = parseConventional(commit);
    if (!c) continue;
    const key = c.breaking ? 'breaking' : c.type;
    if (!groups.has(key)) continue;
    const line = entryLine(c);
    if (seen.has(line)) continue;
    seen.add(line);
    groups.get(key).push(line);
  }
  return SECTIONS.filter(([key]) => groups.get(key).length)
    .map(([key, title]) => `${heading} ${title}\n\n${groups.get(key).join('\n')}`)
    .join('\n\n');
}

const touches = (commit, prefixes) =>
  commit.files.some((f) => prefixes.some((p) => (p.endsWith('/') ? f.startsWith(p) : f === p)));

/** A commit with no file list (a fixture, an empty commit) counts for the app. */
export function isAppCommit(commit, firmwarePaths = []) {
  const scope = parseConventional(commit)?.scope.toLowerCase() ?? '';
  if (APP_IGNORED_SCOPES.includes(scope)) return false;
  return (
    commit.files.length === 0 ||
    commit.files.some((f) => !touches({ files: [f] }, [...APP_IGNORED_PATHS, ...firmwarePaths]))
  );
}

export const isFirmwareCommit = (commit, product) => touches(commit, firmwareDirs(product));

/** `version = "x"` of the `[package]` table. */
export function packageVersion(cargoToml) {
  return tomlVersion(cargoToml, 'package');
}

/** The product's `short_name`, the customer name without the brand. */
export function productName(productToml) {
  const m = /^short_name\s*=\s*"([^"]*)"/m.exec(productToml);
  if (!m || !m[1].trim()) throw new Error('no short_name found');
  return m[1].trim();
}

/** The product's `display_name`, the full customer name. */
export function productDisplayName(productToml) {
  const m = /^display_name\s*=\s*"([^"]*)"/m.exec(productToml);
  if (!m || !m[1].trim()) throw new Error('no display_name found');
  return m[1].trim();
}

export function releaseTitle(appVersion, products) {
  return (
    `Kivori ${appVersion} · ` + products.map((p) => `${p.name} firmware ${p.version}`).join(' · ')
  );
}

/**
 * The body of the first release (no previous app tag): both versions and a pointer to the README,
 * which describes the features, instead of every commit since the repository began.
 */
export function firstReleaseNotes({ version, products, repo = '' }) {
  const readme = repo
    ? `the [README](https://github.com/${repo}/blob/v${version}/README.md)`
    : 'the README';
  const short = `### First release

Kivori ${version}, with ${products.map((p) => `${p.display} firmware ${p.version}`).join(', ')}.

See ${readme} for what Kivori does.
`;
  return { notes: short, short };
}

/**
 * The release body. `products[].previous` is the firmware version at the previous app tag ('' when
 * unknown, which counts as changed); `prevVersion` the previous app version (a first release, with
 * none, uses [`firstReleaseNotes`] instead). Returns { notes, short }: `short` leaves out the compare
 * link.
 */
export function releaseNotes({ commits, products, prevVersion = '', compareUrl = '' }) {
  const firmwarePaths = products.flatMap((p) => p.dirs);
  const app = groupCommits(commits.filter((c) => isAppCommit(c, firmwarePaths)));
  const parts = [app || 'No user-facing changes in the app.'];

  const since = prevVersion ? ` since Kivori ${prevVersion}` : '';
  for (const product of products) {
    const { name, version, previous, dirs } = product;
    let firmware = `### ${products.length === 1 ? 'Firmware' : `${name} firmware`}\n\n${name} firmware ${version} is bundled with this release`;
    if (previous === version) firmware += `, unchanged${since}.`;
    else {
      firmware += previous ? ` (was ${previous}).` : '.';
      const fw = groupCommits(
        commits.filter((c) => touches(c, dirs)),
        '####',
      );
      firmware += `\n\n${fw || `No firmware features or fixes are listed${since}.`}`;
    }
    parts.push(firmware);
  }

  const short = parts.join('\n\n') + '\n';
  const notes = compareUrl ? `${short}\n**Full changelog:** ${compareUrl}\n` : short;
  return { notes, short };
}

function git(root, ...args) {
  return execFileSync('git', ['-c', 'log.showSignature=false', ...args], {
    cwd: root,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
    maxBuffer: 64 * 1024 * 1024,
  });
}

/** The firmware version at a git ref, from the current or a legacy firmware folder; '' if none. */
function firmwareVersionAt(root, ref, product) {
  try {
    git(root, 'cat-file', '-e', `${ref}:products/${product.product}/product.toml`);
  } catch {
    if (!LEGACY_FIRMWARE_DIRS[product.product]) return '';
  }
  // shortcut: versions follow current crate paths; read historical catalog recipes when crates move.
  for (const dir of [
    ...product.crates.map((c) => `${c}/`),
    ...(LEGACY_FIRMWARE_DIRS[product.product] ?? []),
  ]) {
    try {
      return packageVersion(git(root, 'show', `${ref}:${dir}Cargo.toml`));
    } catch {
      // not there at that ref; try the next folder
    }
  }
  return '';
}

export function main(argv, directory = root) {
  const root = directory;
  const { values: opt } = parseArgs({
    args: argv,
    options: {
      version: { type: 'string' },
      'prev-tag': { type: 'string', default: '' },
      to: { type: 'string', default: 'HEAD' },
      'firmware-versions': { type: 'string' },
      repo: { type: 'string', default: '' },
      'out-dir': { type: 'string' },
    },
  });
  if (!opt.version) throw new Error('--version is required');
  const catalog = releaseCatalog(root);
  const versions = opt['firmware-versions'] ? JSON.parse(opt['firmware-versions']) : {};
  if (
    !versions ||
    typeof versions !== 'object' ||
    Array.isArray(versions) ||
    (opt['firmware-versions'] && Object.keys(versions).length !== catalog.length) ||
    Object.keys(versions).some((id) => !catalog.some((p) => p.product === id)) ||
    Object.values(versions).some(
      (v) => typeof v !== 'string' || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(v),
    )
  )
    throw new Error('Invalid catalog firmware version overrides');
  const products = catalog.map((p) => ({ ...p, version: versions[p.product] ?? p.version }));

  const prev = opt['prev-tag'];
  // A first release lists no commits, so the history is read only when there is a previous tag.
  const commits = prev
    ? parseGitLog(
        git(
          root,
          'log',
          '--no-merges',
          '--reverse',
          `--format=${GIT_LOG_FORMAT}`,
          '--name-only',
          `${prev}..${opt.to}`,
        ),
      )
    : [];

  const title = releaseTitle(opt.version, products);
  const { notes, short } = prev
    ? releaseNotes({
        commits,
        products: products.map((p) => ({
          ...p,
          previous: firmwareVersionAt(root, prev, p),
          dirs: [...p.crates.map((c) => `${c}/`), ...(LEGACY_FIRMWARE_DIRS[p.product] ?? [])],
        })),
        prevVersion: prev.replace(/^v/, ''),
        compareUrl: opt.repo
          ? `https://github.com/${opt.repo}/compare/${prev}...v${opt.version}`
          : '',
      })
    : firstReleaseNotes({ version: opt.version, products, repo: opt.repo });

  if (opt['out-dir']) {
    mkdirSync(opt['out-dir'], { recursive: true });
    writeFileSync(join(opt['out-dir'], 'title.txt'), title);
    writeFileSync(join(opt['out-dir'], 'notes.md'), notes);
    writeFileSync(join(opt['out-dir'], 'notes-short.md'), short);
  } else {
    console.log(title + '\n\n' + notes);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
