import { NextResponse, type NextRequest } from 'next/server';
import { recordDownload } from '@/lib/analytics';
import { loadReleases } from '@/lib/releases';

export const dynamic = 'force-dynamic';

/** Stable link for marketing and QR codes: 302 to the newest installer, or to /download without one. */
export async function GET(request: NextRequest): Promise<NextResponse> {
  const result = await loadReleases();
  const latest = result.status === 'ok' ? result.releases[0] : undefined;
  if (!latest?.installer) {
    return NextResponse.redirect(new URL('/download', request.url), 302);
  }
  recordDownload({
    os: 'windows',
    version: latest.version,
    referrer: request.headers.get('referer'),
    utmSource: request.nextUrl.searchParams.get('utm_source'),
  });
  return NextResponse.redirect(latest.installer.url, 302);
}
