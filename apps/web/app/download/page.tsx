import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { BetaBadge } from '@/components/beta-badge';
import { JsonLd } from '@/components/json-ld';
import { formatBytes, formatDate } from '@/lib/format';
import { softwareApplicationJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { loadReleases, type Release } from '@/lib/releases';
import { BTN_PRIMARY } from '@/lib/ui';

// Rendered per request; release data is cached for 600 s by lib/github-cache.ts.
export const dynamic = 'force-dynamic';

export const metadata: Metadata = pageMetadata({
  title: 'Download',
  description: 'Download the Kivori desktop app for Windows 10 and 11, with the version history.',
  path: '/download',
});

function Card({ children }: { children: React.ReactNode }): ReactElement {
  return <div className="rounded-2xl border border-border bg-card p-6 sm:p-8">{children}</div>;
}

function LatestCard({ release }: { release: Release }): ReactElement {
  const { installer } = release;
  return (
    <Card>
      <div className="flex flex-wrap items-center gap-3">
        <h2 className="font-display text-2xl font-semibold">Kivori {release.version}</h2>
        {release.prerelease ? <BetaBadge /> : null}
        {release.date ? (
          <span className="text-sm text-muted-foreground">{formatDate(release.date)}</span>
        ) : null}
      </div>

      {installer ? (
        <div className="mt-6 space-y-5">
          <div className="flex flex-wrap items-center gap-4">
            <a href={`/download/windows/${release.version}`} className={BTN_PRIMARY}>
              Download for Windows
            </a>
            <span className="text-sm text-muted-foreground">
              Windows 10 and 11 · {formatBytes(installer.size)}
            </span>
          </div>
        </div>
      ) : (
        <div className="mt-6">
          <p className="font-display text-xl font-semibold">First build coming soon</p>
          <p className="mt-2 text-muted-foreground">
            This release has no installer yet. Join the waitlist and we will tell you when the first
            build is ready.
          </p>
          <a href="/#waitlist" className={`${BTN_PRIMARY} mt-5`}>
            Join the waitlist
          </a>
        </div>
      )}
      <p className="mt-6 text-sm">
        See what is new in the{' '}
        <a href="/changelog" className="text-primary underline">
          changelog
        </a>
        .
      </p>
    </Card>
  );
}

function Fallback(): ReactElement {
  return (
    <Card>
      <p className="font-display text-xl font-semibold">
        Downloads are temporarily unavailable, please try again shortly
      </p>
    </Card>
  );
}

export default async function DownloadPage(): Promise<ReactElement> {
  const result = await loadReleases();
  const releases = result.status === 'ok' ? result.releases : [];
  const latest = releases[0];
  const older = releases.slice(1);

  return (
    <div className="px-5 py-14 sm:py-20">
      <JsonLd data={softwareApplicationJsonLd(latest?.version)} />
      <div className="mx-auto max-w-3xl">
        <h1 className="font-display text-4xl font-semibold sm:text-5xl">Download Kivori</h1>
        <p className="mt-4 text-lg text-muted-foreground">
          The desktop app pairs with your Kivori and runs offline.
        </p>

        <div className="mt-10">
          {result.status === 'error' ? (
            <Fallback />
          ) : latest ? (
            <LatestCard release={latest} />
          ) : (
            <Card>
              <p className="font-display text-xl font-semibold">First build coming soon</p>
              <a href="/#waitlist" className={`${BTN_PRIMARY} mt-5`}>
                Join the waitlist
              </a>
            </Card>
          )}
        </div>

        <h2 className="mt-12 font-display text-2xl font-semibold">System requirements</h2>
        <ul className="mt-4 list-disc space-y-1 pl-5 text-muted-foreground">
          <li>Windows 10 or 11, 64-bit (x64)</li>
          <li>A Kivori device and a USB-C data cable</li>
          <li>macOS: coming later</li>
        </ul>

        <h2 className="mt-12 font-display text-2xl font-semibold">Installing</h2>
        <p className="mt-4 text-muted-foreground">
          The installer runs for your user only and does not ask for administrator rights. This beta
          installer is not signed yet, so Windows SmartScreen may show &quot;Windows protected your
          PC&quot;. This is expected for an unsigned app. If you got the installer from us, choose{' '}
          <strong className="text-foreground">More info</strong>, then{' '}
          <strong className="text-foreground">Run anyway</strong>. A signed installer is coming, and
          the SmartScreen step will go away with it.
        </p>
        <p className="mt-4 text-muted-foreground">
          Kivori keeps your device up to date from the app, so there is nothing else to download.
        </p>
        {older.length > 0 ? (
          <>
            <h2 className="mt-12 font-display text-2xl font-semibold">Older versions</h2>
            <ul className="mt-4 divide-y divide-border rounded-xl border border-border">
              {older.map((release) => (
                <li
                  key={release.tag}
                  className="flex flex-wrap items-center justify-between gap-2 px-4 py-3 text-sm"
                >
                  <span className="flex items-center gap-2">
                    <a href={`/changelog#v${release.version}`} className="font-medium underline">
                      {release.version}
                    </a>
                    {release.prerelease ? <BetaBadge /> : null}
                  </span>
                  <span className="flex items-center gap-4 text-muted-foreground">
                    {formatDate(release.date)}
                    {release.installer ? (
                      <a
                        href={`/download/windows/${release.version}`}
                        className="text-primary underline"
                      >
                        Download
                      </a>
                    ) : (
                      <span>No installer</span>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          </>
        ) : null}
      </div>
    </div>
  );
}
