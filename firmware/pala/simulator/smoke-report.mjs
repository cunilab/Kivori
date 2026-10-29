import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  createWriteStream,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { join, relative } from 'node:path';
import { finished } from 'node:stream/promises';
import { isDeepStrictEqual } from 'node:util';

export const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
export const REPLAY_SEEDS = [17, 29];
export const REPLAY_CASE_IDS = [
  'mixed-rotary',
  'invalid-bounce-then-detent',
  'buffered-switches',
  'recovery-cancel',
];

export function validateReplay(value, seed) {
  if (
    value?.schemaVersion !== 1 ||
    value.seed !== seed ||
    !REPLAY_SEEDS.includes(seed) ||
    value.product !== 'pala' ||
    typeof value.firmwareVersion !== 'string' ||
    value.goldenFixture !== 'firmware/pala/simulator/fixtures/replay-goldens.json' ||
    value.clockMode !== 'injected-firmware' ||
    value.status !== 'passed' ||
    !Array.isArray(value.cases) ||
    value.cases.length !== 4 ||
    new Set(value.cases.map((item) => item.id)).size !== 4 ||
    !REPLAY_CASE_IDS.every((id) => value.cases.some((item) => item.id === id)) ||
    !['exercisedLayers', 'simulatedLayers', 'notExercised'].every(
      (key) =>
        Array.isArray(value[key]) &&
        value[key].length &&
        value[key].every((s) => typeof s === 'string'),
    )
  ) {
    throw new Error(`Invalid firmware replay report for seed ${seed}`);
  }
  for (const item of value.cases) {
    if (
      typeof item.id !== 'string' ||
      !item.id ||
      item.status !== 'passed' ||
      !Array.isArray(item.inputs?.ticks) ||
      !item.inputs.ticks.length ||
      !Array.isArray(item.expected?.actions) ||
      !/^[0-9a-f]{16}$/.test(item.expected?.frameHash) ||
      typeof item.expected?.reboot !== 'boolean' ||
      !isDeepStrictEqual(item.expected, item.actual)
    ) {
      throw new Error(`Replay mismatch or missing evidence: seed ${seed}, case ${item.id}`);
    }
  }
  return value;
}

export function createReport(root, metadata) {
  mkdirSync(root, { recursive: true });
  const directory = mkdtempSync(join(root, 'run-'));
  const path = join(directory, 'report.json');
  const data = {
    schemaVersion: 1,
    scope: 'virtual-device-smoke',
    status: 'running',
    ...metadata,
    stages: [],
    replays: [],
  };
  const write = () => {
    writeFileSync(`${path}.tmp`, `${JSON.stringify(data, null, 2)}\n`);
    renameSync(`${path}.tmp`, path);
  };
  const finish = (error) => {
    if (!error) {
      const absent = (data.expectedStages ?? []).filter(
        (id) => !data.stages.some((stage) => stage.id === id && stage.status === 'passed'),
      );
      const missingSeeds = (data.expectedSeeds ?? []).filter(
        (seed) => !data.replays.some((replay) => replay.seed === seed && replay.byteIdentical),
      );
      if (
        absent.length ||
        missingSeeds.length ||
        !data.stages.length ||
        data.stages.some((s) => s.status !== 'passed')
      ) {
        error = new Error(`Incomplete smoke report: stages=${absent}, seeds=${missingSeeds}`);
      }
    }
    data.status = error ? 'failed' : 'passed';
    if (error) data.error = String(error.message ?? error);
    write();
    return error;
  };
  write();
  return { directory, path, data, write, finish };
}

// File-backed logs retain partial evidence without collecting compiler output in memory.
export async function runRecorded(report, spec, command, args, options) {
  if (!/^[a-z][a-z0-9-]*$/.test(spec.id) || report.data.stages.some((s) => s.id === spec.id)) {
    throw new Error(`Invalid or duplicate report stage: ${spec.id}`);
  }
  const stdout = `${spec.id}.stdout.log`;
  const stderr = `${spec.id}.stderr.log`;
  const stage = { ...spec, command, args, stdout, stderr, status: 'running' };
  report.data.stages.push(stage);
  report.write();
  const output = createWriteStream(join(report.directory, stdout));
  const errors = createWriteStream(join(report.directory, stderr));
  const { onChild, ...spawnOptions } = options;
  let failure;
  let child;
  const logsDone = Promise.all([finished(output), finished(errors)]).catch((error) => {
    failure ??= error;
    child?.kill();
  });
  try {
    child = spawn(command, args, { ...spawnOptions, stdio: ['ignore', 'pipe', 'pipe'] });
    onChild?.(child);
    child.stdout.pipe(output);
    child.stderr.pipe(errors);
    if (spec.format !== 'replay-json') child.stdout.pipe(process.stdout, { end: false });
    child.stderr.pipe(process.stderr, { end: false });
    await new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('close', (code, signal) => {
        stage.exitCode = code;
        stage.signal = signal;
        if (code === 0) resolve();
        else reject(new Error(`${spec.id} exited ${code ?? signal}`));
      });
    });
  } catch (error) {
    failure ??= error;
    child?.kill();
    if (!child?.pid) {
      output.end();
      errors.end();
    }
  }
  await logsDone;
  if (!failure && spec.expectedMarkers) {
    const path = join(report.directory, stdout);
    if (statSync(path).size > 5 * 1024 * 1024) {
      failure = new Error(`${spec.id}: scenario output exceeds report limit`);
    } else {
      stage.assertions = readFileSync(path, 'utf8')
        .split(/\r?\n/)
        .filter((line) => line.startsWith(spec.expectedMarkers.prefix));
      if (stage.assertions.length !== spec.expectedMarkers.count) {
        failure = new Error(`${spec.id}: expected ${spec.expectedMarkers.count} scenario markers`);
      }
    }
  }
  stage.status = failure ? 'failed' : 'passed';
  if (failure) stage.error = failure.message;
  report.write();
  if (failure) throw failure;
  return stage;
}

export function addReplay(report, seed, first, second) {
  if (report.data.replays.some((item) => item.seed === seed)) {
    throw new Error(`Duplicate firmware replay seed ${seed}`);
  }
  const a = readFileSync(join(report.directory, first.stdout));
  const b = readFileSync(join(report.directory, second.stdout));
  const replay = validateReplay(JSON.parse(a), seed);
  validateReplay(JSON.parse(b), seed);
  if (!a.equals(b)) throw new Error(`Firmware replay is not repeatable for seed ${seed}`);
  report.data.replays.push({
    seed,
    byteIdentical: true,
    reportSha256: sha256(a),
    first: relative(report.directory, join(report.directory, first.stdout)),
    second: relative(report.directory, join(report.directory, second.stdout)),
    evidence: replay,
  });
  report.write();
}
