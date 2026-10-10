import type { Metadata, Viewport } from 'next';
import localFont from 'next/font/local';
import type { ReactElement, ReactNode } from 'react';
import brandIcon from '@brand/icon/kivori-icon.svg';
import { SiteFooter } from '@/components/site-footer';
import { SiteHeader } from '@/components/site-header';
import { DESCRIPTION, resolveSiteUrl, SITE_NAME } from '@/lib/site';
import { THEME_INIT_SCRIPT } from './theme-script';
import './globals.css';

// Self-hosted at build time (Space Grotesk, SIL OFL; see app/fonts/OFL.txt). No runtime font requests.
const display = localFont({
  src: './fonts/space-grotesk-latin-wght-normal.woff2',
  variable: '--font-display-face',
  weight: '300 700',
  display: 'swap',
});

export const metadata: Metadata = {
  metadataBase: resolveSiteUrl(process.env.SITE_URL),
  title: { default: SITE_NAME, template: `%s · ${SITE_NAME}` },
  description: DESCRIPTION,
  // The favicon is the brand SVG itself, imported through @brand (hashed into /_next/static; never copied).
  // An `app/icon.tsx` ImageResponse was tried and dropped: its static prerender is not served by the
  // Worker without an incremental cache (arrives in W3), and it adds the resvg wasm to the bundle.
  icons: { icon: { url: brandIcon.src, type: 'image/svg+xml' } },
  openGraph: { siteName: SITE_NAME, type: 'website' },
};

export const viewport: Viewport = {
  themeColor: '#0c101c',
};

export default function RootLayout({ children }: { children: ReactNode }): ReactElement {
  return (
    // Server default is dark; the inline script switches to light for stored/OS-light before paint.
    <html
      lang="en"
      className={`${display.variable} dark`}
      style={{ colorScheme: 'dark' }}
      suppressHydrationWarning
    >
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
      </head>
      <body className="flex min-h-screen flex-col">
        <SiteHeader />
        <main className="flex-1">{children}</main>
        <SiteFooter />
      </body>
    </html>
  );
}
