import { describe, expect, it } from 'vitest';
import { formatBytes, formatDate } from './format';

describe('formatBytes', () => {
  it('formats KB and MB', () => {
    expect(formatBytes(2048)).toBe('2 KB');
    expect(formatBytes(5.5 * 1024 * 1024)).toBe('5.5 MB');
  });
});

describe('formatDate', () => {
  it('formats in UTC and tolerates bad input', () => {
    expect(formatDate('2026-01-05T23:00:00Z')).toBe('January 5, 2026');
    expect(formatDate(null)).toBe('');
    expect(formatDate('nope')).toBe('');
  });
});
