import { describe, expect, it } from 'vitest';
import { DEFAULT_SITE_URL, resolveSiteUrl } from './site';

describe('resolveSiteUrl', () => {
  it('falls back to the dev origin when unset or blank', () => {
    expect(resolveSiteUrl(undefined).href).toBe(`${DEFAULT_SITE_URL}/`);
    expect(resolveSiteUrl('   ').href).toBe(`${DEFAULT_SITE_URL}/`);
  });

  it('accepts an absolute https origin and drops any path', () => {
    expect(resolveSiteUrl('https://kivori.example/some/path').href).toBe('https://kivori.example/');
  });

  it('rejects malformed or non-http values', () => {
    expect(resolveSiteUrl('not a url').href).toBe(`${DEFAULT_SITE_URL}/`);
    expect(resolveSiteUrl('ftp://kivori.example').href).toBe(`${DEFAULT_SITE_URL}/`);
  });
});
