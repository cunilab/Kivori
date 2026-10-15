#!/usr/bin/env node
// Next app and firmware versions for a release (ADR-0014). Used by .github/workflows/release-start.yml
// (the Release button); safe to run locally, where it only prints unless --write is given.
//
//   node scripts/release-bump.mjs --app none|patch|minor|major [--app-version X.Y.Z] \
//        [--firmware none|patch|minor|major] [--firmware-version A.B.C] [--product pala] \
//        [--dry-run | --write] [--outputs FILE]
//
// The app version is `[workspace.package] version` in Cargo.toml, the firmware version `[package]
// version` in firmware/<product>/Cargo.toml. A bump starts from that Cargo version; `none` keeps it
// and an exact version overrides the bump. Each new version must be greater than the latest existing
// tag of its component (`vX.Y.Z` for the app, `fw-<product>-vA.B.C` for the firmware) and never below
// its Cargo version; with no tag yet, the Cargo version may be released as it is (the first release).
// So `--app none` releases the current app version, and only while no `v<current>` tag exists.
// `--firmware none` keeps the firmware; while no `fw-<product>-v*` tag exists yet, the release also
// tags `fw-<product>-v<current>` so the firmware lineage starts tagged. Below 1.0.0 a major bump is
// still a real major (0.4.2 -> 1.0.0). The run fails when a resulting tag already exists. When neither
// version changes there is no version commit (`commit_needed=false`): the workflow tags the current
// main commit as it is. --write edits the Cargo.toml files whose version changes (the lockfiles are
// refreshed by cargo in the workflow); --outputs appends `key=value` lines for $GITHUB_OUTPUT.
import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { productName } from './release-info.mjs';
import { checkFirmwareRelease } from './firmware-release-guard.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

