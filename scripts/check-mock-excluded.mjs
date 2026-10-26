#!/usr/bin/env node
// Production mock-exclusion guard (T124; FR-028).
//
// Asserts the built production frontend bundle does NOT contain the dev-only browser IPC mock. Run
// AFTER `bun run build` (which emits apps/desktop/dist). The mock module exports a unique sentinel; if it
// survived tree-shaking into a shipped chunk, the build leaked dev-only behaviour.

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const SENTINELS = [
  'kivori-ipc-browser-mock-must-not-ship',
  'kivori-virtual-device-must-not-ship',
  'virtual_snapshot',
  'virtual_input',
  'virtual_connection',
];
const DIST = 'apps/desktop/dist/assets';

if (!existsSync(DIST)) {
  console.error(`error: ${DIST} not found — run "bun run build" (production) first.`);
  process.exit(2);
}

const offenders = readdirSync(DIST)
  .filter((file) => file.endsWith('.js'))
  .filter((file) =>
    SENTINELS.some((sentinel) => readFileSync(join(DIST, file), 'utf8').includes(sentinel)),
  );

if (existsSync('apps/desktop/dist/virtual-device.html')) offenders.push('virtual-device.html');

if (offenders.length > 0) {
  console.error(
    `MOCK LEAK: the production bundle includes the browser IPC mock: ${offenders.join(', ')}`,
  );
  console.error('The dev-only mock must be tree-shaken out of production (T124, FR-028).');
  process.exit(1);
}

console.log('Mock-exclusion check OK: the production bundle excludes the browser IPC mock.');
