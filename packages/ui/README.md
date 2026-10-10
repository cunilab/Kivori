# @kivori/ui

Shared shadcn/ui setup (style `base-nova`, built on Base UI) for the desktop app and the website.
It follows shadcn's monorepo layout: components, hooks, `cn` and the theme live here; each app has
its own `components.json` whose `ui` alias points at this package. The package ships TypeScript
source only (no build step) and expects React 19.

## Layout

- `src/components/*.tsx` - shadcn primitives (add `'use client'` to anything interactive)
- `src/lib/utils.ts` - `cn`
- `src/hooks/*.ts` - shared hooks
- `src/styles/globals.css` - Tailwind imports, theme tokens, `@theme inline`, base layer

## Using it from an app

```css
/* app stylesheet */
@import '@kivori/ui/globals.css';
@source '../../../packages/ui/src'; /* adjust the relative path */
```

```tsx
import { Button } from '@kivori/ui/components/button';
import { cn } from '@kivori/ui/lib/utils';
```

- Desktop (Vite): `resolve.dedupe: ['react', 'react-dom']`.
- Web (Next): `transpilePackages: ['@kivori/ui']`.
- App-only components stay in the app. `Toaster` takes `theme` as a prop; the package has no theme state.

## Adding a component

Run the shadcn CLI from an app, so its aliases route the file into this package:

```sh
cd apps/web && bunx shadcn add <name>
```

Then check the result: add `'use client'` when it uses state, effects or Base UI primitives; import
`cn` from `../lib/utils` and sibling components relatively; give exported functions explicit return
types (lint runs with `--max-warnings 0`); run `bun run format:write`.
