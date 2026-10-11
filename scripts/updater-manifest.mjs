#!/usr/bin/env node
// Writes latest.json, the Tauri 2 updater manifest (ADR-0015), for the Windows NSIS installer.
//
//   node scripts/updater-manifest.mjs --version 0.2.0 --installer path/to/Kivori_0.2.0_x64-setup.exe \
//        --notes notes.md --out latest.json [--repo cunilab/Kivori] [--pub-date 2026-10-12T00:00:00Z]
//
// The signature is the contents of `<installer>.sig`. The URL is the stable release asset URL
// https://github.com/<repo>/releases/download/v<version>/<installer file name>.
import { readFileSync, writeFileSync } from 'node:fs';
import { basename } from 'node:path';
import { fileURLToPath } from 'node:url';

export function buildManifest({ version, notes, pubDate, signature, installerName, repo }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`bad version: ${version}`);
  if (!signature.trim()) throw new Error('empty signature');
  return {
    version,
    notes,
    pub_date: new Date(pubDate).toISOString(),
    platforms: {
      'windows-x86_64': {
        signature: signature.trim(),
        url: `https://github.com/${repo}/releases/download/v${version}/${encodeURIComponent(installerName)}`,
      },
    },
  };
}

function main(argv) {
  const opt = {};
  for (let i = 0; i < argv.length; i += 2) opt[argv[i].replace(/^--/, '')] = argv[i + 1];
  for (const k of ['version', 'installer', 'out'])
    if (!opt[k]) throw new Error(`--${k} is required`);
  const manifest = buildManifest({
    version: opt.version,
    notes: opt.notes ? readFileSync(opt.notes, 'utf8').trim() : '',
    pubDate: opt['pub-date'] ?? new Date().toISOString(),
    signature: readFileSync(`${opt.installer}.sig`, 'utf8'),
    installerName: basename(opt.installer),
    repo: opt.repo ?? 'cunilab/Kivori',
  });
  writeFileSync(opt.out, JSON.stringify(manifest, null, 2) + '\n');
  console.log(`Wrote ${opt.out} for ${manifest.version}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
