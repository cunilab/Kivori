// Build-time image pipeline, run by `predev`/`prebuild`/`preview` through bun (it imports the TS content).
// Nothing here ships in the Worker: it writes static files into the gitignored public/generated/.
//   1. Gallery: docs/assets/*.png -> resized WebP (1600 px and 800 px wide). The 1.6 MB PNG sources stay
//      where they are and are never copied into git.
//   2. Share images: 1200x630 PNGs per product plus default.png, composed with satori (text becomes
//      paths, so no font is needed at rasterise time) and rasterised with sharp. Rendering these at
//      request time with ImageResponse bloats the Worker with the resvg wasm and 500s on prerender.
import { mkdir, readFile, stat } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';
import satori from 'satori';
import { getProducts } from '../content/products/index.ts';
import { PRODUCT_STATUS_LABEL } from '../lib/product-labels.ts';
import { TAGLINE } from '../lib/site.ts';

const WEB_ROOT = fileURLToPath(new URL('..', import.meta.url));
const REPO_ROOT = fileURLToPath(new URL('../../..', import.meta.url));
const OUT = `${WEB_ROOT}public/generated`;
const require = createRequire(import.meta.url);

const GALLERY = [
  {
    source: `${REPO_ROOT}docs/assets/soft-arc-faceplate-blueprint.png`,
    name: 'soft-arc-faceplate-blueprint',
  },
  { source: `${REPO_ROOT}docs/assets/image.png`, name: 'image' },
];
const WIDTHS = [1600, 800];

async function isFresh(output, source) {
  try {
    const [o, s] = await Promise.all([stat(output), stat(source)]);
    return o.mtimeMs >= s.mtimeMs;
  } catch {
    return false;
  }
}

async function buildGallery() {
  await mkdir(OUT, { recursive: true });
  for (const { source, name } of GALLERY) {
    for (const width of WIDTHS) {
      const output = `${OUT}/${name}-${width}.webp`;
      if (await isFresh(output, source)) continue;
      await sharp(source)
        .resize({ width, withoutEnlargement: true })
        .webp({ quality: 82 })
        .toFile(output);
      console.log(`[assets] ${name}-${width}.webp`);
    }
  }
}

const NAVY = '#0c101c';
const GREEN = '#7df2c4';

function el(type, style, children) {
  return { type, props: { style: { display: 'flex', ...style }, children } };
}

function card({ title, line, status, mascotUri }) {
  return el(
    'div',
    {
      width: 1200,
      height: 630,
      background: NAVY,
      color: '#e8ecf5',
      fontFamily: 'Space Grotesk',
      padding: 72,
      position: 'relative',
    },
    [
      el(
        'div',
        { flexDirection: 'column', justifyContent: 'space-between', width: 700, height: 486 },
        [
          el(
            'div',
            { alignItems: 'center', fontSize: 28, color: GREEN, fontWeight: 700 },
            'Desk buddy and controller',
          ),
          el('div', { flexDirection: 'column' }, [
            el(
              'div',
              { fontSize: title.length > 12 ? 84 : 112, fontWeight: 700, lineHeight: 1.05 },
              title,
            ),
            el('div', { fontSize: 34, color: '#a9b3c9', marginTop: 24, lineHeight: 1.25 }, line),
          ]),
          el(
            'div',
            { alignItems: 'center', height: 44 },
            status
              ? [
                  el(
                    'div',
                    {
                      fontSize: 24,
                      color: NAVY,
                      background: GREEN,
                      borderRadius: 999,
                      padding: '6px 22px',
                      fontWeight: 700,
                    },
                    status,
                  ),
                ]
              : [],
          ),
        ],
      ),
      {
        type: 'img',
        props: {
          src: mascotUri,
          width: 420,
          height: 420,
          style: { position: 'absolute', left: 724, top: 105 },
        },
      },
    ],
  );
}

async function buildShareImages() {
  await mkdir(`${OUT}/og`, { recursive: true });
  const fontFile = (weight) =>
    readFile(
      require.resolve(`@fontsource/space-grotesk/files/space-grotesk-latin-${weight}-normal.woff`),
    );
  const fonts = [
    { name: 'Space Grotesk', data: await fontFile(700), weight: 700, style: 'normal' },
    { name: 'Space Grotesk', data: await fontFile(400), weight: 400, style: 'normal' },
  ];
  const mascotPng = await sharp(`${REPO_ROOT}assets/mascot.svg`, { density: 288 })
    .resize(420, 420)
    .png()
    .toBuffer();
  const mascotUri = `data:image/png;base64,${mascotPng.toString('base64')}`;

  const cards = [
    { slug: 'default', title: 'Kivori', line: TAGLINE },
    ...getProducts().map((p) => ({
      slug: p.slug,
      title: p.name,
      line: p.tagline,
      status: PRODUCT_STATUS_LABEL[p.status],
    })),
  ];
  for (const { slug, ...rest } of cards) {
    const svg = await satori(card({ ...rest, mascotUri }), { width: 1200, height: 630, fonts });
    await sharp(Buffer.from(svg)).png().toFile(`${OUT}/og/${slug}.png`);
    console.log(`[assets] og/${slug}.png`);
  }
}

await buildGallery();
await buildShareImages();
