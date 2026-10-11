#!/usr/bin/env node
// Release title and notes for an app release (ADR-0014): the app's changelog section for the version
// plus the firmware it bundles. Used by .github/workflows/release.yml.
//
//   node scripts/release-info.mjs --version 0.2.0 [--prev-tag v0.1.0] [--product desk-buddy] \
//        [--out-dir DIR]    writes DIR/title.txt and DIR/notes.md (default: print both)
//
// The firmware version comes from firmware/<product>/Cargo.toml and the display name from
// products/<product>/product.toml; nothing is hardcoded. The firmware changelog section is added only
// when the firmware version differs from the one at --prev-tag.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/** The body of the `## [version]` section of a release-please changelog, or '' when absent. */
export function changelogSection(text, version) {
  const lines = text.split(/\r?\n/);
  const heading = new RegExp(`^##\\s+\\[?v?${version.replace(/\./g, '\\.')}\\]?(\\s|\\(|$)`);
  const start = lines.findIndex((l) => heading.test(l));
  if (start < 0) return '';
  let end = lines.findIndex((l, i) => i > start && /^##\s/.test(l) && !/^###/.test(l));
  if (end < 0) end = lines.length;
  return lines
    .slice(start + 1, end)
    .join('\n')
    .trim();
}

/** `version = "x"` of the `[package]` table. */
export function packageVersion(cargoToml) {
  const m = /\[package\][^[]*?^version\s*=\s*"([^"]+)"/ms.exec(cargoToml);
  if (!m) throw new Error('no [package] version found');
  return m[1];
}

/** `display_name` without the brand prefix ("Kivori Desk Buddy" -> "Desk Buddy"). */
export function productName(productToml) {
  const m = /^display_name\s*=\s*"([^"]+)"/m.exec(productToml);
  if (!m) throw new Error('no display_name found');
  return m[1].replace(/^Kivori\s+/, '');
}

export function releaseTitle(appVersion, productDisplay, firmwareVersion) {
  return `Kivori ${appVersion} · ${productDisplay} firmware ${firmwareVersion}`;
}

export function releaseNotes({ appSection, productDisplay, firmwareVersion, firmwareSection }) {
  const parts = [];
  parts.push(appSection || '_No user-facing changes in the app._');
  parts.push(
    `### Firmware\n\n${productDisplay} firmware ${firmwareVersion} is bundled with this release.`,
  );
  if (firmwareSection) {
    parts[parts.length - 1] += `\n\n#### What changed in the firmware\n\n${firmwareSection}`;
  }
  return parts.join('\n\n') + '\n';
}

function git(...args) {
  return execFileSync('git', args, {
    cwd: root,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  });
}

function readOr(path, fallback = '') {
  try {
    return readFileSync(path, 'utf8');
  } catch {
    return fallback;
  }
}

function main(argv) {
  const opt = {};
  for (let i = 0; i < argv.length; i += 2) opt[argv[i].replace(/^--/, '')] = argv[i + 1];
  if (!opt.version) throw new Error('--version is required');
  const product = opt.product ?? 'desk-buddy';
  const fwDir = join(root, 'firmware', product);
  const firmwareVersion = packageVersion(readFileSync(join(fwDir, 'Cargo.toml'), 'utf8'));
  const productDisplay = productName(
    readFileSync(join(root, 'products', product, 'product.toml'), 'utf8'),
  );

  let firmwareChanged = false;
  if (opt['prev-tag']) {
    try {
      const prev = packageVersion(git('show', `${opt['prev-tag']}:firmware/${product}/Cargo.toml`));
      firmwareChanged = prev !== firmwareVersion;
    } catch {
      firmwareChanged = true; // no previous release (or it predates the firmware folder)
    }
  }

  const title = releaseTitle(opt.version, productDisplay, firmwareVersion);
  const notes = releaseNotes({
    appSection: changelogSection(readOr(join(root, 'CHANGELOG.md')), opt.version),
    productDisplay,
    firmwareVersion,
    firmwareSection: firmwareChanged
      ? changelogSection(readOr(join(fwDir, 'CHANGELOG.md')), firmwareVersion)
      : '',
  });

  if (opt['out-dir']) {
    mkdirSync(opt['out-dir'], { recursive: true });
    writeFileSync(join(opt['out-dir'], 'title.txt'), title);
    writeFileSync(join(opt['out-dir'], 'notes.md'), notes);
  } else {
    console.log(title + '\n\n' + notes);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
