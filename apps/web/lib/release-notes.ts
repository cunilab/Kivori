import notes010 from '@/content/releases/0.1.0.md';

// Curated notes remain a fallback for releases without a published body.
const NOTES: Record<string, string> = {
  '0.1.0': notes010,
};

export const FALLBACK_NOTE = 'Improvements and fixes.';

export type NoteBlock =
  | { kind: 'paragraph'; text: string }
  | { kind: 'heading'; text: string }
  | { kind: 'list'; items: string[] };

function clean(text: string): string {
  return text
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/<[^>]*>/g, '')
    .replace(/[`*_~]/g, '')
    .replace(/https?:\/\/\S+/gi, '')
    .replace(/(?:api\.)?github\.com(?:\/[^\s]*)?/gi, '')
    .replace(/\bcunilab\/kivori(?:#[0-9]+)?\b/gi, '')
    .replace(/ESP32|ST7789|HW-040|SuperMini/gi, 'device')
    .replace(/blueprint/gi, 'design')
    .replace(/\b\d+(?:\.\d+)?\s?mm\b/gi, 'specified size')
    .replace(/\bTBC\b/g, 'pending confirmation')
    .replace(/\(?#[0-9]+\)?/g, '')
    .replace(/\b[0-9a-f]{7,40}\b/gi, '')
    .replace(/@[\w-]+/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Parses the tiny supported subset: blank-line separated paragraphs and `- ` bullet runs. */
export function parseNotes(source: string): NoteBlock[] {
  const blocks: NoteBlock[] = [];
  let paragraph: string[] = [];
  let list: string[] | null = null;
  const flushParagraph = (): void => {
    if (paragraph.length) blocks.push({ kind: 'paragraph', text: paragraph.join(' ') });
    paragraph = [];
  };
  const flushList = (): void => {
    if (list?.length) blocks.push({ kind: 'list', items: list });
    list = null;
  };
  for (const raw of source.split(/\r?\n/)) {
    const line = clean(raw.trim());
    if (/^full changelog:?$/i.test(line)) continue;
    if (!line) {
      flushParagraph();
      flushList();
    } else if (/^#{1,6}\s/.test(line)) {
      flushParagraph();
      flushList();
      blocks.push({ kind: 'heading', text: line.replace(/^#{1,6}\s+/, '') });
    } else if (line.startsWith('- ')) {
      flushParagraph();
      (list ??= []).push(line.slice(2).trim());
    } else {
      flushList();
      paragraph.push(line);
    }
  }
  flushParagraph();
  flushList();
  return blocks;
}

/** Notes for a version, or the one-line fallback when no file was written for it. */
export function notesFor(version: string, notes?: string): NoteBlock[] {
  const text = notes || NOTES[version];
  const blocks = text ? parseNotes(text) : [];
  return blocks.length ? blocks : [{ kind: 'paragraph', text: FALLBACK_NOTE }];
}
