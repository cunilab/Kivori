import { describe, expect, it } from 'vitest';
import { downloadState } from './download-page';
import type { Release } from './releases';

const release = (installer: Release['installer']): Release => ({
  version: '0.1.0',
  tag: 'v0.1.0',
  date: '2026-02-01T00:00:00Z',
  prerelease: true,
  installer,
});

describe('downloadState', () => {
  it('is ready when the newest release has an installer', () => {
    const installer = { size: 10, url: 'https://objects.example.test/a.exe' };
    expect(downloadState({ status: 'ok', releases: [release(installer)] })).toMatchObject({
      kind: 'ready',
      installer,
    });
  });

  it('is the empty state while no release has an installer, history still listed', () => {
    const releases = [release(null)];
    expect(downloadState({ status: 'ok', releases })).toEqual({ kind: 'empty', releases });
    expect(downloadState({ status: 'ok', releases: [] })).toEqual({ kind: 'empty', releases: [] });
  });

  it('is unavailable when the releases could not be loaded', () => {
    expect(downloadState({ status: 'error' })).toEqual({ kind: 'unavailable' });
  });
});
