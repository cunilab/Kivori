import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as pause } from 'node:timers/promises';
import test from 'node:test';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../../..');
const posixTest = process.platform === 'win32' ? test.skip : test;
function put(path, text, mode) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text, mode ? { mode } : undefined);
}
function shim(path, code) {
  put(path, `#!/usr/bin/env node\n${code}\n`, 0o755);
}

test('the actual production frontend guard rejects every virtual marker and developer page', () => {
  const root = mkdtempSync(join(tmpdir(), 'kivori-frontend-boundary-'));
  try {
    const assets = join(root, 'apps/desktop/dist/assets');
    mkdirSync(assets, { recursive: true });
    const artifact = join(assets, 'app.js');
    const check = () =>
      spawnSync(process.execPath, [join(repo, 'scripts/check-mock-excluded.mjs')], {
        cwd: root,
        encoding: 'utf8',
        timeout: 5000,
      });
    put(artifact, 'export const value = 1;');
    assert.equal(check().status, 0);
    for (const marker of [
      'kivori-ipc-browser-mock-must-not-ship',
      'kivori-virtual-device-must-not-ship',
      'virtual_snapshot',
      'virtual_input',
      'virtual_connection',
      'KIVORI-VIRTUAL-DEVICE',
      'fnv1a64-rgb565-le',
      'PASS S5.4',
      'PASS S5.5',
    ]) {
      put(artifact, JSON.stringify(marker));
      assert.equal(check().status, 1, marker);
    }
    put(artifact, 'export const value = 1;');
    put(join(root, 'apps/desktop/dist/virtual-device.html'), '<html></html>');
    assert.equal(check().status, 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

posixTest(
  'the actual offline guard checks simulator dependencies/source and desktop graph isolation',
  () => {
    const root = mkdtempSync(join(tmpdir(), 'kivori-offline-boundary-'));
    try {
      const bin = join(root, 'bin');
      const policy = join(root, 'policy.json');
      put(join(root, 'firmware/fixture/Cargo.toml'), 'fixture');
      put(join(root, 'firmware/fixture/src/lib.rs'), '');
      const source = join(root, 'firmware/fixture/simulator/src/main.rs');
      put(source, 'fn main() {}');
      put(join(root, 'firmware/fixture/simulator/Cargo.toml'), 'fixture');
      shim(join(bin, 'bun'), 'process.stdout.write("firmware/fixture\\0");');
      shim(join(bin, 'rustc'), 'console.log("host: x86_64-unknown-linux-gnu");');
      shim(
        join(bin, 'cargo'),
        `
      const cfg = JSON.parse(require('node:fs').readFileSync(${JSON.stringify(policy)}));
      const args = process.argv.slice(2);
      const simulator = process.cwd().endsWith('/simulator');
      if (args[0] === 'metadata') {
        const name = simulator ? 'fixture-simulator' : process.cwd().endsWith('/fixture') ? 'fixture-firmware' : 'kivori-desktop';
        console.log(JSON.stringify({packages:[{name,dependencies:simulator && cfg.mode === 'direct' ? [{name:'reqwest'}] : []}]}));
      } else if (args[0] === 'tree') {
        console.log(args.includes('--manifest-path') && cfg.mode === 'transitive' ? 'reqwest v1.0.0' : args.includes('-p') && cfg.mode === 'desktop' ? 'fixture-firmware v1.0.0' : 'serde v1.0.0');
      } else process.exit(90);
    `,
      );
      const check = (mode) => {
        put(policy, JSON.stringify({ mode }));
        return spawnSync('bash', [join(repo, 'scripts/check-offline-deps.sh')], {
          cwd: root,
          env: { ...process.env, PATH: `${bin}:${process.env.PATH}` },
          encoding: 'utf8',
          timeout: 15000,
        });
      };
      assert.equal(check('clean').status, 0);
      for (const mode of ['direct', 'transitive', 'desktop']) {
        const result = check(mode);
        assert.equal(result.status, 1, `${mode}: ${result.stderr}`);
        assert.match(result.stdout, /VIOLATION/);
      }
      put(source, 'use std::net::TcpStream;');
      const result = check('clean');
      assert.equal(result.status, 1, result.stderr);
      assert.match(result.stdout, /network APIs/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);

posixTest(
  'stopping the launcher during its metadata probe prevents later builds and passing reports',
  { timeout: 10000 },
  async () => {
    const root = mkdtempSync(join(tmpdir(), 'kivori-cancel-boundary-'));
    const product = join(root, 'firmware/pala/simulator');
    const ready = join(root, 'probe-ready');
    const called = join(root, 'cargo-called');
    let child;
    try {
      mkdirSync(product, { recursive: true });
      for (const name of ['run.mjs', 'smoke-report.mjs'])
        copyFileSync(join(here, name), join(product, name));
      for (const name of ['smoke-report.test.mjs', 'boundary.test.mjs']) {
        put(
          join(product, name),
          "import test from 'node:test'; test('fixture tooling', () => {});",
        );
      }
      for (const file of [
        'Cargo.lock',
        'firmware/pala/Cargo.lock',
        'firmware/pala/simulator/Cargo.lock',
        'firmware/pala/simulator/fixtures/replay-goldens.json',
      ]) {
        put(join(root, file), '{}');
      }
      const bin = join(root, 'bin');
      shim(join(bin, 'git'), 'if (process.argv[2] === "rev-parse") console.log("1".repeat(40));');
      shim(
        join(bin, 'rustc'),
        `require('node:fs').writeFileSync(${JSON.stringify(ready)}, String(process.pid)); setTimeout(() => console.log('host: x86_64-unknown-linux-gnu'), 1500);`,
      );
      shim(
        join(bin, 'cargo'),
        `require('node:fs').writeFileSync(${JSON.stringify(called)}, 'called'); process.exit(91);`,
      );
      child = spawn(process.execPath, [join(product, 'run.mjs'), '--smoke'], {
        cwd: root,
        env: { ...process.env, PATH: `${bin}:${process.env.PATH}` },
        stdio: ['ignore', 'ignore', 'ignore'],
      });
      const closed = new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('cancelled launcher did not exit')), 7000);
        child.once('error', reject);
        child.once('close', (code) => {
          clearTimeout(timer);
          resolve(code);
        });
      });
      const deadline = Date.now() + 5000;
      while (!existsSync(ready) && Date.now() < deadline) await pause(10);
      assert.ok(existsSync(ready), 'metadata probe was not reached');
      child.kill('SIGTERM');
      assert.notEqual(await closed, 0);
      assert.equal(existsSync(called), false, 'Cargo ran after cancellation');
      const reports = join(product, 'target/reports');
      const value = JSON.parse(readFileSync(join(reports, readdirSync(reports)[0], 'report.json')));
      assert.equal(value.status, 'failed');
      assert.match(value.error, /cancelled/);
    } finally {
      child?.kill('SIGKILL');
      if (existsSync(ready)) {
        try {
          process.kill(Number(readFileSync(ready)), 'SIGKILL');
        } catch {
          /* already exited */
        }
      }
      rmSync(root, { recursive: true, force: true });
    }
  },
);
