import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { strings } from '../lib/i18n/strings';
import { splashStatusText } from '../lib/splash';

describe('splash status line', () => {
  it('maps the native phases to real status text, never a percentage', () => {
    expect(splashStatusText('starting')).toBe('Starting');
    expect(splashStatusText('looking')).toBe('Looking for your Kivori');
    for (const text of [strings.splash.starting, strings.splash.looking]) {
      expect(text).not.toMatch(/%|\d/);
    }
  });

  it('reads an unknown or missing phase as the first phase', () => {
    expect(splashStatusText(null)).toBe(strings.splash.starting);
    expect(splashStatusText('bogus')).toBe(strings.splash.starting);
  });
});

describe('splash page', () => {
  beforeEach(() => {
    vi.resetModules();
    document.body.innerHTML =
      '<main id="splash"><img id="buddy" /><p id="version"></p><p id="status"></p></main>';
    window.history.replaceState({}, '', '/splash.html?v=1.2.3&phase=looking');
  });
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows the buddy, the version and the phase from its URL', async () => {
    await import('../splash');
    expect(document.getElementById('version')?.textContent).toBe('Version 1.2.3');
    expect(document.getElementById('status')?.textContent).toBe('Looking for your Kivori');
    expect(document.getElementById('buddy')?.getAttribute('src')).toBeTruthy();
  });
});
