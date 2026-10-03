import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import './index.css';
import { brandIconUrl } from './lib/brand';
import { applyTheme } from './lib/theme';

// Set from the bundled asset URL so the tab icon also works under `vite dev` (a static href that
// points outside the app folder is not served there).
document.getElementById('favicon')?.setAttribute('href', brandIconUrl);

applyTheme();

const container = document.getElementById('root');
if (container) {
  createRoot(container).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
