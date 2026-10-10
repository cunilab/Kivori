import { recordDownload } from '@/lib/analytics';
import { loadReleases, type Release } from '@/lib/releases';

// Server-only. Streams installers through the Worker so the browser never sees the release host.

export const SUPPORTED_OS = ['windows'] as const;
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

export function isSupportedOs(os: string): boolean {
  return (SUPPORTED_OS as readonly string[]).includes(os);
}

export function isValidVersion(version: string): boolean {
  return version === 'latest' || SEMVER.test(version);
}

function internalRedirect(requestUrl: string): Response {
  return Response.redirect(new URL('/download', requestUrl).href, 302);
}

function startsAtZero(range: string | null): boolean {
  return !range || /^bytes=0-/.test(range);
}

export interface DownloadDeps {
  load?: typeof loadReleases;
  fetcher?: typeof fetch;
  record?: typeof recordDownload;
}

/**
 * Resolves `/download/<os>/<version>` to a streamed installer. 404 for an unknown OS or malformed
 * version; a 302 to `/download` when there is no asset. The response never carries an outside `location`.
 */
export async function downloadResponse(
  request: Request,
  os: string,
  version: string,
  { load = loadReleases, fetcher = fetch, record = recordDownload }: DownloadDeps = {},
): Promise<Response> {
  if (!isSupportedOs(os) || !isValidVersion(version)) {
    return new Response('Not found', { status: 404 });
  }

  const result = await load();
  const releases: Release[] = result.status === 'ok' ? result.releases : [];
  const release =
    version === 'latest'
      ? releases[0]
      : releases.find((candidate) => candidate.version === version);
  if (!release?.installer) return internalRedirect(request.url);

  const range = request.headers.get('range');
  let upstream: Response;
  try {
    upstream = await fetcher(release.installer.url, {
      headers: range ? { range } : {},
      redirect: 'follow',
    });
  } catch {
    return internalRedirect(request.url);
  }
  if ((upstream.status !== 200 && upstream.status !== 206) || !upstream.body) {
    return internalRedirect(request.url);
  }

  if (startsAtZero(range)) {
    const url = new URL(request.url);
    record({
      os: 'windows',
      version: release.version,
      referrer: request.headers.get('referer'),
      utmSource: url.searchParams.get('utm_source'),
    });
  }

  const headers = new Headers({
    'content-type': 'application/octet-stream',
    'content-disposition': `attachment; filename="Kivori-Setup-${release.version}.exe"`,
    'cache-control': 'no-store',
    'accept-ranges': 'bytes',
  });
  const length = upstream.headers.get('content-length');
  if (length) headers.set('content-length', length);
  const contentRange = upstream.headers.get('content-range');
  if (upstream.status === 206 && contentRange) headers.set('content-range', contentRange);

  return new Response(upstream.body, { status: upstream.status, headers });
}
