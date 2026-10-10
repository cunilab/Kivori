import type { Metadata } from 'next';
import { ogImagePath, SITE_NAME } from '@/lib/site';

interface PageMetadataInput {
  /** Page title without the site suffix; omit for the home page (the layout default applies). */
  title?: string | { absolute: string };
  description: string;
  path: string;
  /** Product slug whose build-time share image to use; omit for the default one. */
  ogSlug?: string;
}

/** Title, description, canonical and OpenGraph/Twitter in one place. URLs resolve via `metadataBase`. */
export function pageMetadata({ title, description, path, ogSlug }: PageMetadataInput): Metadata {
  const plain = typeof title === 'object' ? title.absolute : title;
  const image = {
    url: ogImagePath(ogSlug),
    width: 1200,
    height: 630,
    alt: plain && typeof title === 'string' ? `${plain} · ${SITE_NAME}` : (plain ?? SITE_NAME),
  };
  const shown = typeof title === 'string' ? `${title} · ${SITE_NAME}` : (plain ?? SITE_NAME);
  return {
    ...(title ? { title } : {}),
    description,
    alternates: { canonical: path },
    openGraph: {
      siteName: SITE_NAME,
      type: 'website',
      title: shown,
      description,
      url: path,
      images: [image],
    },
    twitter: { card: 'summary_large_image', title: shown, description, images: [image.url] },
  };
}
