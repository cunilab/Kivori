#!/usr/bin/env bun
// Freshness check for the resource facts the virtual device reports in its Device internals
// (`simulator/fixtures/resource-facts.json` in the catalog firmware crate). The `elf` block of that fixture is a
// copy of what `bun scripts/firmware-budget.mjs --json` measures on the catalog product ELF, so
// it must not drift from the firmware it models.
//
//   bun scripts/virtual-resource-facts.mjs [--check | --write]
//
// --check (default) compares the fixture with a fresh measurement (2% tolerance) and exits 1 when
// they disagree; --write refreshes only the `elf` block. Build the ELF first with
// `bun scripts/build-firmware.mjs` (or `just fw-virtual-facts-update`).
// Exit status 1 when the fixture is stale, 2 when the ELF/budget cannot be analysed.

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveFirmware } from './build-firmware.mjs';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
/** The fixture of the catalog product (KIVORI_PRODUCT/KIVORI_BOARD select one when several exist). */
export function fixturePath() {
  const plan = resolveFirmware(repo, process.env.KIVORI_PRODUCT, process.env.KIVORI_BOARD);
  return join(plan.cwd, 'simulator/fixtures/resource-facts.json');
}
/** Largest relative difference between the fixture and a fresh measurement still accepted. */
export const TOLERANCE = 0.02;

/** The `elf` block of the fixture from `firmware-budget.mjs --json` output. */
export function factsFromBudget(json) {
  return {
    app_image_bytes: json.measured.appImageBytes,
    asset_blob_bytes: json.measured.assetBlobBytes,
    static_ram_bytes: json.measured.staticRamBytes,
    stack_region_bytes: json.measured.stackRegionBytes,
    stack_peak_bytes: json.stack.directBytes,
  };
}

/**
 * Fields of `fixtureElf` that disagree with the measurement by more than `tolerance` (relative to
 * the measured value), as `{ field, fixture, measured, percent }`. A field missing or not a number
 * on either side always counts as a mismatch (`percent` is null).
 */
export function compare(fixtureElf, measuredJson, tolerance = TOLERANCE) {
  const measured = factsFromBudget(measuredJson);
  const mismatches = [];
  for (const [field, m] of Object.entries(measured)) {
    const f = fixtureElf?.[field];
    if (!Number.isFinite(f) || !Number.isFinite(m)) {
      mismatches.push({ field, fixture: f ?? null, measured: m ?? null, percent: null });
      continue;
    }
    const diff = m === 0 ? (f === 0 ? 0 : Infinity) : Math.abs(f - m) / Math.abs(m);
    if (diff > tolerance) mismatches.push({ field, fixture: f, measured: m, percent: diff * 100 });
  }
  return mismatches;
}

function measure() {
  try {
    const out = execFileSync(
      process.execPath,
      [join(repo, 'scripts/firmware-budget.mjs'), '--json'],
      {
        encoding: 'utf8',
        maxBuffer: 1 << 28,
        stdio: ['ignore', 'pipe', 'inherit'],
      },
    );
    return JSON.parse(out);
  } catch (e) {
    // A violated budget still prints the JSON on stdout (exit 1); anything else cannot be analysed.
    if (e.status === 1 && e.stdout) return JSON.parse(e.stdout);
    throw new Error('cannot analyse the firmware ELF (see firmware-budget output above)', {
      cause: e,
    });
  }
}

export function main(argv) {
  let mode = 'check';
  for (const a of argv) {
    if (a === '--check') mode = 'check';
    else if (a === '--write') mode = 'write';
    else throw new Error(`unknown argument: ${a}`);
  }
  const budget = measure();
  const path = fixturePath();
  const fixture = JSON.parse(readFileSync(path, 'utf8'));
  if (mode === 'write') {
    fixture.elf = factsFromBudget(budget);
    writeFileSync(path, `${JSON.stringify(fixture, null, 2)}\n`);
    console.log(`virtual-resource-facts: wrote the elf block of ${path}`);
    return 0;
  }
  const mismatches = compare(fixture.elf, budget, TOLERANCE);
  if (!mismatches.length) {
    console.log(
      `virtual-resource-facts: OK (fixture matches the product ELF within ${TOLERANCE * 100}%)`,
    );
    return 0;
  }
  console.error('virtual-resource-facts: the committed fixture is stale:');
  for (const m of mismatches) {
    const pct = m.percent === null ? 'missing or not a number' : `${m.percent.toFixed(1)}% off`;
    console.error(`  ${m.field}: fixture ${m.fixture}, measured ${m.measured} (${pct})`);
  }
  console.error(
    'Run `bun scripts/virtual-resource-facts.mjs --write` (after `bun scripts/build-firmware.mjs`) and commit the fixture.',
  );
  return 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (e) {
    console.error(`virtual-resource-facts: ${e.message}`);
    process.exitCode = 2;
  }
}
