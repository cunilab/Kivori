import notes010 from '@/content/releases/0.1.0.md';

// Curated, user-facing notes. Add `content/releases/<version>.md` and list it here; the GitHub release
// body is never read. Only `- ` bullets and plain paragraphs are understood.
const NOTES: Record<string, string> = {
  '0.1.0': notes010,
};

export const FALLBACK_NOTE = 'Improvements and fixes.';

export type NoteBlock = { kind: 'paragraph'; text: string } | { kind: 'list'; items: string[] };

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
    const line = raw.trim();
    if (!line) {
      flushParagraph();
      flushList();
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
export function notesFor(version: string, source: Record<string, string> = NOTES): NoteBlock[] {
  const text = source[version];
  const blocks = text ? parseNotes(text) : [];
  return blocks.length ? blocks : [{ kind: 'paragraph', text: FALLBACK_NOTE }];
}
