#!/usr/bin/env bun
// Next app and firmware versions for a release (ADR-0014). Used by .github/workflows/release-start.yml
// (the Release button); safe to run locally, where it only prints unless --write is given.
//
//   bun scripts/release-bump.mjs --app none|patch|minor|major [--app-version X.Y.Z] \
//        [--firmware none|patch|minor|major] [--firmware-version A.B.C] [--product ID] \
//        [--firmware-overrides JSON_MAP] [--dry-run | --write] [--outputs FILE]
//
// The app version is `[workspace.package] version` in Cargo.toml, the firmware version `[package]
// version` in the catalog-selected firmware crates. A bump starts from that Cargo version; `none` keeps it
// and an exact version overrides the bump. Each new version must be greater than the latest existing
// tag of its component (`vX.Y.Z` for the app, `fw-<product>-vA.B.C` for the firmware) and never below
// its Cargo version; with no tag yet, the Cargo version may be released as it is (the first release).
// So `--app none` releases the current app version, and only while no `v<current>` tag exists.
// `--firmware none` keeps the firmware; while no `fw-<product>-v*` tag exists yet, the release also
// tags `fw-<product>-v<current>` so the firmware lineage starts tagged. Below 1.0.0 a major bump is
// still a real major (0.4.2 -> 1.0.0). The run fails when a resulting tag already exists. When neither
// version changes, `commit_needed=false`: finalize validates the merged version PR without edits.
// Prepare requires a reviewable version change. --write edits changed Cargo.toml versions (lockfiles
// are refreshed by cargo in prepare); --outputs appends `key=value` lines for $GITHUB_OUTPUT.
import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual, parseArgs } from 'node:util';

import { releaseCatalog } from './release-catalog.mjs';
import { checkFirmwareRelease, firmwareTagPrefixes } from './firmware-release-guard.mjs';

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
  const start = lines.findIndex((line) => {
    const header = /^[ \t]*\[([^\]]+)\][ \t]*(?:#[^\r\n]*)?\r?$/.exec(line);
    return header && header[1].replace(/\s/g, '') === table;
  });
  if (start < 0)
    throw new Error(`Cannot safely edit version in [${table}]; use a plain version line`);
  let end = lines.findIndex((l, i) => i > start && /^\s*\[/.test(l));
  if (end < 0) end = lines.length;
  return [start, end];
}

