// Dev-only studio renderer for the marketing product images. Not part of the web build.
//   bun apps/web/tools/render-studio/render.mjs [shot-name ...]
// Serves scene.html locally (127.0.0.1, only while rendering), loads the enclosure STLs from
// hardware/enclosure/export/ at render time (they are never copied into public/), renders each shot in
// the installed Google Chrome and writes WebP files (plus a 1200 px variant) to public/media/renders/.
import { createServer } from 'node:http';
import { readFile, mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname, join, normalize } from 'node:path';
import puppeteer from 'puppeteer-core';
import sharp from 'sharp';

const HERE = dirname(fileURLToPath(import.meta.url));
const WEB = join(HERE, '../..');
const STL_DIR = join(WEB, '../../hardware/enclosure/export');
const SCREENS_DIR = join(WEB, 'public/media/screens');
const OUT_DIR = join(WEB, 'public/media/renders');
const THREE_DIR = dirname(createRequire(import.meta.url).resolve('three/package.json'));
const CHROME =
  process.env.CHROME_PATH ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

const PARTS = ['Shell', 'Visor', 'BackPlate', 'Knob', 'CapLeft', 'CapMiddle', 'CapRight']; // Carrier is internal
const WHITE = '#ffffff';
const TYPES = { '.js': 'text/javascript', '.html': 'text/html', '.webp': 'image/webp' };

// Camera: azimuth/elevation in degrees around `target` (mm, world space, floor at y = 0), distance in mm.
// The device is about 110 wide, 70 deep; its face centre is near [0, 14, 0].
const SHOTS = [
  {
    name: 'hero-34',
    width: 1200,
    height: 900,
    screen: 'buddy',
    fov: 22,
    target: [4, 11, 2],
    azimuth: -32,
    elevation: 24,
    distance: 320,
  },
  {
    name: 'front',
    width: 1200,
    height: 800,
    screen: 'buddy',
    fov: 20,
    target: [0, 11, 0],
    azimuth: 0,
    elevation: 16,
    distance: 290,
  },
  {
    name: 'top',
    width: 1200,
    height: 900,
    screen: 'buddy',
    fov: 20,
    target: [0, 8, 0],
    azimuth: 0,
    elevation: 89,
    distance: 330,
    floorShadow: false,
  },
  {
    name: 'side',
    width: 1200,
    height: 800,
    screen: 'buddy',
    fov: 18,
    target: [0, 14, 0],
    azimuth: 90,
    elevation: 3,
    distance: 380,
  },
  {
    name: 'knob',
    width: 1200,
    height: 900,
    screen: 'buddy',
    fov: 24,
    target: [6, 14, 4],
    azimuth: -16,
    elevation: 30,
    distance: 275,
  },
  {
    name: 'hero-34-transparent',
    width: 1200,
    height: 900,
    screen: 'buddy',
    fov: 22,
    target: [4, 11, 2],
    azimuth: -32,
    elevation: 24,
    distance: 320,
    transparent: true,
  },
  ...['clock', 'volume', 'media'].map((view) => ({
    name: `front-${view}`,
    width: 1200,
    height: 800,
    screen: view,
    fov: 20,
    target: [0, 11, 0],
    azimuth: 0,
    elevation: 16,
    distance: 290,
  })),
];

const server = createServer(async (req, res) => {
  try {
    const url = new URL(req.url ?? '/', 'http://localhost');
    const path = decodeURIComponent(url.pathname);
    let file;
    if (path === '/') file = join(HERE, 'scene.html');
    else if (path === '/scene.js') file = join(HERE, 'scene.js');
    else if (path.startsWith('/three/')) file = join(THREE_DIR, normalize(path.slice(7)));
    else if (path.startsWith('/stl/') && PARTS.includes(path.slice(5)))
      file = join(STL_DIR, `${path.slice(5)}.stl`);
    else if (path.startsWith('/screens/') && /^[a-z-]+\.webp$/.test(path.slice(9)))
      file = join(SCREENS_DIR, path.slice(9));
    if (!file || (path.startsWith('/three/') && !file.startsWith(THREE_DIR))) {
      res.writeHead(404).end();
      return;
    }
    const ext = file.slice(file.lastIndexOf('.'));
    res
      .writeHead(200, { 'content-type': TYPES[ext] ?? 'application/octet-stream' })
      .end(await readFile(file));
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;

const wanted = process.argv.slice(2);
const shots = wanted.length ? SHOTS.filter((s) => wanted.includes(s.name)) : SHOTS;
await mkdir(OUT_DIR, { recursive: true });

const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: true,
  args: ['--ignore-gpu-blocklist', '--enable-webgl', '--hide-scrollbars'],
});
try {
  const page = await browser.newPage();
  page.on('console', (m) => console.log('[page]', m.text()));
  page.on('pageerror', (e) => console.error('[page error]', e.message));
  await page.goto(origin, { waitUntil: 'load' });
  await page.waitForFunction('window.studioReady === true', { timeout: 60000 });
  for (const s of shots) {
    await page.setViewport({ width: s.width, height: s.height, deviceScaleFactor: 2 });
    await page.evaluate((cfg) => window.shot(cfg), {
      ...s,
      background: s.transparent ? null : WHITE,
    });
    const png = await page.screenshot({ type: 'png', omitBackground: Boolean(s.transparent) });
    if (s.transparent) {
      await sharp(png)
        .png({ compressionLevel: 9 })
        .toFile(join(OUT_DIR, `${s.name}.png`));
      await sharp(png)
        .webp({ quality: 90, alphaQuality: 100 })
        .toFile(join(OUT_DIR, `${s.name}.webp`));
    } else {
      await sharp(png)
        .webp({ quality: 90 })
        .toFile(join(OUT_DIR, `${s.name}.webp`));
      await sharp(png)
        .resize({ width: 1200 })
        .webp({ quality: 90 })
        .toFile(join(OUT_DIR, `${s.name}-1200.webp`));
    }
    console.log('rendered', s.name);
  }
} finally {
  await browser.close();
  server.close();
}
