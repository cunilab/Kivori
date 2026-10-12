#!/usr/bin/env node
// Frontend offline-asset guard (offline-first rule; docs/architecture.md).
//
// Fails if the desktop frontend references any remote URL (CDN script/style/font/image, or any
// http(s) origin other than the local dev server). All assets must be bundled locally so the app
// works with no internet. Scans apps/desktop/index.html, apps/desktop/src/** and packages/ui/src/** (shared source the desktop bundles).
//
// App updates (ADR-0015) are native only: the webview must never get a network-capable Tauri plugin,
// so no such JS package may be a dependency or an import, and no capability may grant its permissions.

import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, extname } from 'node:path';

const ROOT = 'apps/desktop';
const SCAN_FILES = [join(ROOT, 'index.html')];
const SCAN_DIRS = [join(ROOT, 'src'), join('packages', 'ui', 'src')];
const EXTS = new Set(['.ts', '.tsx', '.js', '.jsx', '.css', '.html']);

// Only the local dev server is an allowed http(s) origin; everything else is a shipped remote ref.
const ALLOWED = /^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?(\/|$)/i;
const REMOTE = /\bhttps?:\/\/[^\s"'`)]+/gi;
// Network-capable Tauri plugins: never in the webview (JS package or import) or a capability.
const NET_PLUGINS = ['updater', 'http', 'websocket', 'upload'];
const NET_IMPORT = new RegExp(`@tauri-apps/plugin-(${NET_PLUGINS.join('|')})\\b`);
const NET_PERMISSION = new RegExp(`"(${NET_PLUGINS.join('|')}):`);
const CAPABILITIES = join(ROOT, 'src-tauri', 'capabilities');

function walk(dir) {
  if (!existsSync(dir)) return [];
  const out = [];
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) out.push(...walk(path));
    else if (EXTS.has(extname(path))) out.push(path);
  }
  return out;
}

const files = [...SCAN_FILES.filter(existsSync), ...SCAN_DIRS.flatMap(walk)];
const violations = [];

for (const file of files) {
  readFileSync(file, 'utf8')
    .split('\n')
    .forEach((line, index) => {
      const matches = line.match(REMOTE);
      if (!matches) return;
      for (const url of matches) {
        if (!ALLOWED.test(url)) violations.push(`${file}:${index + 1}: remote URL ${url}`);
      }
    });
}

for (const file of [...files, join(ROOT, 'package.json')].filter(existsSync)) {
  readFileSync(file, 'utf8')
    .split('\n')
    .forEach((line, index) => {
      if (NET_IMPORT.test(line)) violations.push(`${file}:${index + 1}: network plugin package`);
    });
}
for (const name of existsSync(CAPABILITIES) ? readdirSync(CAPABILITIES) : []) {
  const file = join(CAPABILITIES, name);
  if (NET_PERMISSION.test(readFileSync(file, 'utf8'))) {
    violations.push(`${file}: grants a network plugin permission to the webview`);
  }
}

if (violations.length > 0) {
  console.error('FRONTEND OFFLINE VIOLATION: remote references or webview network access found:');
  for (const v of violations) console.error(`  ${v}`);
  console.error('All frontend assets must be bundled locally (FR-029, SC-006).');
  process.exit(1);
}

console.log(`Frontend offline check OK: no remote URLs in ${files.length} scanned files.`);
