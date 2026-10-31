import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { firmwareInputs } from './firmware-release-guard.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));

/** `firmware/pip/build-inputs.txt`: what the firmware source id (BuildInfo) hashes. */
const listed = (product) =>
  readFileSync(new URL(`../firmware/${product}/build-inputs.txt`, import.meta.url), 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith('#'));

// The firmware source id must change exactly when the release guard says the firmware changed.
// Both read the same notion of "firmware input"; this fails when they drift apart, so a new
// dependency or build script cannot be left out of the id.
test('the firmware source id hashes exactly the release guard firmware inputs', () => {
  const { paths } = firmwareInputs(root, 'pip');
  assert.deepEqual(listed('pip').sort(), [...paths].sort());
});

test('the input list names no duplicate and no absolute or parent path', () => {
  const paths = listed('pip');
  assert.equal(new Set(paths).size, paths.length);
  for (const path of paths) assert.doesNotMatch(path, /^\/|\.\.|\\/, path);
});
