// Product-owned launcher. No flashing, real OS services or shipping app entry is used.
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { createRequire } from 'node:module';
import { mkdirSync, copyFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const mode = process.argv[2];
if (mode !== undefined && mode !== '--smoke') throw new Error('usage: run.mjs [--smoke]');
const env = { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || '2' };
delete env.RUSTFLAGS;
delete env.CARGO_ENCODED_RUSTFLAGS;
delete env.KIVORI_FIRMWARE_PATH;
delete env.KIVORI_FIRMWARE_MANIFEST;
let active;
let ui;
function stop() {
  active?.kill();
  void ui?.close();
}
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
async function run(command, args, options = {}) {
  const child = spawn(command, args, { cwd: root, env, stdio: 'inherit', ...options });
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
  const host = await new Promise((yes, no) => {
    const child = spawn('rustc', ['-vV'], { cwd: root, env });
    let text = '';
    child.stdout.on('data', (chunk) => {
      text += chunk;
    });
    child.once('error', no);
    child.once('exit', (code) => {
      const host = /^host: (.+)$/m.exec(text)?.[1];
      if (code !== 0 || !host) no(new Error('rustc host target unavailable'));
      else yes(host);
    });
  });
  const manifest = 'firmware/pala/simulator/Cargo.toml';
  const target = resolve(root, 'firmware/pala/simulator/target');
  await run('cargo', [
    'build',
    '--locked',
    '--manifest-path',
    manifest,
    '--target',
    host,
    '--target-dir',
    target,
  ]);
  await run('cargo', [
    'build',
    '--locked',
    '-p',
    'kivori-desktop',
    '--features',
    'virtual-device',
    '--example',
    'virtual_device',
  ]);
  const ext = process.platform === 'win32' ? '.exe' : '';
  const runner = resolve(target, host, 'debug', `kivori-pip-simulator${ext}`);
  let app = resolve(root, 'target/debug/examples', `virtual_device${ext}`);
  if (mode === '--smoke') {
    await run('cargo', [
      'test',
      '--locked',
      '--manifest-path',
      manifest,
      '--target',
      host,
      '--target-dir',
      target,
    ]);
    await run(runner, ['--input-smoke']);
    await run('cargo', [
      'test',
      '--locked',
      '-p',
      'kivori-desktop',
      '--features',
      'virtual-device',
      '--lib',
      'virtual_device::tests',
    ]);
    const frame = resolve(target, 'smoke-frame.ppm');
    await run(app, [runner, '--smoke', frame]);
    await run(app, [runner, '--connection-smoke']);
    await run(app, [runner, '--desktop-smoke']);
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
} finally {
  active?.kill();
  await ui?.close();
}
