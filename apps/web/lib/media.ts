// Typed manifest of the marketing images in public/media. Swap a file there (same name, same size) to
// replace a render with a photo. Alt text sells what the picture shows; it never names parts.
export interface MediaImage {
  src: string;
  width: number;
  height: number;
  alt: string;
}

const render = (
  name: string,
  ext: string,
  width: number,
  height: number,
  alt: string,
): MediaImage => ({
  src: `/media/renders/${name}.${ext}`,
  width,
  height,
  alt,
});

const screen = (name: string, alt: string): MediaImage => ({
  src: `/media/screens/${name}.webp`,
  width: 960,
  height: 960,
  alt,
});

const FRONT_ALT =
  'Kivori seen from the front: a black case, a big white-topped knob, three white keys and a small screen with the buddy on it.';

export const renders = {
  hero: render(
    'hero-34',
    'webp',
    2400,
    1800,
    'Kivori on a white desk: a black case with a big white-topped knob, three white keys and a bright little screen.',
  ),
  heroTransparent: render(
    'hero-34-transparent',
    'webp',
    2400,
    1800,
    'Kivori, the black and white desk buddy with a big knob, three keys and a bright little screen.',
  ),
  front: render('front', 'webp', 2400, 1600, FRONT_ALT),
  top: render(
    'top',
    'webp',
    2400,
    1800,
    'Kivori from above: the screen at the back, a roomy knob in the middle and three keys in front, easy to find by feel.',
  ),
  side: render(
    'side',
    'webp',
    2400,
    1600,
    'Kivori from the side: a slim wedge that tilts the screen towards you.',
  ),
  knob: render(
    'knob',
    'webp',
    2400,
    1800,
    'A close look at the knob and keys: grippy ribbed edge and soft keycap-style buttons.',
  ),
  frontClock: render(
    'front-clock',
    'webp',
    2400,
    1600,
    'Kivori showing a large clock on its screen.',
  ),
  frontVolume: render(
    'front-volume',
    'webp',
    2400,
    1600,
    'Kivori showing the volume dial on its screen.',
  ),
  frontMedia: render(
    'front-media',
    'webp',
    2400,
    1600,
    'Kivori showing the song that is playing on its screen.',
  ),
  // Transparent cut-outs (3000x2000 and 3000x1800) for coloured page grounds.
  heroCutout: render(
    'hero-34-cutout',
    'webp',
    3000,
    2000,
    'Kivori, the black and white desk buddy with a big knob, three keys and a bright little screen.',
  ),
  cutoutBuddy: render('front-cutout-buddy', 'webp', 3000, 1800, FRONT_ALT),
  cutoutVolume: render(
    'front-cutout-volume',
    'webp',
    3000,
    1800,
    'Kivori showing the volume dial on its screen.',
  ),
  cutoutMedia: render(
    'front-cutout-media',
    'webp',
    3000,
    1800,
    'Kivori showing the song that is playing on its screen.',
  ),
  cutoutSystem: render(
    'front-cutout-system',
    'webp',
    3000,
    1800,
    'Kivori showing live processor and memory gauges on its screen.',
  ),
} as const satisfies Record<string, MediaImage>;

export const screens = {
  buddy: screen(
    'buddy',
    'The Buddy view: your keycap friend with what each control does right now.',
  ),
  clock: screen('clock', 'The Clock view: a big, easy-to-read time.'),
  volume: screen('volume', 'The Volume view: a clear dial that follows your system volume.'),
  media: screen('media', 'The Media view: what is playing, with play and pause state.'),
  system: screen(
    'system',
    'The System view: live processor and memory gauges with a one-minute history.',
  ),
} as const satisfies Record<string, MediaImage>;

export const moods = {
  idle: screen('mood-idle', 'Buddy relaxed and waiting.'),
  happy: screen('mood-happy', 'Buddy smiling after something worked.'),
  listening: screen('mood-listening', 'Buddy with happy closed eyes.'),
  attentive: screen('mood-attentive', 'Buddy paying close attention.'),
  busy: screen('mood-busy', 'Buddy concentrating while it works.'),
  sleeping: screen('mood-sleeping', 'Buddy dozing off when you are away.'),
} as const satisfies Record<string, MediaImage>;

/** The 1200 px twin of a render (written by the asset step next to each 2400 px file). */
export function smallRender(image: MediaImage): MediaImage {
  return {
    ...image,
    src: image.src.replace(/\.webp$/, '-1200.webp'),
    width: image.width / 2,
    height: image.height / 2,
  };
}
