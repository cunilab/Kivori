// Draws the demo device's 240x240 screen (at 2x) from a DemoState: the volume gauge and browser tab
// strip are drawn here; buddy, media, clock and system use the real frames from public/media/screens.
import { TAB_COUNT, moodFor, screenLabels, type DemoState } from './demo';

export const SCREEN_NAMES = [
  'buddy',
  'clock',
  'media',
  'system',
  'mood-idle',
  'mood-happy',
  'mood-listening',
  'mood-attentive',
] as const;
export type ScreenName = (typeof SCREEN_NAMES)[number];
export type ScreenImages = Partial<Record<ScreenName, HTMLImageElement>>;

const S = 2;
const MINT = '#7df2c4';
const DIM = '#8f9bb0';
const FAINT = '#6c7891';
const TRACK = '#1d2840';
const SKY = '#4fb7ff';
const AMBER = '#f5a524';
export const FALLBACK_BG = '#080c16';

function ready(image: HTMLImageElement | undefined): image is HTMLImageElement {
  return Boolean(image && image.complete && image.naturalWidth > 0);
}

/** The panel background is the top-left pixel of the idle mood frame, so drawn views match the frames. */
export function sampleBackground(image: HTMLImageElement): string {
  try {
    const probe = document.createElement('canvas');
    probe.width = probe.height = 4;
    const ctx = probe.getContext('2d');
    if (!ctx) return FALLBACK_BG;
    ctx.drawImage(image, 0, 0, 4, 4, 0, 0, 4, 4);
    const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
    return `rgb(${r},${g},${b})`;
  } catch {
    return FALLBACK_BG;
  }
}

export interface ScreenContext {
  ctx: CanvasRenderingContext2D;
  images: ScreenImages;
  background: string;
  /** CSS font-family list for the pixel font (next/font variable), resolved by the component. */
  family: string;
  /** Wall-clock label for the buddy view. */
  time: string;
}

type Align = 'left' | 'center' | 'right';

function text(
  c: ScreenContext,
  value: string,
  x: number,
  y: number,
  size: number,
  color: string,
  align: Align = 'center',
  weight = 400,
): void {
  c.ctx.font = `${weight} ${size * S}px ${c.family}`;
  c.ctx.fillStyle = color;
  c.ctx.textAlign = align;
  c.ctx.textBaseline = 'middle';
  c.ctx.fillText(value, x * S, y * S);
}

function clear(c: ScreenContext): void {
  c.ctx.imageSmoothingEnabled = false;
  c.ctx.fillStyle = c.background;
  c.ctx.fillRect(0, 0, 240 * S, 240 * S);
}

function drawBuddy(c: ScreenContext, state: DemoState): void {
  clear(c);
  const face = c.images[moodFor(state)];
  if (ready(face)) c.ctx.drawImage(face, 30 * S, 22 * S, 180 * S, 180 * S);
  const labels = screenLabels(state);
  text(c, c.time, 120, 13, 9, DIM);
  if (labels.profileLabel) text(c, labels.profileLabel, 8, 13, 8, MINT, 'left', 700);
  if (labels.muted) text(c, 'MUTED', 232, 13, 8, AMBER, 'right', 700);
  for (const [x, heading, value] of labels.columns) {
    text(c, heading, x, 205, 6.5, FAINT);
    text(c, value, x, 219, 8, '#ffffff', 'center', 700);
  }
}

function drawVolume(c: ScreenContext, state: DemoState): void {
  clear(c);
  const { ctx } = c;
  const cx = 120;
  const cy = 112;
  const r = 84;
  const start = Math.PI * 0.75;
  const sweep = Math.PI * 1.5;
  ctx.lineCap = 'round';
  ctx.lineWidth = 16 * S;
  ctx.strokeStyle = TRACK;
  ctx.beginPath();
  ctx.arc(cx * S, cy * S, r * S, start, start + sweep);
  ctx.stroke();
  const level = state.muted ? 0 : state.vol;
  if (level > 0) {
    ctx.strokeStyle = SKY;
    ctx.beginPath();
    ctx.arc(cx * S, cy * S, r * S, start, start + sweep * (level / 100));
    ctx.stroke();
  }
  if (state.muted) {
    text(c, 'MUTED', cx, cy, 20, AMBER, 'center', 700);
  } else {
    const digits = String(state.vol);
    ctx.font = `700 ${46 * S}px ${c.family}`;
    const wn = ctx.measureText(digits).width / S;
    ctx.font = `700 ${16 * S}px ${c.family}`;
    const wp = ctx.measureText('%').width / S;
    const left = cx - (wn + 3 + wp) / 2;
    text(c, digits, left, cy - 2, 46, '#ffffff', 'left', 700);
    text(c, '%', left + wn + 3, cy + 10, 16, '#ffffff', 'left', 700);
  }
  text(c, 'VOLUME', cx, cy + 40, 8, DIM);
  text(c, '0', 52, 200, 7, FAINT);
  text(c, '100', 188, 200, 7, FAINT);
}

function drawTabs(c: ScreenContext, state: DemoState): void {
  clear(c);
  const { ctx } = c;
  const w = 30;
  const gap = 6;
  const x0 = (240 - (TAB_COUNT * w + (TAB_COUNT - 1) * gap)) / 2;
  text(c, 'BROWSER', 8, 13, 8, MINT, 'left', 700);
  for (let i = 0; i < TAB_COUNT; i++) {
    const on = i === state.tab;
    ctx.fillStyle = on ? MINT : TRACK;
    ctx.beginPath();
    ctx.roundRect((x0 + i * (w + gap)) * S, (on ? 92 : 98) * S, w * S, (on ? 34 : 26) * S, 6 * S);
    ctx.fill();
  }
  text(c, `TAB ${state.tab + 1} OF ${TAB_COUNT}`, 120, 156, 14, '#ffffff', 'center', 700);
  text(c, 'turn to switch', 120, 180, 7, DIM);
}

function drawFrame(c: ScreenContext, name: ScreenName): void {
  clear(c);
  const frame = c.images[name];
  if (ready(frame)) c.ctx.drawImage(frame, 0, 0, 240 * S, 240 * S);
}

export function drawScreen(c: ScreenContext, state: DemoState): void {
  switch (state.view) {
    case 'volume':
      return drawVolume(c, state);
    case 'tabs':
      return drawTabs(c, state);
    case 'media':
    case 'clock':
    case 'system':
      return drawFrame(c, state.view);
    default:
      return drawBuddy(c, state);
  }
}
