import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { TOLERANCE, compare, factsFromBudget } from './virtual-resource-facts.mjs';

const budget = {
  measured: {
    appImageBytes: 343686,
    assetBlobBytes: 127253,
    staticRamBytes: 158332,
    stackRegionBytes: 162180,
    blobCapBytes: 131072,
  },
  stack: { directBytes: 115744, peakBytes: 117456 },
};

const elf = {
  app_image_bytes: 343686,
  asset_blob_bytes: 127253,
  static_ram_bytes: 158332,
  stack_region_bytes: 162180,
  stack_peak_bytes: 115744,
};

test('factsFromBudget maps the measured fields and the direct stack depth', () => {
  assert.deepEqual(factsFromBudget(budget), elf);
});

test('compare accepts an exact match and a difference within the tolerance', () => {
  assert.equal(TOLERANCE, 0.02);
  assert.deepEqual(compare(elf, budget, TOLERANCE), []);
  const near = { ...elf, app_image_bytes: Math.round(343686 * 1.019) };
  assert.deepEqual(compare(near, budget, TOLERANCE), []);
});

test('compare reports a field over the tolerance with fixture, measured and percent', () => {
  const stale = { ...elf, stack_peak_bytes: Math.round(115744 * 1.1) };
  const out = compare(stale, budget, TOLERANCE);
  assert.equal(out.length, 1);
  assert.equal(out[0].field, 'stack_peak_bytes');
  assert.equal(out[0].fixture, stale.stack_peak_bytes);
  assert.equal(out[0].measured, 115744);
  assert.ok(Math.abs(out[0].percent - 10) < 0.1);
});

test('compare treats a missing fixture field as a mismatch', () => {
  const partial = { ...elf };
  delete partial.asset_blob_bytes;
  const out = compare(partial, budget, TOLERANCE);
  assert.equal(out.length, 1);
  assert.equal(out[0].field, 'asset_blob_bytes');
  assert.equal(out[0].percent, null);
  assert.equal(compare(undefined, budget, TOLERANCE).length, 5);
});
