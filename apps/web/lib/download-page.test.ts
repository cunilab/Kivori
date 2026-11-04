import { describe, expect, it } from 'vitest';
import { downloadState } from './download-page';
import type { Release } from './releases';

const release = (installer: Release['installer']): Release => ({
  version: '0.1.0',
  tag: 'v0.1.0',
  date: '2026-02-01T00:00:00Z',
  prerelease: false,
  notes: '',
  firmware: [],
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

  it('distinguishes a missing installer from an empty release history', () => {
    const releases = [release(null)];
    expect(downloadState({ status: 'ok', releases })).toEqual({
      kind: 'missing',
      release: releases[0],
    });
    expect(downloadState({ status: 'ok', releases: [] })).toEqual({ kind: 'empty', releases: [] });
  });

  it('does not choose beta as the stable default download', () => {
    const stable = release({ size: 10, url: 'https://objects.example.test/stable.exe' });
    const beta = { ...stable, version: '0.2.0-beta.1', prerelease: true };
    expect(downloadState({ status: 'ok', releases: [beta, stable] })).toMatchObject({
      kind: 'ready',
      release: stable,
    });
    expect(downloadState({ status: 'ok', releases: [beta] }).kind).toBe('empty');
  });

  it('is unavailable when the releases could not be loaded', () => {
    expect(downloadState({ status: 'error' })).toEqual({ kind: 'unavailable' });
  });
});
