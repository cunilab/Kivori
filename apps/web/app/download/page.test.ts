import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { loadReleases, type Release, type ReleasesResult } from '@/lib/releases';
import DownloadPage from './page';

vi.mock('@/lib/releases', () => ({ loadReleases: vi.fn() }));

const installer = { size: 1024, url: 'https://example.test/installer' };
const release: Release = {
  version: '0.3.1',
  tag: 'v0.3.1',
  date: '2026-11-03T00:00:00Z',
  prerelease: false,
  installer,
  macInstaller: installer,
  notes: '',
  firmware: [],
};

async function page(result: ReleasesResult): Promise<string> {
  vi.mocked(loadReleases).mockResolvedValue(result);
  return renderToStaticMarkup(await DownloadPage());
}

describe('download page release states', () => {
  it('offers each available installer and preserves the stale notice', async () => {
    const html = await page({ status: 'ok', releases: [release], stale: true });
    expect(html).toContain('href="/download/windows/latest"');
    expect(html).toContain('href="/download/macos/latest"');
    expect(html).toContain('Release information may be out of date');
  });

  it('offers only the available macOS installer for a mac-only release', async () => {
    const html = await page({ status: 'ok', releases: [{ ...release, installer: null }] });
    expect(html).not.toContain('href="/download/windows/latest"');
    expect(html).toContain('href="/download/macos/latest"');
    expect(html).toContain('Windows build not published');
  });

  it('explains empty and unavailable states without suggesting downloads', async () => {
    const empty = await page({ status: 'ok', releases: [] });
    expect(empty).toContain('First release coming soon');
    expect(empty).toContain('Get updates');
    expect(empty).not.toContain('href="/download/windows/latest"');
    const unavailable = await page({ status: 'error' });
    expect(unavailable).toContain('Downloads are temporarily unavailable');
    expect(unavailable).not.toContain('href="/download/macos/latest"');
  });
});

it('offers Linux formats and JSON-LD only when the latest release has Linux assets', async () => {
  const absent = await page({ status: 'ok', releases: [release] });
  expect(absent).not.toContain('href="/download/linux/latest');
  expect(absent).not.toContain('Linux (Ubuntu/Debian x86_64)');
  const present = await page({
    status: 'ok',
    releases: [{ ...release, linuxDeb: installer, linuxAppImage: installer }],
  });
  expect(present).toContain('href="/download/linux/latest"');
  expect(present).toContain('href="/download/linux/latest?format=appimage"');
  expect(present).toContain('Linux (Ubuntu/Debian x86_64)');
  const imageOnly = await page({
    status: 'ok',
    releases: [{ ...release, linuxAppImage: installer }],
  });
  expect(imageOnly).not.toContain('href="/download/linux/latest"');
  expect(imageOnly).toContain('href="/download/linux/latest?format=appimage"');
});
