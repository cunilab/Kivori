import { describe, expect, it } from 'vitest';
import { FALLBACK_NOTE, notesFor, parseNotes } from './release-notes';

describe('release notes', () => {
  it('parses paragraphs and bullet runs', () => {
    expect(parseNotes('Intro line\ncontinues.\n\n- one\n- two\n\nOutro')).toEqual([
      { kind: 'paragraph', text: 'Intro line continues.' },
      { kind: 'list', items: ['one', 'two'] },
      { kind: 'paragraph', text: 'Outro' },
    ]);
  });

  it('returns the curated notes for a version that has a file', () => {
    const blocks = notesFor('0.1.0');
    expect(blocks.some((block) => block.kind === 'list')).toBe(true);
    expect(JSON.stringify(blocks)).not.toContain(FALLBACK_NOTE);
  });

  it('falls back to a one-line note for a version without a file', () => {
    expect(notesFor('9.9.9')).toEqual([{ kind: 'paragraph', text: 'Improvements and fixes.' }]);
  });
});
