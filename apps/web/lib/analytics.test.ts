import { describe, expect, it, vi } from 'vitest';
import { downloadDataPoint, recordDownload, referrerHost } from './analytics';

describe('downloadDataPoint', () => {
  it('stores os, version, referrer host and utm_source only', () => {
    expect(
      downloadDataPoint({
        os: 'windows',
        version: '0.2.0',
        referrer: 'https://news.example/post?id=1',
        utmSource: 'reddit',
      }),
    ).toEqual({
      blobs: ['windows', '0.2.0', 'news.example', 'reddit'],
      doubles: [1],
      indexes: ['windows'],
    });
  });

  it('uses empty strings for missing or invalid values', () => {
    expect(referrerHost('not a url')).toBe('');
    expect(
      downloadDataPoint({ os: 'windows', version: '1', referrer: null, utmSource: null }).blobs,
    ).toEqual(['windows', '1', '', '']);
  });
});

describe('recordDownload', () => {
  it('writes to a provided dataset', () => {
    const writeDataPoint = vi.fn();
    recordDownload(
      { os: 'windows', version: '1', referrer: null, utmSource: null },
      { writeDataPoint },
    );
    expect(writeDataPoint).toHaveBeenCalledOnce();
  });

  it('is a no-op without the binding', () => {
    expect(() =>
      recordDownload({ os: 'windows', version: '1', referrer: null, utmSource: null }),
    ).not.toThrow();
  });
});
