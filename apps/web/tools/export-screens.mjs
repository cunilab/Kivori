// Dev-only: turns the device renderer's 240x240 frames into the marketing screen images in
// public/media/screens/. Not part of the web build, so CI never needs Rust. The WebP output is committed.
//
// 1. Produce the frames (needs Rust; roughly 1 GB of build output, delete target/ afterwards):
//      export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
//      cargo run -p kivori-golden-frames --example desk_review -- <frames-dir>
//      cargo run -p kivori-golden-frames --example mascot_review -- <frames-dir>/mascot   (also writes
//      assets/compiled/review/*.ppm, which this script reads for the buddy moods)
// 2. bun apps/web/tools/export-screens.mjs <frames-dir> [<review-ppm-dir>]
//
// Each frame is scaled x4 with nearest-neighbour (960x960, pixels stay crisp) and saved as lossless WebP.
// Only normal user-facing views are listed; debug, probe and diagnostic frames are deliberately left out.
import { mkdir, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import sharp from 'sharp';

const OUT = fileURLToPath(new URL('../public/media/screens/', import.meta.url));
const SCALE = 4;

// The five display views, one frame each.
const VIEWS = {
  buddy: 'buddy_controls.mascot.png',
  clock: 'clock_0941_30s.png',
  volume: 'volume_42.png',
  media: 'media_playing_info.png',
  system: 'system_history.png',
};

// Buddy moods, from the mascot review PPMs (P6, 240x240).
const MOODS = {
  'mood-idle': 'idle-0',
  'mood-happy': 'happy-1200',
  'mood-listening': 'face-listening',
  'mood-attentive': 'face-attentive',
  'mood-busy': 'busy-1200',
  'mood-sleeping': 'sleeping-600',
};

async function save(input, name) {
  await sharp(input)
    .resize(240 * SCALE, 240 * SCALE, { kernel: 'nearest' })
    .webp({ lossless: true, effort: 6 })
    .toFile(join(OUT, `${name}.webp`));
  console.log('wrote', `${name}.webp`);
}

const [
  framesDir,
  ppmDir = fileURLToPath(new URL('../../../assets/compiled/review/', import.meta.url)),
] = process.argv.slice(2);
if (!framesDir) {
  console.error('usage: bun apps/web/tools/export-screens.mjs <frames-dir> [<review-ppm-dir>]');
  process.exit(1);
}

await mkdir(OUT, { recursive: true });
for (const [name, file] of Object.entries(VIEWS)) await save(join(framesDir, file), name);
for (const [name, file] of Object.entries(MOODS)) {
  const bytes = await readFile(join(ppmDir, `${file}.ppm`));
  // Header is "P6\n240 240\n255\n" (15 bytes), then raw RGB.
  await save(
    await sharp(bytes.subarray(15), { raw: { width: 240, height: 240, channels: 3 } })
      .png()
      .toBuffer(),
    name,
  );
}