const VERSION_LINE = /^(\s*version\s*=\s*)(["'])([^"']+)(\2[^\r\n]*\r?)$/;

/** The explicit version of a TOML table (`workspace.package`, `package`). Run with Bun. */
export function tomlVersion(toml, table) {
  const value = table.split('.').reduce((value, key) => value?.[key], Bun.TOML.parse(toml));
  if (!value) throw new Error(`no [${table}] table`);
  if (typeof value.version !== 'string') throw new Error(`no version in [${table}]`);
  return value.version;
}

/** Preserve formatting, refusing any edit that changes more than the intended version. */
export function setTomlVersion(toml, table, version) {
  parseSemver(version);
  const expected = Bun.TOML.parse(toml);
  tomlVersion(toml, table);
  table.split('.').reduce((value, key) => value[key], expected).version = version;
  const lines = toml.split('\n');
  const [start, end] = tableBounds(lines, table);
  for (let i = start + 1; i < end; i++) {
    if (VERSION_LINE.test(lines[i])) {
      lines[i] = lines[i].replace(
        VERSION_LINE,
        (_, prefix, quote, _old, suffix) => `${prefix}${quote}${version}${suffix}`,
      );
      const updated = lines.join('\n');
      if (!isDeepStrictEqual(Bun.TOML.parse(updated), expected))
        throw new Error(`Cannot safely edit version in [${table}]`);
      return updated;
    }
  }
  throw new Error(`Cannot safely edit version in [${table}]; use a plain version line`);
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
  // A renamed product's tags under its earlier id are still its released versions.
  const fwPrefixes = firmwareTagPrefixes(product);
  const firmwarePrevious = fwPrefixes
    .map((prefix) => latestTagVersion(tags, prefix))
    .reduce(
      (best, version) => (!best || (version && compareSemver(version, best) > 0) ? version : best),
      '',
    );
  const released = (version) => fwPrefixes.some((prefix) => exists(`${prefix}${version}`));
  const fwOverride = (firmwareExact ?? '').trim().replace(/^v/, '');
  const keeps =
    (fwOverride ? fwOverride === firmwareCurrent : firmwareBump === 'none') &&
    released(firmwareCurrent);
  if (keeps && compareSemver(firmwareCurrent, firmwarePrevious) < 0) {
    throw new Error(
      `firmware ${firmwareCurrent} is below the latest released version ${firmwarePrevious}`,
    );
  }
  if (!keeps) {
    const version = fwOverride || bumpVersion(firmwareCurrent, firmwareBump);
    const taken = fwPrefixes.map((prefix) => `${prefix}${version}`).find(exists);
    if (taken) throw new Error(`tag ${taken} already exists; pick another version`);
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
      'firmware-overrides': { type: 'string' },
      product: { type: 'string' },
      'dry-run': { type: 'boolean', default: false },
      write: { type: 'boolean', default: false },
      outputs: { type: 'string' },
    },
  });
  if (opt.write && opt['dry-run']) throw new Error('--write and --dry-run exclude each other');
  const catalog = releaseCatalog(root);
  if (opt.product && !catalog.some((p) => p.product === opt.product))
    throw new Error(`Unknown active product: ${opt.product}`);
  if (catalog.length !== 1 && !opt.product && (opt.firmware !== 'none' || opt['firmware-version']))
    throw new Error('Select --product or use --firmware-overrides with multiple products');
  const overrides = opt['firmware-overrides'] ? JSON.parse(opt['firmware-overrides']) : {};
  if (!overrides || typeof overrides !== 'object' || Array.isArray(overrides))
    throw new Error('Firmware overrides must be a JSON object');
  if (
    opt['firmware-overrides'] &&
    (opt.product || opt.firmware !== 'none' || opt['firmware-version'])
  )
    throw new Error('--firmware-overrides excludes --product, --firmware and --firmware-version');
  for (const [id, value] of Object.entries(overrides)) {
    if (!catalog.some((p) => p.product === id)) throw new Error(`Unknown active product: ${id}`);
    if (typeof value !== 'string' || (!BUMPS.includes(value) && !SEMVER.test(value)))
      throw new Error(`Invalid firmware override for ${id}: use a bump or X.Y.Z`);
  }

  const appToml = join(root, 'Cargo.toml');
  const appText = readFileSync(appToml, 'utf8');
  const tags = localTagsAt(root);
  const firmware = catalog.map((product) => {
    const selected = !opt.product || opt.product === product.product;
    let bump = selected ? opt.firmware : 'none';
    let exact = selected ? opt['firmware-version'] : '';
    if (Object.hasOwn(overrides, product.product)) {
      const value = overrides[product.product];
      bump = BUMPS.includes(value) ? value : 'none';
      exact = BUMPS.includes(value) ? '' : value;
    }
    const plan = planRelease({
      appCurrent: tomlVersion(appText, 'workspace.package'),
      firmwareCurrent: product.version,
      product: product.product,
      appBump: opt.app,
      appExact: opt['app-version'],
      firmwareBump: bump,
      firmwareExact: exact,
      tags,
    });
    return { ...product, ...plan };
  });
  const plan = firmware[0];
  const crateVersions = new Map();
  for (const product of firmware) {
    for (const crate of product.crates) {
      if (crateVersions.has(crate) && crateVersions.get(crate) !== product.firmware)
        throw new Error(`Conflicting firmware versions for shared crate ${crate}`);
      crateVersions.set(crate, product.firmware);
    }
  }
  const edits = new Map();
  if (plan.appChanged)
    edits.set('Cargo.toml', setTomlVersion(appText, 'workspace.package', plan.app));
  for (const [crate, version] of crateVersions) {
    const path = `${crate}/Cargo.toml`;
    const text = readFileSync(join(root, path), 'utf8');
    if (version !== tomlVersion(text, 'package'))
      edits.set(path, setTomlVersion(text, 'package', version));
  }
  // Every bundled lineage must pass before any version or workflow output is written.
  for (const product of firmware) {
    for (const crate of product.crates) {
      const guard = checkFirmwareRelease(
        root,
        product.product,
        product.firmwarePrevious,
        product.firmware,
        tags,
        crate,
        [...edits.keys()],
      );
      console.log(`${product.product} (${crate}) firmware guard: ${guard}.`);
    }
  }
  const commitNeeded = firmware.some((p) => p.commitNeeded);
  const body = firmware.map((p) => commitBody(p, p.name)).join(' ');
  console.log(`Kivori app: ${plan.appCurrent} -> ${plan.app}, tag ${plan.appTag}`);
  for (const product of firmware)
    console.log(
      `${product.name} firmware: ${product.firmwareCurrent} -> ${product.firmware}` +
        (product.firmwareTag ? `, tag ${product.firmwareTag}` : ', unchanged, no firmware tag'),
    );
  console.log(
    commitNeeded ? `Commit: chore(release): ${plan.appTag}` : 'Commit: none, versions unchanged',
  );
  console.log(body);

  // Lock refresh may only update version bookkeeping, never repair a stale dependency resolution.
  for (const cwd of [root, ...[...crateVersions.keys()].map((crate) => join(root, crate))]) {
    execFileSync('cargo', ['metadata', '--locked', '--all-features', '--format-version', '1'], {
      cwd,
      stdio: ['ignore', 'ignore', 'pipe'],
    });
  }
  if (opt.write) {
    for (const [path, text] of edits) writeFileSync(join(root, path), text);
    console.log(edits.size ? `Wrote ${[...edits.keys()].join(', ')}.` : 'Nothing to write.');
  } else console.log('Dry run: nothing written.');

  if (opt.outputs) {
    const out = {
      app_version: plan.app,
      app_tag: plan.appTag,
      app_previous: plan.appPrevious,
      app_changed: String(plan.appChanged),
      firmware: JSON.stringify(
        firmware.map((p) => ({
          product: p.product,
          name: p.name,
          version: p.firmware,
          tag: p.firmwareTag,
        })),
      ),
      firmware_versions: JSON.stringify(
        Object.fromEntries(firmware.map((p) => [p.product, p.firmware])),
      ),
      firmware_crates: JSON.stringify([...crateVersions.keys()]),
      release_files: JSON.stringify([
        'Cargo.toml',
        'Cargo.lock',
        ...[...crateVersions.keys()].flatMap((c) => [`${c}/Cargo.toml`, `${c}/Cargo.lock`]),
      ]),
      commit_needed: String(commitNeeded),
      commit_body: body,
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
