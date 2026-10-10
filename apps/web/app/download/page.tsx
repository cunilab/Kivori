import { CheckIcon, InfoIcon, MonitorIcon, AppleIcon } from 'lucide-react';
import type { Metadata } from 'next';
import { Fragment, type ReactElement } from 'react';
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@kivori/ui/components/accordion';
import { Badge } from '@kivori/ui/components/badge';
import { Button } from '@kivori/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@kivori/ui/components/table';
import { JsonLd } from '@/components/json-ld';
import { OsDownloadButton } from '@/components/os-download-button';
import { ReleaseNotes } from '@/components/release-notes';
import { Eyebrow, Section } from '@/components/section';
import { downloadState } from '@/lib/download-page';
import { formatBytes, formatDate } from '@/lib/format';
import { softwareApplicationJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { notifyHref } from '@/lib/product-labels';
import { loadReleases } from '@/lib/releases';

// Rendered per request; release data is cached for 600 s by lib/github-cache.ts.
export const dynamic = 'force-dynamic';

export const metadata: Metadata = pageMetadata({
  title: 'Download',
  description: 'Download the Kivori desktop app for Windows 10 and 11, with the version history.',
  path: '/download',
});

const REQUIREMENTS = ['Windows 10 or 11, 64-bit', 'A free USB-C port', 'Your Kivori'] as const;

const pill = 'keycap h-11 px-6 text-base';

export default async function DownloadPage(): Promise<ReactElement> {
  const result = await loadReleases();
  const state = downloadState(result);
  const releases = result.status === 'ok' ? result.releases : [];
  const ready = state.kind === 'ready' ? state : null;

  return (
    <>
      <JsonLd data={softwareApplicationJsonLd(ready?.release.version)} />
      <section
        aria-labelledby="download-title"
        className="px-5 pt-20 pb-24 text-center sm:px-8 sm:pt-28 sm:pb-32"
      >
        <div className="mx-auto max-w-3xl">
          <Eyebrow>Kivori Desktop</Eyebrow>
          <h1 id="download-title" className="display-1 mt-3">
            Download Kivori.
          </h1>
          <p className="lead mx-auto mt-6 max-w-xl">
            The app that pairs with your Kivori. It runs offline and keeps your device up to date.
          </p>
          <div className="mt-12">
            {ready ? (
              <OsDownloadButton version={ready.release.version} beta={ready.release.prerelease} />
            ) : state.kind === 'empty' ? (
              <div className="flex flex-col items-center gap-4">
                <p className="font-display text-2xl font-bold tracking-tight">
                  First release coming soon
                </p>
                <Button
                  size="lg"
                  className="keycap h-14 px-9 text-lg"
                  nativeButton={false}
                  render={<a href={notifyHref('kivori')} />}
                >
                  Join the waitlist
                </Button>
                <a href="#platforms" className="text-sm font-medium text-primary hover:underline">
                  Other platforms &rsaquo;
                </a>
              </div>
            ) : (
              <p className="text-lg text-muted-foreground">
                Downloads are temporarily unavailable. Please try again shortly.
              </p>
            )}
          </div>
        </div>
      </section>

      <Section
        id="platforms"
        tone="surface"
        eyebrow="Platforms"
        title="Pick your computer."
        className="scroll-mt-14"
      >
        <div className="grid gap-6 md:grid-cols-2">
          <Card className="rounded-2xl p-2 shadow-none ring-1 ring-border">
            <CardHeader className="p-6 sm:p-8">
              <span className="flex size-12 items-center justify-center rounded-2xl bg-accent text-accent-foreground">
                <MonitorIcon className="size-6" aria-hidden="true" />
              </span>
              <CardTitle className="mt-4 font-display text-3xl font-bold tracking-tight">
                Windows
              </CardTitle>
              <CardDescription className="text-base">
                {ready
                  ? [
                      `Version ${ready.release.version}`,
                      ready.release.date ? formatDate(ready.release.date) : null,
                      `about ${formatBytes(ready.installer.size)}`,
                    ]
                      .filter(Boolean)
                      .join(' · ')
                  : 'First release coming soon'}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-6 px-6 pb-6 sm:px-8 sm:pb-8">
              <ul className="space-y-2.5">
                {REQUIREMENTS.map((item) => (
                  <li key={item} className="flex items-center gap-2.5 text-base">
                    <CheckIcon className="size-4 text-primary" aria-hidden="true" />
                    {item}
                  </li>
                ))}
              </ul>
              <p className="flex gap-2.5 text-sm text-muted-foreground">
                <InfoIcon className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
                If Windows shows a protection notice, choose More info → Run anyway.
              </p>
              {ready ? (
                <Button
                  size="lg"
                  className={pill}
                  nativeButton={false}
                  render={<a href="/download/windows/latest" />}
                >
                  Download for Windows
                </Button>
              ) : (
                <Button
                  size="lg"
                  className={pill}
                  nativeButton={false}
                  render={<a href={notifyHref('kivori')} />}
                >
                  Join the waitlist
                </Button>
              )}
            </CardContent>
          </Card>

          <Card className="rounded-2xl p-2 shadow-none ring-1 ring-border">
            <CardHeader className="p-6 sm:p-8">
              <span className="flex size-12 items-center justify-center rounded-2xl bg-secondary text-muted-foreground">
                <AppleIcon className="size-6" aria-hidden="true" />
              </span>
              <CardTitle className="mt-4 font-display text-3xl font-bold tracking-tight">
                macOS
              </CardTitle>
              <CardDescription className="text-base">Coming later</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6 px-6 pb-6 sm:px-8 sm:pb-8">
              <p className="text-base text-muted-foreground">
                A Mac version comes after the Windows beta. Leave your email and we will tell you
                the day it is ready.
              </p>
              <div className="flex flex-wrap gap-3">
                <Button size="lg" className={pill} disabled>
                  Download for macOS
                </Button>
                <Button
                  size="lg"
                  variant="outline"
                  className={pill}
                  nativeButton={false}
                  render={<a href={notifyHref('kivori')} />}
                >
                  Join the waitlist
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </Section>

      <Section id="history" eyebrow="Version history" title="Every release.">
        {releases.length === 0 ? (
          <p className="text-lg text-muted-foreground">
            {state.kind === 'unavailable'
              ? 'The version history could not be loaded right now.'
              : 'No releases yet. The first one is coming soon.'}
          </p>
        ) : (
          <div className="overflow-x-auto rounded-2xl bg-card ring-1 ring-border">
            <Table className="min-w-[34rem]">
              <TableHeader>
                <TableRow>
                  <TableHead className="h-12 px-6">Version</TableHead>
                  <TableHead className="h-12">Date</TableHead>
                  <TableHead className="h-12">Status</TableHead>
                  <TableHead className="h-12 pr-6 text-right">Windows</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {releases.map((release) => (
                  <Fragment key={release.tag}>
                    <TableRow className="border-b-0 hover:bg-transparent">
                      <TableCell className="px-6 py-4 text-base font-semibold">
                        {release.version}
                      </TableCell>
                      <TableCell className="text-muted-foreground">
                        {formatDate(release.date)}
                      </TableCell>
                      <TableCell>
                        {release.prerelease ? <Badge variant="secondary">Beta</Badge> : null}
                      </TableCell>
                      <TableCell className="pr-6 text-right">
                        {release.installer ? (
                          <a
                            href={`/download/windows/${release.version}`}
                            className="font-medium text-primary hover:underline"
                          >
                            Download
                          </a>
                        ) : (
                          <span className="text-muted-foreground">Not available</span>
                        )}
                      </TableCell>
                    </TableRow>
                    <TableRow className="hover:bg-transparent">
                      <TableCell colSpan={4} className="px-6 pt-0 pb-2 whitespace-normal">
                        <Accordion>
                          <AccordionItem value="notes" className="border-0">
                            <AccordionTrigger className="w-fit flex-none gap-2 py-1 text-sm font-medium text-muted-foreground hover:text-foreground hover:no-underline">
                              What&apos;s new in {release.version}
                            </AccordionTrigger>
                            <AccordionContent>
                              <div className="pt-2">
                                <ReleaseNotes version={release.version} />
                              </div>
                            </AccordionContent>
                          </AccordionItem>
                        </Accordion>
                      </TableCell>
                    </TableRow>
                  </Fragment>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </Section>
    </>
  );
}
