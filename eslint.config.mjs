// Flat ESLint config (ESLint 9+). Strict TypeScript linting for the frontend workspaces.
import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    ignores: [
      '**/dist/**',
      '**/node_modules/**',
      '**/target/**',
      '**/*.d.ts',
      '.claude/**',
      '**/.next/**',
      '**/.open-next/**',
      '**/.wrangler/**',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    rules: {
      // TypeScript's checker already flags undefined identifiers; `no-undef` only creates false
      // positives for DOM/vitest globals in a TS codebase (typescript-eslint's own recommendation).
      'no-undef': 'off',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/explicit-module-boundary-types': 'warn',
    },
  },
);
