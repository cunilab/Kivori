#!/usr/bin/env node
// Next app and firmware versions for a release (ADR-0014). Used by .github/workflows/release-start.yml
// (the Release button); safe to run locally, where it only prints unless --write is given.
//
//   node scripts/release-bump.mjs --app patch|minor|major [--app-version X.Y.Z] \
//        [--firmware none|patch|minor|major] [--firmware-version A.B.C] [--product pala] \
//        [--dry-run | --write] [--outputs FILE]
//
// The app version is `[workspace.package] version` in Cargo.toml, the firmware version `[package]
// version` in firmware/<product>/Cargo.toml. An exact version overrides the bump and must be greater
// than the current one. Below 1.0.0 a major bump is still a real major (0.4.2 -> 1.0.0). The run fails
// when the resulting tag (`vX.Y.Z`, `fw-<product>-vA.B.C`) already exists. --write edits the two
// Cargo.toml files (the lockfiles are refreshed by cargo in the workflow); --outputs appends
// `key=value` lines for $GITHUB_OUTPUT.
import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { productName } from './release-info.mjs';

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

/**
 * The next version: the exact override when given (an optional leading `v` is accepted), else the
 * bump. An override must be greater than the current version.
 */
export function nextVersion(current, { bump, exact }) {
  const override = (exact ?? '').trim().replace(/^v/, '');
  if (!override) return bumpVersion(current, bump);
  parseSemver(override);
  if (compareSemver(override, current) <= 0) {
    throw new Error(`${override} is not greater than the current version ${current}`);
  }
  return override;
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
 * The whole release plan. `tagExists(name)` is injected so the plan stays pure; any problem is an
 * Error whose message is meant for the person who pressed the button.
 */
export function planRelease({
  appCurrent,
  firmwareCurrent,
  product,
  appBump,
  appExact,
  firmwareBump,
  firmwareExact,
  tagExists,
}) {
  if (!['patch', 'minor', 'major'].includes(appBump)) {
    throw new Error(`app bump must be patch, minor or major, not "${appBump}"`);
  }
  const app = nextVersion(appCurrent, { bump: appBump, exact: appExact });
  const firmware = nextVersion(firmwareCurrent, { bump: firmwareBump, exact: firmwareExact });
  const firmwareChanged = firmware !== firmwareCurrent;
  const tags = [appTag(app)];
  if (firmwareChanged) tags.push(firmwareTag(product, firmware));
  for (const tag of tags) {
    if (tagExists(tag)) throw new Error(`tag ${tag} already exists; pick another version`);
  }
  return {
    appCurrent,
    app,
    appTag: tags[0],
    firmwareCurrent,
    firmware,
    firmwareChanged,
    firmwareTag: firmwareChanged ? tags[1] : '',
  };
}

/** One-line commit body naming the firmware. */
export function commitBody(plan, productDisplay) {
  return plan.firmwareChanged
    ? `${productDisplay} firmware ${plan.firmware} (was ${plan.firmwareCurrent}), tag ${plan.firmwareTag}.`
    : `${productDisplay} firmware ${plan.firmware}, unchanged.`;
}

function localTagExists(name) {
  try {
    execFileSync('git', ['rev-parse', '-q', '--verify', `refs/tags/${name}`], {
      cwd: root,
      stdio: 'ignore',
    });
    return true;
  } catch {
    return false;
  }
}

function main(argv) {
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

  const appToml = join(root, 'Cargo.toml');
  const fwToml = join(root, 'firmware', opt.product, 'Cargo.toml');
  const appText = readFileSync(appToml, 'utf8');
  const fwText = readFileSync(fwToml, 'utf8');
  const display = productName(
    readFileSync(join(root, 'products', opt.product, 'product.toml'), 'utf8'),
  );

  const plan = planRelease({
    appCurrent: tomlVersion(appText, 'workspace.package'),
    firmwareCurrent: tomlVersion(fwText, 'package'),
    product: opt.product,
    appBump: opt.app,
    appExact: opt['app-version'],
    firmwareBump: opt.firmware,
    firmwareExact: opt['firmware-version'],
    tagExists: localTagExists,
  });

  const how = (exact, bump) => (exact.trim() ? 'exact' : bump);
  console.log(
    `Kivori app:   ${plan.appCurrent} -> ${plan.app} (${how(opt['app-version'], opt.app)}), tag ${plan.appTag}`,
  );
  console.log(
    plan.firmwareChanged
      ? `${display} firmware: ${plan.firmwareCurrent} -> ${plan.firmware} (${how(opt['firmware-version'], opt.firmware)}), tag ${plan.firmwareTag}`
      : `${display} firmware: ${plan.firmware} (unchanged, no firmware tag)`,
  );
  console.log(`Commit: chore(release): ${plan.appTag}`);
  console.log(`        ${commitBody(plan, display)}`);

  if (opt.write) {
    writeFileSync(appToml, setTomlVersion(appText, 'workspace.package', plan.app));
    if (plan.firmwareChanged)
      writeFileSync(fwToml, setTomlVersion(fwText, 'package', plan.firmware));
    console.log(
      'Wrote Cargo.toml' + (plan.firmwareChanged ? ` and firmware/${opt.product}/Cargo.toml` : ''),
    );
  } else {
    console.log('Dry run: nothing written.');
  }

  if (opt.outputs) {
    const out = {
      app_version: plan.app,
      app_tag: plan.appTag,
      firmware_version: plan.firmware,
      firmware_previous: plan.firmwareCurrent,
      firmware_changed: String(plan.firmwareChanged),
      firmware_tag: plan.firmwareTag,
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
