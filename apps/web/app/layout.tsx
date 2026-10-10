import type { Metadata, Viewport } from 'next';
import { GeistSans } from 'geist/font/sans';
import type { ReactElement, ReactNode } from 'react';
import brandIcon from '@brand/icon/kivori-icon.svg';
import { SiteFooter } from '@/components/site-footer';
import { JsonLd } from '@/components/json-ld';
import { SiteHeader } from '@/components/site-header';
import { organizationJsonLd } from '@/lib/jsonld';
import { DESCRIPTION, ogImagePath, resolveSiteUrl, SITE_NAME } from '@/lib/site';
import { THEME_INIT_SCRIPT } from './theme-script';
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

export const viewport: Viewport = {
  themeColor: '#fbfbfd',
};

export default function RootLayout({ children }: { children: ReactNode }): ReactElement {
  return (
    // Server default is light; the inline script switches to dark for stored/OS-dark before paint.
    // Geist Sans is self-hosted by the `geist` package (SIL OFL), so there are no runtime font requests.
    <html
      lang="en"
      className={GeistSans.variable}
      style={{ colorScheme: 'light' }}
      suppressHydrationWarning
    >
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
      </head>
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
