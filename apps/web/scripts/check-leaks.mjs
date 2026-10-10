// Fails when a client-facing build artifact contains something that must not be public: part names,
// exact dimensions, placeholders or any link to the source host. Run after `next build`.
// Scans the prerendered HTML/RSC payloads, static client JS, public/ and the product copy in content/. The server Worker bundle is
// not scanned: lib/releases.ts legitimately holds the release API URL there.
import { readdir, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const ROOT = fileURLToPath(new URL('..', import.meta.url));

const PATTERNS = [
  /github\.com/i,
  /api\.github\.com/i,
  /cunilab\/Kivori/i,
  /ESP32/i,
  /ST7789/i,
  /HW-040/i,
  /SuperMini/i,
  /blueprint/i,
  /\b\d+(\.\d+)?\s?mm\b/i,
  /\bTBC\b/,
];

const SCANS = [
  { dir: '.next/server/app', ext: ['.html', '.rsc', '.meta'] },
  { dir: '.next/static', ext: ['.js'] },
  { dir: 'public', ext: null },
  // Most pages render per request, so the product copy (rendered verbatim) is checked at its source too.
  { dir: 'content', ext: ['.ts', '.md'], skip: /\.test\.ts$/ },
];

const TEXT_EXT = new Set([
  '.html',
  '.rsc',
  '.meta',
  '.js',
  '.json',
  '.txt',
  '.xml',
  '.svg',
  '.css',
]);

async function* walk(dir) {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* walk(path);
    else yield path;
  }
}

const extOf = (path) => path.slice(path.lastIndexOf('.')).toLowerCase();

const hits = [];
let scanned = 0;
// Next's bundled core-js polyfill chunk carries its own licence URL in a data string. It is third-party
// boilerplate, not site content, so exactly that URL is ignored.
const THIRD_PARTY = /https:\/\/github\.com\/zloirock\/core-js[^"'\s]*/g;

for (const { dir, ext, skip } of SCANS) {
  for await (const file of walk(join(ROOT, dir))) {
    if (skip?.test(file)) continue;
    const e = extOf(file);
    if (ext ? !ext.includes(e) : !TEXT_EXT.has(e)) continue;
    const text = (await readFile(file, 'utf8')).replace(THIRD_PARTY, '');
    scanned += 1;
    for (const pattern of PATTERNS) {
      const match = pattern.exec(text);
      if (match) hits.push(`${file.slice(ROOT.length)}: ${match[0]} (${pattern})`);
    }
  }
}

if (scanned === 0) {
  console.error('[check-leaks] nothing to scan; run `next build` first');
  process.exit(1);
}
if (hits.length > 0) {
  console.error(`[check-leaks] ${hits.length} leak(s) found:`);
  for (const hit of hits) console.error(`  ${hit}`);
  process.exit(1);
}
console.log(`[check-leaks] ok: ${scanned} files scanned, no leaks`);
