import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { BetaBadge } from '@/components/beta-badge';
import { formatDate } from '@/lib/format';
import { pageMetadata } from '@/lib/metadata';
import { loadReleases } from '@/lib/releases';

// Rendered per request; release data is cached for 600 s by lib/github-cache.ts.
export const dynamic = 'force-dynamic';

export const metadata: Metadata = pageMetadata({
  title: 'Changelog',
  description: 'Every Kivori desktop version and when it came out.',
  path: '/changelog',
});

export default async function ChangelogPage(): Promise<ReactElement> {
  const result = await loadReleases();

  return (
    <div className="px-5 py-14 sm:py-20">
      <div className="mx-auto max-w-3xl">
        <h1 className="font-display text-4xl font-semibold sm:text-5xl">Changelog</h1>

        {result.status === 'error' ? (
          <p className="mt-8 text-muted-foreground">
            We could not load the version history right now. Please try again shortly.
          </p>
        ) : result.releases.length === 0 ? (
          <p className="mt-8 text-muted-foreground">No releases yet.</p>
        ) : (
          <ol className="mt-10 space-y-12">
            {result.releases.map((release) => (
              <li key={release.tag} id={`v${release.version}`} className="scroll-mt-20">
                <div className="flex flex-wrap items-center gap-3">
                  <h2 className="font-display text-2xl font-semibold">
                    <a href={`#v${release.version}`}>Version {release.version}</a>
                  </h2>
                  {release.prerelease ? <BetaBadge /> : null}
                  <span className="text-sm text-muted-foreground">
                    · {formatDate(release.date)}
                  </span>
                </div>
              </li>
            ))}
          </ol>
        )}
      </div>
    </div>
  );
}
