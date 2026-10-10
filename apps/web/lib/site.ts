export const DEFAULT_SITE_URL = 'http://localhost:3000';

/**
 * Resolves the public site origin from `SITE_URL` (no path, no trailing slash). Falls back to the
 * local dev origin when unset or not an absolute http(s) URL.
 */
export function resolveSiteUrl(raw: string | undefined): URL {
  const value = raw?.trim();
  if (value) {
    try {
      const url = new URL(value);
      if (url.protocol === 'http:' || url.protocol === 'https:') {
        return new URL(url.origin);
      }
    } catch {
      // fall through to the default
    }
  }
  return new URL(DEFAULT_SITE_URL);
}

export const SITE_NAME = 'Kivori';
export const TAGLINE = 'Control the desktop physically. Understand the desktop visually.';
export const DESCRIPTION =
  'Kivori is a desk buddy you control your computer with: the buddy is why you want one, the physical knob and button are why you keep using it, and the display shows important desktop and system state around the buddy.';
export const GITHUB_URL = 'https://github.com/cunilab/Kivori';

/** Resolves a path (`/products`) against the public site origin into an absolute URL. */
export function absoluteUrl(
  path: string,
  origin: URL = resolveSiteUrl(process.env.SITE_URL),
): string {
  return new URL(path, origin).href;
}

/** Build-time share image path for a slug; `scripts/build-assets.mjs` writes these. */
export function ogImagePath(slug?: string): string {
  return `/generated/og/${slug ?? 'default'}.png`;
}
