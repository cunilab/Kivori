import { defineCloudflareConfig } from '@opennextjs/cloudflare';

// W1 has no incremental cache; the R2 cache arrives with the download slice (W3).
export default defineCloudflareConfig();
