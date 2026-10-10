import { initOpenNextCloudflareForDev } from '@opennextjs/cloudflare';
import type { NextConfig } from 'next';
import { fileURLToPath } from 'node:url';

// The repo root: brand art lives in <root>/assets and is imported through `@brand` (never copied).
const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url));

const nextConfig: NextConfig = {
  // Let the bundler and the file tracer read the shared brand assets outside apps/web.
  turbopack: { root: REPO_ROOT },
  outputFileTracingRoot: REPO_ROOT,
  // @kivori/ui ships TypeScript source (no build step); Next compiles it with the app.
  transpilePackages: ['@kivori/ui'],
  // Images are pre-sized in the repo; Cloudflare image resizing is a paid add-on.
  images: { unoptimized: true },
};

initOpenNextCloudflareForDev();

export default nextConfig;
