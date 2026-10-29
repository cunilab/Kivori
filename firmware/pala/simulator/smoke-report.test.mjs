import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import {
  addReplay,
  createReport,
  REPLAY_CASE_IDS,
  runRecorded,
  validateReplay,
} from './smoke-report.mjs';

function replay() {
  return {
    schemaVersion: 1,
    product: 'pala',
    firmwareVersion: '0.1.0',
    goldenFixture: 'firmware/pala/simulator/fixtures/replay-goldens.json',
    exercisedLayers: ['firmware Runtime'],
    simulatedLayers: ['raw input'],
    notExercised: ['desktop deadlines'],
    seed: 17,
    clockMode: 'injected-firmware',
    status: 'passed',
    cases: REPLAY_CASE_IDS.map((id) => ({
      id,
      status: 'passed',
      inputs: { ticks: [{ nowMs: 100, edges: [] }] },
      expected: { actions: [], frameHash: '0123456789abcdef', reboot: false },
      actual: { actions: [], frameHash: '0123456789abcdef', reboot: false },
    })),
  };
}

test('replay evidence rejects wrong seeds, duplicate cases and trace/hash mismatches', () => {
  assert.equal(validateReplay(replay(), 17).seed, 17);
  assert.throws(() => validateReplay(replay(), 29));
  for (const mutate of [
    (r) => (r.status = 'failed'),
    (r) => (r.clockMode = 'real-elapsed'),
    (r) => r.cases.pop(),
    (r) => (r.cases[1].id = r.cases[0].id),
    (r) => (r.cases[0].actual.frameHash = '0000000000000000'),
    (r) => r.cases[0].actual.actions.push({ control: 'button', kind: 'press', deviceMs: 99 }),
    (r) => delete r.cases[0].inputs,
    (r) => delete r.cases[0].expected.frameHash,
  ]) {
    const value = replay();
    mutate(value);
    assert.throws(() => validateReplay(value, 17));
  }
});

test('two successful reports must still be byte-identical', () => {
  const root = mkdtempSync(join(tmpdir(), 'kivori-replay-report-'));
  try {
    const report = createReport(root, {});
    const a = { stdout: 'first.log' };
    const b = { stdout: 'second.log' };
    const value = JSON.stringify(replay());
    writeFileSync(join(report.directory, a.stdout), value);
    writeFileSync(join(report.directory, b.stdout), `${value}\n`);
    assert.throws(() => addReplay(report, 17, a, b), /not repeatable/);
    writeFileSync(join(report.directory, b.stdout), value);
    addReplay(report, 17, a, b);
    assert.equal(report.data.replays[0].byteIdentical, true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('failed child retains logs and cannot be reported as a successful stage', async () => {
  const root = mkdtempSync(join(tmpdir(), 'kivori-child-report-'));
  try {
    const report = createReport(root, { expectedStages: ['scenario'] });
    await assert.rejects(
      runRecorded(
        report,
        { id: 'scenario' },
        process.execPath,
        ['-e', 'console.log("partial evidence"); process.exit(7)'],
        {},
      ),
      /exited 7/,
    );
    assert.equal(report.data.stages[0].exitCode, 7);
    assert.match(readFileSync(join(report.directory, 'scenario.stdout.log'), 'utf8'), /partial/);
    report.finish();
    assert.equal(JSON.parse(readFileSync(report.path)).status, 'failed');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('zero exit without required scenario assertions fails; absent stages and seeds fail', async () => {
  const root = mkdtempSync(join(tmpdir(), 'kivori-missing-report-'));
  try {
    const report = createReport(root, { expectedStages: ['scenario'], expectedSeeds: [17] });
    await assert.rejects(
      runRecorded(
        report,
        { id: 'scenario', expectedMarkers: { prefix: 'PASS scenario', count: 1 } },
        process.execPath,
        ['-e', ''],
        {},
      ),
      /expected 1 scenario markers/,
    );
    report.finish();
    assert.equal(report.data.status, 'failed');
    assert.match(report.data.error, /seeds=17/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test(
  'spawn failure terminates its logs and produces a failed stage',
  { timeout: 5000 },
  async () => {
    const root = mkdtempSync(join(tmpdir(), 'kivori-spawn-report-'));
    try {
      const report = createReport(root, {});
      await assert.rejects(
        runRecorded(
          report,
          { id: 'missing-command' },
          'kivori-simulator-command-not-found',
          [],
          {},
        ),
      );
      assert.equal(report.data.stages[0].status, 'failed');
      report.finish();
      assert.equal(report.data.status, 'failed');
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);

test('a complete successful child stage produces a passed report', async () => {
  const root = mkdtempSync(join(tmpdir(), 'kivori-pass-report-'));
  try {
    const report = createReport(root, { expectedStages: ['scenario'] });
    await runRecorded(
      report,
      { id: 'scenario', expectedMarkers: { prefix: 'PASS scenario', count: 1 } },
      process.execPath,
      ['-e', 'console.log("PASS scenario")'],
      {},
    );
    assert.equal(report.finish(), undefined);
    assert.equal(JSON.parse(readFileSync(report.path)).status, 'passed');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('report stage names cannot escape their artifact directory', async () => {
  const root = mkdtempSync(join(tmpdir(), 'kivori-path-report-'));
  try {
    const report = createReport(root, {});
    await assert.rejects(runRecorded(report, { id: '../escape' }, process.execPath, [], {}));
    assert.equal(report.data.stages.length, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
