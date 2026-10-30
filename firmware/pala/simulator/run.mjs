// Product-owned launcher. No flashing, real OS services or shipping app entry is used.
import { execFileSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { createRequire } from 'node:module';
import { mkdirSync, copyFileSync, readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { addReplay, createReport, REPLAY_SEEDS, runRecorded, sha256 } from './smoke-report.mjs';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const mode = process.argv[2];
if (process.argv.length > 3 || (mode !== undefined && mode !== '--smoke'))
  throw new Error('usage: run.mjs [--smoke]');
const env = { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || '2' };
delete env.RUSTFLAGS;
delete env.CARGO_ENCODED_RUSTFLAGS;
// Load only artifacts built in this tool's declared output locations.
delete env.CARGO_TARGET_DIR;
delete env.CARGO_BUILD_TARGET;
delete env.KIVORI_FIRMWARE_PATH;
delete env.KIVORI_FIRMWARE_MANIFEST;
let active;
let ui;
const expectedStages = [
  'report-tests',
  'build-runner',
  'build-desktop',
  'runner-tests',
  'input',
  'io',
  'bridge-tests',
  'vertical-slice',
  'connections',
  'desktop',
  'recovery',
  ...REPLAY_SEEDS.flatMap((seed) => [`replay-${seed}-first`, `replay-${seed}-second`]),
];
const report =
  mode === '--smoke'
    ? createReport(resolve(root, 'firmware/pala/simulator/target/reports'), {
        expectedStages,
        expectedSeeds: REPLAY_SEEDS,
        startedAt: new Date().toISOString(),
        externalChecksRequired: [
          'production release-surface guards',
          'offline guards',
          'independent no_std build',
        ],
        notExercised: [
          'physical hardware',
          'real OS actions',
          'signatures/installers',
          'shipping native setup',
          'whole-desktop virtual time',
        ],
      })
    : undefined;
let smokeError;
let cancellation;
function stop() {
  cancellation ??= new Error('Virtual-device run cancelled');
  active?.kill();
  void ui?.close();
}
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
async function run(command, args, options = {}) {
  if (cancellation) throw cancellation;
  const { stage, ...spawnOptions } = options;
  if (report) {
    return runRecorded(report, stage, command, args, {
      cwd: root,
      env,
      ...spawnOptions,
      onChild: (child) => {
        active = child;
      },
    }).finally(() => {
      active = undefined;
    });
  }
  const child = spawn(command, args, { cwd: root, env, stdio: 'inherit', ...spawnOptions });
  active = child;
  await new Promise((yes, no) => {
    child.once('error', no);
    child.once('exit', (code, signal) =>
      code === 0 ? yes() : no(new Error(`${command} ${args.join(' ')} exited ${code ?? signal}`)),
    );
  });
  active = undefined;
}
try {
  if (report) {
    const git = (args) => execFileSync('git', args, { cwd: root });
    report.data.source = {
      commit: git(['rev-parse', 'HEAD']).toString().trim(),
      dirty: git(['status', '--porcelain']).length > 0,
      diffSha256: sha256(git(['diff', '--binary', 'HEAD'])),
    };
    report.data.build = {
      launcherRuntime: process.versions.bun
        ? `bun ${process.versions.bun}`
        : `node ${process.version}`,
      node: execFileSync('node', ['--version'], { cwd: root, encoding: 'utf8' }).trim(),
      platform: process.platform,
    };
    report.data.inputSha256 = Object.fromEntries(
      [
        'Cargo.lock',
        'firmware/pala/Cargo.lock',
        'firmware/pala/simulator/Cargo.lock',
        'firmware/pala/simulator/run.mjs',
        'firmware/pala/simulator/smoke-report.mjs',
        'firmware/pala/simulator/fixtures/replay-goldens.json',
      ].map((file) => [file, sha256(readFileSync(resolve(root, file)))]),
    );
    report.write();
    console.log(`Virtual smoke report: ${report.path}`);
    await run(
      'node',
      [
        '--test',
        'firmware/pala/simulator/smoke-report.test.mjs',
        'firmware/pala/simulator/boundary.test.mjs',
      ],
      {
        stage: {
          id: 'report-tests',
          clockMode: 'real-elapsed',
          layers: ['report failure handling'],
        },
      },
    );
  }
  const host = await new Promise((yes, no) => {
    if (cancellation) return no(cancellation);
    const child = spawn('rustc', ['-vV'], { cwd: root, env });
    active = child;
    let text = '';
    child.stdout.on('data', (chunk) => {
      text += chunk;
    });
    child.once('error', (error) => {
      active = undefined;
      no(error);
    });
    child.once('close', (code) => {
      active = undefined;
      if (cancellation) return no(cancellation);
      const host = /^host: (.+)$/m.exec(text)?.[1];
      if (code !== 0 || !host) no(new Error('rustc host target unavailable'));
      else {
        if (report) report.data.build.rustc = text.trim();
        yes(host);
      }
    });
  });
  const manifest = 'firmware/pala/simulator/Cargo.toml';
  const target = resolve(root, 'firmware/pala/simulator/target');
  if (report) report.data.build.hostTarget = host;
  await run(
    'cargo',
    ['build', '--locked', '--manifest-path', manifest, '--target', host, '--target-dir', target],
    { stage: { id: 'build-runner', clockMode: 'not-applicable', layers: ['build tooling'] } },
  );
  await run(
    'cargo',
    [
      'build',
      '--locked',
      '-p',
      'kivori-desktop',
      '--features',
      'virtual-device',
      '--example',
      'virtual_device',
    ],
    { stage: { id: 'build-desktop', clockMode: 'not-applicable', layers: ['build tooling'] } },
  );
  const ext = process.platform === 'win32' ? '.exe' : '';
  const runner = resolve(target, host, 'debug', `kivori-pip-simulator${ext}`);
  let app = resolve(root, 'target/debug/examples', `virtual_device${ext}`);
  if (mode === '--smoke') {
    report.data.build.runnerSha256 = sha256(readFileSync(runner));
    report.data.build.desktopSha256 = sha256(readFileSync(app));
    report.write();
    await run(
      'cargo',
      ['test', '--locked', '--manifest-path', manifest, '--target', host, '--target-dir', target],
      {
        stage: {
          id: 'runner-tests',
          clockMode: 'test-specific',
          layers: ['firmware runner tests'],
        },
      },
    );
    await run(runner, ['--input-smoke'], {
      stage: {
        id: 'input',
        clockMode: 'injected-firmware',
        seed: null,
        layers: ['firmware Runtime'],
        simulatedLayers: ['input', 'clock', 'transport', 'display capture'],
        expectedMarkers: { prefix: 'PASS S5.3 ', count: 32 },
      },
    });
    await run(runner, ['--io-smoke'], {
      stage: {
        id: 'io',
        clockMode: 'injected-firmware',
        layers: ['buffered firmware Runtime', 'production transmit queue'],
        simulatedLayers: ['64-byte FIFO', 'SPI wire time', 'scheduled input', 'display capture'],
        expectedMarkers: { prefix: 'PASS IO ', count: 4 },
      },
    });
    await run(
      'cargo',
      [
        'test',
        '--locked',
        '-p',
        'kivori-desktop',
        '--features',
        'virtual-device',
        '--lib',
        'virtual_device::tests',
      ],
      {
        stage: {
          id: 'bridge-tests',
          clockMode: 'test-specific',
          layers: ['desktop child-process bridge'],
        },
      },
    );
    const frame = resolve(target, 'smoke-frame.ppm');
    for (const [id, args, prefix, count, layers] of [
      [
        'vertical-slice',
        ['--smoke', frame],
        'PASS KIVORI-VIRTUAL-DEVICE:',
        1,
        ['firmware Runtime', 'desktop session/action core'],
      ],
      [
        'connections',
        ['--connection-smoke'],
        'PASS S5.2 ',
        9,
        ['firmware Runtime', 'desktop session/connection policy'],
      ],
      [
        'desktop',
        ['--desktop-smoke'],
        'PASS S5.4 ',
        14,
        ['labelled firmware-session and desktop-action cases', 'ConfigStore'],
      ],
      [
        'recovery',
        ['--recovery-smoke'],
        'PASS S5.5 ',
        9,
        ['labelled policy/admission and virtual-session cases'],
      ],
      [
        'presence',
        ['--presence-smoke'],
        'PASS PRESENCE ',
        5,
        ['production host-presence tracker/actions', 'desktop session', 'firmware Runtime'],
      ],
    ]) {
      await run(app, [runner, ...args], {
        stage: {
          id,
          clockMode: id === 'recovery' ? 'mixed-real-session-and-injected-policy' : 'real-elapsed',
          seed: null,
          layers,
          simulatedLayers: ['OS services', 'hardware adapters', 'flash/download/install executors'],
          expectedMarkers: { prefix, count },
        },
      });
    }
    copyFileSync(frame, resolve(report.directory, 'firmware-frame.ppm'));
    report.data.firmwareFrame = { path: 'firmware-frame.ppm', sha256: sha256(readFileSync(frame)) };
    for (const seed of REPLAY_SEEDS) {
      const runs = [];
      for (const repetition of ['first', 'second']) {
        runs.push(
          await run(runner, ['--replay', String(seed)], {
            stage: {
              id: `replay-${seed}-${repetition}`,
              seed,
              format: 'replay-json',
              clockMode: 'injected-firmware',
              layers: ['firmware Runtime'],
              simulatedLayers: ['input', 'clock', 'transport', 'display capture'],
            },
          }),
        );
      }
      addReplay(report, seed, ...runs);
      console.log(`Replay seed ${seed}: four cases passed; repeated JSON is byte-identical.`);
    }
    console.log(`Firmware pixel artifact: ${frame}`);
  } else {
    if (process.platform === 'darwin') {
      // A local .app gives macOS a distinct inspectable development window; no installation/signing.
      const bundle = resolve(target, 'Kivori Virtual Device.app/Contents');
      mkdirSync(resolve(bundle, 'MacOS'), { recursive: true });
      const executable = resolve(bundle, 'MacOS/virtual_device');
      copyFileSync(app, executable);
      writeFileSync(
        resolve(bundle, 'Info.plist'),
        `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>virtual_device</string>
<key>CFBundleIdentifier</key><string>id.immer.kivori.virtual-device-dev</string>
<key>CFBundleName</key><string>Kivori Virtual Device</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
</dict></plist>`,
      );
      app = executable;
    }
    // Programmatic Vite binds its own strict port before starting the app; never reuse another server.
    const require = createRequire(resolve(root, 'apps/desktop/package.json'));
    const { createServer } = await import(pathToFileURL(require.resolve('vite')).href);
    ui = await createServer({
      root: resolve(root, 'apps/desktop'),
      configFile: resolve(root, 'apps/desktop/vite.config.ts'),
      server: { host: 'localhost', port: 1420, strictPort: true },
    });
    await ui.listen();
    ui.httpServer.once('close', () => active?.kill());
    await run(app, [runner]);
  }
} catch (error) {
  smokeError = error;
} finally {
  active?.kill();
  await ui?.close();
  if (report) {
    smokeError = report.finish(smokeError ?? cancellation);
    console.log(`Virtual smoke report (${report.data.status}): ${report.path}`);
  }
}
if (smokeError) throw smokeError;