/** [major, minor, patch] of a plain `X.Y.Z` (no pre-release or build part), or an error. */
export function parseSemver(text) {
  const m = SEMVER.exec(String(text).trim());
  if (!m) throw new Error(`"${text}" is not a version of the form X.Y.Z`);
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

export function compareSemver(a, b) {
  const [x, y] = [parseSemver(a), parseSemver(b)];
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1;
  return 0;
}

/** Semver bump; `none` keeps the version. A major bump of 0.x gives 1.0.0. */
export function bumpVersion(version, kind) {
  const [major, minor, patch] = parseSemver(version);
  switch (kind) {
    case 'none':
      return version;
    case 'patch':
      return `${major}.${minor}.${patch + 1}`;
    case 'minor':
      return `${major}.${minor + 1}.0`;
    case 'major':
      return `${major + 1}.0.0`;
    default:
      throw new Error(`unknown bump "${kind}" (none, patch, minor or major)`);
  }
}

/** Every bump the Release button offers; `none` keeps the Cargo version. */
export const BUMPS = ['none', 'patch', 'minor', 'major'];

/**
 * The next version: the exact override when given (an optional leading `v` is accepted), else the
 * bump of `current` (the Cargo version). It is never below `current`, and it must be greater than
 * `latest`, the version of the component's latest tag ('' when it has none yet, in which case
 * `current` itself may be released).
 */
export function nextVersion(current, { bump, exact, latest = '' }) {
  const override = (exact ?? '').trim().replace(/^v/, '');
  if (override) parseSemver(override);
  const next = override || bumpVersion(current, bump);
  if (compareSemver(next, current) < 0) {
    throw new Error(`${next} is below the current version ${current} in Cargo.toml`);
  }
  if (latest && compareSemver(next, latest) <= 0) {
    throw new Error(`${next} is not greater than the latest released version ${latest}`);
  }
  return next;
}

/** The highest `<prefix>X.Y.Z` among `tags`, as `X.Y.Z`; '' when there is none. */
export function latestTagVersion(tags, prefix) {
  let best = '';
  for (const tag of tags) {
    if (!tag.startsWith(prefix)) continue;
    const version = tag.slice(prefix.length);
    if (!SEMVER.test(version)) continue;
    if (!best || compareSemver(version, best) > 0) best = version;
  }
  return best;
}

function tableBounds(lines, table) {
  const start = lines.findIndex((l) => l.trim() === `[${table}]`);
  if (start < 0) throw new Error(`no [${table}] table`);
  let end = lines.findIndex((l, i) => i > start && /^\s*\[/.test(l));
  if (end < 0) end = lines.length;
  return [start, end];
}

const VERSION_LINE = /^(\s*version\s*=\s*")([^"]*)(".*)$/;

/** `version = "x"` of a TOML table (`workspace.package`, `package`). */
export function tomlVersion(toml, table) {
  const lines = toml.split('\n');
  const [start, end] = tableBounds(lines, table);
  for (let i = start + 1; i < end; i++) {
    const m = VERSION_LINE.exec(lines[i]);
    if (m) return m[2];
  }
  throw new Error(`no version in [${table}]`);
}

/** The same TOML with only that table's `version` line changed. */
export function setTomlVersion(toml, table, version) {
  const lines = toml.split('\n');
  const [start, end] = tableBounds(lines, table);
  for (let i = start + 1; i < end; i++) {
    if (VERSION_LINE.test(lines[i])) {
      lines[i] = lines[i].replace(VERSION_LINE, `$1${version}$3`);
      return lines.join('\n');
    }
  }
  throw new Error(`no version in [${table}]`);
}

export const appTag = (version) => `v${version}`;
export const firmwareTag = (product, version) => `fw-${product}-v${version}`;

/**
 * The whole release plan. `tags` lists every existing tag name, injected so the plan stays pure; any
 * problem is an Error whose message is meant for the person who pressed the button.
 */
export function planRelease({
  appCurrent,
  firmwareCurrent,
  product,
  appBump,
  appExact,
  firmwareBump,
  firmwareExact,
  tags,
}) {
  for (const [what, bump] of [
    ['app', appBump],
    ['firmware', firmwareBump],
  ]) {
    if (!BUMPS.includes(bump)) {
      throw new Error(`${what} bump must be none, patch, minor or major, not "${bump}"`);
    }
  }
  const exists = (tag) => tags.includes(tag);

  // An existing tag is named first: it is the clearest reason a version cannot be released.
  const appPrevious = latestTagVersion(tags, 'v');
  const appOverride = (appExact ?? '').trim().replace(/^v/, '');
  const appWanted = appOverride || bumpVersion(appCurrent, appBump);
  if (exists(appTag(appWanted))) {
    throw new Error(
      appBump === 'none' && !appOverride
        ? `app bump none releases the current version ${appCurrent}, but tag ${appTag(appCurrent)} already exists; pick patch, minor or major`
        : `tag ${appTag(appWanted)} already exists; pick another version`,
    );
  }
  const app = nextVersion(appCurrent, { bump: appBump, exact: appExact, latest: appPrevious });

  // Firmware that keeps an already tagged version is simply unchanged; anything else is a firmware
  // release under the same rules as the app, tagged even when the version stays (the first tag).
  const fwPrefix = firmwareTag(product, '');
  const firmwarePrevious = latestTagVersion(tags, fwPrefix);
  const fwOverride = (firmwareExact ?? '').trim().replace(/^v/, '');
  const keeps =
    (fwOverride ? fwOverride === firmwareCurrent : firmwareBump === 'none') &&
    exists(firmwareTag(product, firmwareCurrent));
  if (keeps && compareSemver(firmwareCurrent, firmwarePrevious) < 0) {
    throw new Error(
      `firmware ${firmwareCurrent} is below the latest released version ${firmwarePrevious}`,
    );
  }
  if (!keeps) {
    const wanted = firmwareTag(product, fwOverride || bumpVersion(firmwareCurrent, firmwareBump));
    if (exists(wanted)) throw new Error(`tag ${wanted} already exists; pick another version`);
  }
  const firmware = keeps
    ? firmwareCurrent
    : nextVersion(firmwareCurrent, {
        bump: firmwareBump,
        exact: firmwareExact,
        latest: firmwarePrevious,
      });
  const fwTag = keeps ? '' : firmwareTag(product, firmware);

  const appChanged = app !== appCurrent;
  const firmwareChanged = firmware !== firmwareCurrent;
  return {
    appCurrent,
    appPrevious,
    app,
    appChanged,
    appTag: appTag(app),
    firmwareCurrent,
    firmwarePrevious,
    firmware,
    firmwareChanged,
    firmwareTag: fwTag,
    commitNeeded: appChanged || firmwareChanged,
  };
}

/** One-line commit (or tag) body naming the firmware. */
export function commitBody(plan, productDisplay) {
  if (plan.firmwareChanged) {
    return `${productDisplay} firmware ${plan.firmware} (was ${plan.firmwareCurrent}), tag ${plan.firmwareTag}.`;
  }
  if (plan.firmwareTag) {
    const tagged = plan.firmwarePrevious ? 'tag' : 'first tagged as';
    return `${productDisplay} firmware ${plan.firmware}, ${tagged} ${plan.firmwareTag}.`;
  }
  return `${productDisplay} firmware ${plan.firmware}, unchanged.`;
}

function localTagsAt(root) {
  return execFileSync('git', ['tag', '--list'], { cwd: root, encoding: 'utf8' })
    .split('\n')
    .map((t) => t.trim())
    .filter(Boolean);
}

export function main(argv, directory = root) {
  const root = directory;
  const { values: opt } = parseArgs({
    args: argv,
    options: {
      app: { type: 'string', default: 'minor' },
      'app-version': { type: 'string', default: '' },
      firmware: { type: 'string', default: 'none' },
      'firmware-version': { type: 'string', default: '' },
      product: { type: 'string', default: 'pala' },
      'dry-run': { type: 'boolean', default: false },
      write: { type: 'boolean', default: false },
      outputs: { type: 'string' },
    },
  });
  if (opt.write && opt['dry-run']) throw new Error('--write and --dry-run exclude each other');
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(opt.product)) throw new Error('invalid product id');

  const appToml = join(root, 'Cargo.toml');
  const fwToml = join(root, 'firmware', opt.product, 'Cargo.toml');
  const appText = readFileSync(appToml, 'utf8');
  const fwText = readFileSync(fwToml, 'utf8');
  const display = productName(
    readFileSync(join(root, 'products', opt.product, 'product.toml'), 'utf8'),
  );

  const tags = localTagsAt(root);
  const plan = planRelease({
    appCurrent: tomlVersion(appText, 'workspace.package'),
    firmwareCurrent: tomlVersion(fwText, 'package'),
    product: opt.product,
    appBump: opt.app,
    appExact: opt['app-version'],
    firmwareBump: opt.firmware,
    firmwareExact: opt['firmware-version'],
    tags,
  });
  const guard = checkFirmwareRelease(root, opt.product, plan.firmwarePrevious, plan.firmware, tags);
  console.log(`Firmware guard: ${guard}.`);

  const how = (exact, bump) => (exact.trim() ? 'exact' : bump);
  console.log(
    plan.appPrevious
      ? `Previous release: ${appTag(plan.appPrevious)}`
      : 'Previous release: none (this is the first release)',
  );
  console.log(
    plan.appChanged
      ? `Kivori app:   ${plan.appCurrent} -> ${plan.app} (${how(opt['app-version'], opt.app)}), tag ${plan.appTag}`
      : `Kivori app:   ${plan.app} (the current version, as it is), tag ${plan.appTag}`,
  );
  console.log(
    plan.firmwareChanged
      ? `${display} firmware: ${plan.firmwareCurrent} -> ${plan.firmware} (${how(opt['firmware-version'], opt.firmware)}), tag ${plan.firmwareTag}`
      : plan.firmwareTag
        ? `${display} firmware: ${plan.firmware} (unchanged, first firmware tag ${plan.firmwareTag})`
        : `${display} firmware: ${plan.firmware} (unchanged, no firmware tag)`,
  );
  if (plan.commitNeeded) {
    console.log(`Commit: chore(release): ${plan.appTag}`);
    console.log(`        ${commitBody(plan, display)}`);
  } else {
    console.log('Commit: none, no version changes; the current main commit is tagged as it is');
    console.log(`        ${commitBody(plan, display)}`);
  }

  if (opt.write) {
    const wrote = [];
    if (plan.appChanged) {
      writeFileSync(appToml, setTomlVersion(appText, 'workspace.package', plan.app));
      wrote.push('Cargo.toml');
    }
    if (plan.firmwareChanged) {
      writeFileSync(fwToml, setTomlVersion(fwText, 'package', plan.firmware));
      wrote.push(`firmware/${opt.product}/Cargo.toml`);
    }
    console.log(wrote.length ? `Wrote ${wrote.join(' and ')}.` : 'Nothing to write.');
  } else {
    console.log('Dry run: nothing written.');
  }

  if (opt.outputs) {
    const out = {
      app_version: plan.app,
      app_tag: plan.appTag,
      app_previous: plan.appPrevious,
      app_changed: String(plan.appChanged),
      firmware_version: plan.firmware,
      firmware_previous: plan.firmwareCurrent,
      firmware_changed: String(plan.firmwareChanged),
      firmware_tag: plan.firmwareTag,
      commit_needed: String(plan.commitNeeded),
      commit_body: commitBody(plan, display),
    };
    appendFileSync(
      opt.outputs,
      Object.entries(out)
        .map(([k, v]) => `${k}=${v}\n`)
        .join(''),
    );
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`release-bump: ${e.message}`);
    process.exit(1);
  }
}
