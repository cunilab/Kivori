import type { Metadata, Viewport } from 'next';
import localFont from 'next/font/local';
import type { ReactElement, ReactNode } from 'react';
import brandIcon from '@brand/icon/kivori-icon.svg';
import { SiteFooter } from '@/components/site-footer';
import { JsonLd } from '@/components/json-ld';
import { SiteHeader } from '@/components/site-header';
import { organizationJsonLd } from '@/lib/jsonld';
import { DESCRIPTION, ogImagePath, resolveSiteUrl, SITE_NAME } from '@/lib/site';
import './globals.css';

export const metadata: Metadata = {
  metadataBase: resolveSiteUrl(process.env.SITE_URL),
  title: { default: SITE_NAME, template: `%s · ${SITE_NAME}` },
  description: DESCRIPTION,
  // The favicon is the brand SVG itself, imported through @brand (hashed into /_next/static; never copied).
  // An `app/icon.tsx` ImageResponse was tried and dropped: its static prerender is not served by the
  // Worker without an incremental cache (arrives in W3), and it adds the resvg wasm to the bundle.
  icons: { icon: { url: brandIcon.src, type: 'image/svg+xml' } },
  // Pages without their own share image fall back to the build-time default (scripts/build-assets.mjs).
  openGraph: {
    siteName: SITE_NAME,
    type: 'website',
    images: [{ url: ogImagePath(), width: 1200, height: 630, alt: SITE_NAME }],
  },
  twitter: { card: 'summary_large_image', images: [ogImagePath()] },
};

// Fonts are self-hosted from app/fonts (latin subset, SIL OFL; licences alongside), so the build needs no
// network and the browser makes no Google requests. Bricolage Grotesque and Instrument Sans are
// variable fonts: one file covers the weight range.
const display = localFont({
  src: './fonts/bricolage-grotesque-latin-wght.woff2',
  weight: '700 800',
  variable: '--font-bricolage',
  display: 'swap',
  adjustFontFallback: 'Arial',
});
const body = localFont({
  src: './fonts/instrument-sans-latin-wght.woff2',
  weight: '400 600',
  variable: '--font-instrument',
  display: 'swap',
  adjustFontFallback: 'Arial',
});
const pixel = localFont({
  src: [
    { path: './fonts/silkscreen-latin-400.woff2', weight: '400', style: 'normal' },
    { path: './fonts/silkscreen-latin-700.woff2', weight: '700', style: 'normal' },
  ],
  variable: '--font-silkscreen',
  display: 'swap',
  adjustFontFallback: 'Arial',
});

export const viewport: Viewport = {
  themeColor: '#0b0f1a',
};

export default function RootLayout({ children }: { children: ReactNode }): ReactElement {
  return (
    // The web is one committed dark look (see globals.css); the `dark` class keeps shared primitives' dark variants on.
    <html
      lang="en"
      className={`dark ${display.variable} ${body.variable} ${pixel.variable}`}
      style={{ colorScheme: 'dark' }}
    >
      <body className="flex min-h-screen flex-col">
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-[60] focus:rounded-full focus:bg-primary focus:px-4 focus:py-2 focus:text-primary-foreground"
        >
          Skip to content
        </a>
        <SiteHeader />
        <main id="main" className="flex-1">
          {children}
        </main>
        <SiteFooter />
        <JsonLd data={organizationJsonLd()} />
      </body>
    </html>
  );
}
