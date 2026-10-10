import { describe, expect, it } from 'vitest';
import { detectOs } from './os';

describe('detectOs', () => {
  it('detects Windows', () => {
    expect(detectOs('Windows', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toBe('windows');
  });

  it('detects a Mac from the platform hint or the user agent', () => {
    expect(detectOs('macOS', undefined)).toBe('macos');
    expect(detectOs(undefined, 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe('macos');
  });

  it('defaults to Windows when unknown, and for phones', () => {
    expect(detectOs(undefined, undefined)).toBe('windows');
    expect(detectOs('Linux', 'X11; Linux x86_64')).toBe('windows');
    expect(detectOs(undefined, 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)')).toBe(
      'windows',
    );
  });
});
