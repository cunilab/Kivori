import { getCloudflareContext } from '@opennextjs/cloudflare';

/** The slice of the Analytics Engine binding we use. */
interface DownloadsDataset {
  writeDataPoint(point: { blobs: string[]; doubles: number[]; indexes: string[] }): void;
}

export interface DownloadEvent {
  os: string;
  version: string;
  /** The Referer header (a full URL); only its host is kept. */
  referrer: string | null;
  utmSource: string | null;
}

export function referrerHost(referrer: string | null): string {
  if (!referrer) return '';
  try {
    return new URL(referrer).host;
  } catch {
    return '';
  }
}

/** Blobs `[os, version, referrer host, utm_source]`, double 1, index `os`. No IP, no user agent. */
export function downloadDataPoint(event: DownloadEvent): {
  blobs: string[];
  doubles: number[];
  indexes: string[];
} {
  return {
    blobs: [
      event.os,
      event.version,
      referrerHost(event.referrer),
      (event.utmSource ?? '').slice(0, 64),
    ],
    doubles: [1],
    indexes: [event.os],
  };
}

/** Writes one download data point; a no-op when the `DOWNLOADS` binding is absent (dev, tests). */
export function recordDownload(event: DownloadEvent, dataset?: DownloadsDataset): void {
  try {
    const target =
      dataset ??
      (getCloudflareContext().env as unknown as { DOWNLOADS?: DownloadsDataset }).DOWNLOADS;
    target?.writeDataPoint(downloadDataPoint(event));
  } catch {
    // counting must never break the redirect
  }
}
