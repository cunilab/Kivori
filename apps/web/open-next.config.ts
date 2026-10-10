import { defineCloudflareConfig } from '@opennextjs/cloudflare';
import staticAssetsIncrementalCache from '@opennextjs/cloudflare/overrides/incremental-cache/static-assets-incremental-cache';

// Read-only cache backed by Workers static assets: serves the prerendered /products/[slug] pages
// (SSG pages are not served without an incremental cache). The R2 cache arrives with the download
// slice (W3) and replaces this once pages need revalidation.
export default defineCloudflareConfig({ incrementalCache: staticAssetsIncrementalCache });
