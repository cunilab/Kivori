import { fileURLToPath, URL } from 'node:url';
import { readFileSync } from 'node:fs';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [
    {
      // `.md` imports resolve to their text, like the loader in next.config.ts.
      name: 'md-as-text',
      transform(_code, id) {
        if (!id.endsWith('.md')) return null;
        return { code: `export default ${JSON.stringify(readFileSync(id, 'utf8'))};`, map: null };
      },
    },
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./', import.meta.url)),
      '@brand': fileURLToPath(new URL('../../assets', import.meta.url)),
    },
  },
  test: {
    environment: 'node',
    globals: true,
    include: ['**/*.test.ts'],
    exclude: ['node_modules/**', '.next/**', '.open-next/**'],
  },
});
