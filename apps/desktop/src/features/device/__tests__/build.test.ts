import { describe, expect, it } from 'vitest';
import { buildNotices } from '../build';

const A = 'c2ff8c4f68de';
const B = '0123456789ab';

describe('buildNotices', () => {
  it('flags uncommitted changes whatever the mode', () => {
    for (const dev of [false, true]) {
      expect(buildNotices({ sourceId: A, dirty: true }, A, dev).dirty).toBe(true);
      expect(buildNotices({ sourceId: A, dirty: false }, A, dev).dirty).toBe(false);
    }
  });

  it('flags a different source id in developer mode only', () => {
    expect(buildNotices({ sourceId: A, dirty: false }, B, true).differs).toBe(true);
    expect(buildNotices({ sourceId: A, dirty: false }, B, false).differs).toBe(false);
    expect(buildNotices({ sourceId: A, dirty: false }, A, true).differs).toBe(false);
  });

  it('never guesses from an unknown source id or an unreported build', () => {
    expect(buildNotices(null, B, true)).toEqual({ dirty: false, differs: false });
    expect(buildNotices(undefined, B, true)).toEqual({ dirty: false, differs: false });
    expect(buildNotices({ sourceId: null, dirty: false }, B, true).differs).toBe(false);
    expect(buildNotices({ sourceId: A, dirty: false }, null, true).differs).toBe(false);
    expect(buildNotices({ sourceId: A, dirty: false }, undefined, true).differs).toBe(false);
  });
});
