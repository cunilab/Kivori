import { describe, expect, it } from 'vitest';
import { friendlyAppName, groupApps, isBundleId } from '../app-names';

describe('app names', () => {
  it('names known apps the same under every alias', () => {
    for (const id of ['chrome', 'Chrome.exe', 'com.google.chrome']) {
      expect(friendlyAppName(id)).toBe('Chrome');
    }
    expect(friendlyAppName('com.todesktop.230313mzl4w4u92')).toBe('Cursor');
    expect(friendlyAppName('idea64.exe')).toBe('IntelliJ IDEA');
  });

  it('falls back to the last bundle segment, de-numbered and title-cased', () => {
    expect(friendlyAppName('com.example.my-notes')).toBe('My Notes');
    expect(friendlyAppName('notepad++.exe')).toBe('Notepad++');
    expect(friendlyAppName('tool64.exe')).toBe('Tool');
    expect(isBundleId('com.google.chrome')).toBe(true);
    expect(isBundleId('chrome.exe')).toBe(false);
  });

  it('merges aliases and prefers the id that belongs to this OS', () => {
    const ids = ['chrome.exe', 'com.google.chrome', 'msedge'];
    expect(groupApps(ids, 'macos')).toEqual([
      { name: 'Chrome', primary: 'com.google.chrome', ids: ['chrome.exe', 'com.google.chrome'] },
      { name: 'Edge', primary: 'msedge', ids: ['msedge'] },
    ]);
    expect(groupApps(ids, 'windows')[0]?.primary).toBe('chrome.exe');
    // Nothing for this OS: keep what is stored rather than hide the app.
    expect(groupApps(['com.google.chrome'], 'windows')[0]?.primary).toBe('com.google.chrome');
  });
});
