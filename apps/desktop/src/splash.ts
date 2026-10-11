import './splash.css';
import { mascotUrl } from './lib/brand';
import { format, strings } from './lib/i18n/strings';
import { applyTheme } from './lib/theme';
import { splashStatusText } from './lib/splash';

// The startup splash entry (a second, tiny page in the same bundle: no React, no app code). It shows
// the status the native core reports; it never invents progress. Opened by `runtime::splash`.

applyTheme();

const params = new URLSearchParams(window.location.search);
const byId = (id: string): HTMLElement | null => document.getElementById(id);

byId('buddy')?.setAttribute('src', mascotUrl);
const version = byId('version');
if (version)
  version.textContent = format(strings.splash.version, { version: params.get('v') ?? '' });

const status = byId('status');
const setPhase = (phase: string | null): void => {
  if (status) status.textContent = splashStatusText(phase);
};
setPhase(params.get('phase'));

// Events are optional garnish: if the Tauri bridge is missing (a plain browser), the first line stays.
void import('@tauri-apps/api/event')
  .then(async ({ listen }) => {
    await listen<string>('splash-phase', (event) => setPhase(event.payload));
    await listen('splash-close', () => byId('splash')?.classList.add('closing'));
  })
  .catch(() => {});
