#!/usr/bin/env node
// Writes latest.json, the Tauri 2 updater manifest (ADR-0015), for Windows NSIS and Linux AppImage installers.
//
//   node scripts/updater-manifest.mjs --version 0.2.0 --installer path/to/Kivori_0.2.0_x64-setup.exe \
//        --notes notes.md --out latest.json [--repo cunilab/Kivori] [--pub-date 2026-10-12T00:00:00Z]
//
// Linux: add --platform linux-x86_64 and use the .AppImage installer (never .deb).
// Combine CI platform artifacts: --base windows/latest.json --merge linux/latest.json --out latest.json.
// The signature is the contents of `<installer>.sig`. The URL is the stable release asset URL
// https://github.com/<repo>/releases/download/v<version>/<installer file name>.
import { readFileSync, writeFileSync } from 'node:fs';
import { basename } from 'node:path';
import { fileURLToPath } from 'node:url';

export function buildManifest({
  version,
  notes,
  pubDate,
  signature,
  installerName,
  repo,
  platform = 'windows-x86_64',
}) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`bad version: ${version}`);
  if (!['windows-x86_64', 'linux-x86_64'].includes(platform))
    throw new Error(`unsupported platform: ${platform}`);
  if (platform === 'linux-x86_64' && !installerName.endsWith('.AppImage'))
    throw new Error('Linux updater requires an AppImage');
  if (!signature.trim()) throw new Error('empty signature');
  return {
    version,
    notes,
    pub_date: new Date(pubDate).toISOString(),
    platforms: {
      [platform]: {
        signature: signature.trim(),
        url: `https://github.com/${repo}/releases/download/v${version}/${encodeURIComponent(installerName)}`,
      },
    },
  };
}

export function mergeManifests(base, extra) {
  if (base.version !== extra.version) throw new Error('updater version mismatch');
  for (const [platform, artifact] of Object.entries(extra.platforms)) {
    if (!['windows-x86_64', 'linux-x86_64'].includes(platform))
      throw new Error(`unsupported platform: ${platform}`);
    if (base.platforms[platform]) throw new Error(`duplicate platform: ${platform}`);
    if (!artifact.signature?.trim() || !artifact.url?.startsWith('https://github.com/'))
      throw new Error(`invalid artifact: ${platform}`);
  }
  return { ...base, platforms: { ...base.platforms, ...extra.platforms } };
}

function main(argv) {
  const opt = {};
  for (let i = 0; i < argv.length; i += 2) opt[argv[i].replace(/^--/, '')] = argv[i + 1];
  if (opt.base || opt.merge) {
    if (!opt.base || !opt.merge || !opt.out)
      throw new Error('--base, --merge and --out are required');
    const merged = mergeManifests(
      JSON.parse(readFileSync(opt.base, 'utf8')),
      JSON.parse(readFileSync(opt.merge, 'utf8')),
    );
    writeFileSync(opt.out, JSON.stringify(merged, null, 2) + '\n');
    return;
  }
  for (const k of ['version', 'installer', 'out'])
    if (!opt[k]) throw new Error(`--${k} is required`);
  const manifest = buildManifest({
    version: opt.version,
    notes: opt.notes ? readFileSync(opt.notes, 'utf8').trim() : '',
    pubDate: opt['pub-date'] ?? new Date().toISOString(),
    signature: readFileSync(`${opt.installer}.sig`, 'utf8'),
    installerName: basename(opt.installer),
    repo: opt.repo ?? 'cunilab/Kivori',
    platform: opt.platform ?? 'windows-x86_64',
  });
  writeFileSync(opt.out, JSON.stringify(manifest, null, 2) + '\n');
  console.log(`Wrote ${opt.out} for ${manifest.version}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
