import { describe, expect, it, vi } from 'vitest';

vi.mock('@opennextjs/cloudflare', () => ({ initOpenNextCloudflareForDev: () => undefined }));

describe('next.config redirects', () => {
  it('sends /changelog to the download history', async () => {
    const { default: config } = await import('../next.config');
    const rules = (await config.redirects?.()) ?? [];
    expect(rules).toContainEqual(
      expect.objectContaining({ source: '/changelog', destination: '/download#history' }),
    );
  });
});
