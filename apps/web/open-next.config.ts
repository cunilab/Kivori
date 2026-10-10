import { defineCloudflareConfig } from '@opennextjs/cloudflare';
import staticAssetsIncrementalCache from '@opennextjs/cloudflare/overrides/incremental-cache/static-assets-incremental-cache';

// Read-only cache backed by Workers static assets: serves the prerendered /products/[slug] pages
// (SSG pages are not served without an incremental cache). /download and /changelog are dynamic and
// cache their GitHub calls with the Workers Cache API (lib/github-cache.ts), so no R2 is needed.
export default defineCloudflareConfig({ incrementalCache: staticAssetsIncrementalCache });
