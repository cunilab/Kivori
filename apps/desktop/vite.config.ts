import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath, URL } from 'node:url';
import { searchForWorkspaceRoot } from 'vite';
import { defineConfig } from 'vitest/config';

const BRAND_ASSETS = fileURLToPath(new URL('../../assets', import.meta.url));

// Vite + Vitest config for the Kivori desktop frontend (Device Studio, US4). React plugin for JSX;
// Tailwind v4 provides the shadcn utility layer; jsdom powers component/canvas/store tests.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      // The brand art (mascot, app icon) lives once at the repo root, shared with the firmware asset
      // compiler and the native icons. Import it with `?url`; never copy or redraw it.
      '@brand': BRAND_ASSETS,
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    fs: { allow: [searchForWorkspaceRoot(process.cwd()), BRAND_ASSETS] },
  },
  build: { outDir: 'dist', emptyOutDir: true },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
  },
});
