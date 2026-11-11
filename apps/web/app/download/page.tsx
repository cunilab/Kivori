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
import { ReleaseNotes } from '@/components/release-notes';
import { Eyebrow, Section } from '@/components/section';
import { downloadState } from '@/lib/download-page';
import { formatBytes, formatDate } from '@/lib/format';
import { softwareApplicationJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { loadReleases } from '@/lib/releases';
import { notifyHref } from '@/lib/product-labels';

// Rendered per request; release data is cached for 600 s by lib/github-cache.ts.
export const dynamic = 'force-dynamic';

export const metadata: Metadata = pageMetadata({
  title: 'Download',
  description:
    'Download Kivori Desktop for Windows, Apple Silicon macOS and Linux, with the version history.',
  path: '/download',
});

const REQUIREMENTS = [
  'Windows 10 or 11, 64-bit',
  'A free USB-C port',
  'A supported Kivori device',
] as const;

const pill = 'site-button h-11 px-6 text-base';

export default async function DownloadPage(): Promise<ReactElement> {
  const result = await loadReleases();
  const state = downloadState(result);
  const releases = result.status === 'ok' ? result.releases : [];
  const ready = state.kind === 'ready' ? state : null;
  const stable = releases.find((release) => !release.prerelease && !release.version.includes('-'));
  const stale = result.status === 'ok' && result.stale;

  return (
    <>
      <JsonLd
        data={softwareApplicationJsonLd(
          stable?.version,
          undefined,
          Boolean(stable?.macInstaller),
          Boolean(stable?.linuxDeb || stable?.linuxAppImage),
        )}
      />
      <section aria-labelledby="download-title" className="px-5 pt-14 pb-2 sm:px-8 sm:pt-20">
        <div className="mx-auto max-w-[1180px]">
          <Eyebrow>Kivori Desktop</Eyebrow>
          <h1 id="download-title" className="display-1 mt-3">
            Download Kivori.
          </h1>
          <p className="lead mt-6 max-w-xl">
            One desktop app for Kivori devices, including KV-P1. It runs offline and keeps your
            device up to date.
          </p>
          <div className="mt-6 text-sm text-muted-foreground">
            {state.kind === 'unavailable' ? (
              <p>Downloads are temporarily unavailable. Please try again shortly.</p>
            ) : null}
            {state.kind === 'empty' ? (
              <p>
                First release coming soon.{' '}
                <a
                  href={notifyHref('kv-p1')}
                  className="font-semibold text-teal underline underline-offset-4"
                >
                  Get updates
                </a>
              </p>
            ) : null}
            {stale ? (
              <p role="status">
                Release information may be out of date. Please check again shortly.
              </p>
            ) : null}
            {result.status === 'ok' && result.fetchedAt ? (
              <p className="mt-2">
                Last checked{' '}
                <time dateTime={new Date(result.fetchedAt).toISOString()}>
                  {new Date(result.fetchedAt).toLocaleString('en-GB', {
                    timeZone: 'UTC',
                    dateStyle: 'medium',
                    timeStyle: 'short',
                  })}{' '}
                  UTC
                </time>
              </p>
            ) : null}
          </div>
        </div>
      </section>

      <Section
        id="platforms"

        title="Choose your platform."
        className="scroll-mt-14"
      >
        <div className="grid gap-6 md:grid-cols-2">
          <Card className="rounded-lg p-2 shadow-none ring-1 ring-border">
            <CardHeader className="p-6 sm:p-8">
              <span className="flex size-12 items-center justify-center rounded-md bg-accent text-accent-foreground">
                <MonitorIcon className="size-6" aria-hidden="true" />
              </span>
              <CardTitle className="mt-4 font-display text-3xl font-semibold tracking-tight">
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
                  : state.kind === 'unavailable'
                    ? 'Release information temporarily unavailable'
                    : stable
                      ? 'Windows build not published for this release'
                      : 'First stable release coming soon'}
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
                <Button size="lg" className={pill} disabled>
                  Download for Windows
                </Button>
              )}
            </CardContent>
          </Card>

          <Card className="rounded-lg p-2 shadow-none ring-1 ring-border">
            <CardHeader className="p-6 sm:p-8">
              <span className="flex size-12 items-center justify-center rounded-md bg-secondary text-muted-foreground">
                <AppleIcon className="size-6" aria-hidden="true" />
              </span>
              <CardTitle className="mt-4 font-display text-3xl font-semibold tracking-tight">
                macOS
              </CardTitle>
              <CardDescription className="text-base">
                {stable?.macInstaller
                  ? `Version ${stable.version} · Apple Silicon`
                  : state.kind === 'unavailable'
                    ? 'Release information temporarily unavailable'
                    : 'Mac build not published for the latest stable release'}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-6 px-6 pb-6 sm:px-8 sm:pb-8">
              <p className="text-base text-muted-foreground">
                Native build for Apple Silicon Macs. Per-app volume and automatic app updates are
                not available on macOS.
              </p>
              <div className="flex flex-wrap gap-3">
                {stable?.macInstaller ? (
                  <Button
                    size="lg"
                    className={pill}
                    nativeButton={false}
                    render={<a href="/download/macos/latest" />}
                  >
                    Download for macOS
                  </Button>
                ) : (
                  <Button size="lg" className={pill} disabled>
                    Download for macOS
                  </Button>
                )}
              </div>
            </CardContent>
          </Card>
          {stable?.linuxDeb || stable?.linuxAppImage ? (
            <Card className="rounded-lg p-2 shadow-none ring-1 ring-border">
              <CardHeader className="p-6 sm:p-8">
                <CardTitle className="font-display text-3xl font-semibold tracking-tight">
                  Linux
                </CardTitle>
                <CardDescription>Version {stable.version} · x86_64</CardDescription>
              </CardHeader>
              <CardContent className="space-y-6 px-6 pb-6 sm:px-8 sm:pb-8">
                <p className="text-base text-muted-foreground">
                  Ubuntu / Debian. Volume and media controls work on X11 and Wayland; shortcuts and
                  automatic app profiles require X11. AppImage updates automatically when signed;
                  update Debian packages through a newer download.
                </p>
                <p className="text-sm text-muted-foreground">
                  The Debian package installs USB access rules. AppImage users must install the
                  Kivori udev rule separately. AppImage may need libfuse2 (libfuse2t64 on Ubuntu
                  24.04+) or --appimage-extract-and-run.
                </p>
                <a href="/support#linux" className="text-primary underline">
                  Linux setup and USB access
                </a>
                <div className="flex flex-wrap gap-3">
                  {stable.linuxDeb ? (
                    <Button
                      className={pill}
                      nativeButton={false}
                      render={<a href="/download/linux/latest" />}
                    >
                      Download .deb
                    </Button>
                  ) : null}
                  {stable.linuxAppImage ? (
                    <Button
                      className={pill}
                      nativeButton={false}
                      render={<a href="/download/linux/latest?format=appimage" />}
                    >
                      Download AppImage
                    </Button>
                  ) : null}
                </div>
              </CardContent>
            </Card>
          ) : null}
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
          <div className="overflow-x-auto rounded-md bg-card ring-1 ring-border">
            <Table className="min-w-[34rem]">
              <TableHeader>
                <TableRow>
                  <TableHead className="h-12 px-6">Desktop app</TableHead>
                  <TableHead className="h-12">Date</TableHead>
                  <TableHead className="h-12">Bundled firmware</TableHead>
                  <TableHead className="h-12">Status</TableHead>
                  <TableHead className="h-12 pr-6 text-right">Downloads</TableHead>
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
                      <TableCell className="text-muted-foreground">
                        {release.firmware.length
                          ? release.firmware.map((firmware) => (
                              <div key={firmware.product}>
                                {firmware.model} {firmware.version}
                              </div>
                            ))
                          : 'Details unavailable'}
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
                        {release.linuxDeb ? (
                          <a
                            className="ml-3 font-medium text-primary hover:underline"
                            href={`/download/linux/${release.version}`}
                          >
                            Linux .deb
                          </a>
                        ) : null}
                        {release.linuxAppImage ? (
                          <a
                            className="ml-3 font-medium text-primary hover:underline"
                            href={`/download/linux/${release.version}?format=appimage`}
                          >
                            AppImage
                          </a>
                        ) : null}
                      </TableCell>
                    </TableRow>
                    <TableRow className="hover:bg-transparent">
                      <TableCell colSpan={5} className="px-6 pt-0 pb-2 whitespace-normal">
                        <Accordion>
                          <AccordionItem value="notes" className="border-0">
                            <AccordionTrigger className="w-fit flex-none gap-2 py-1 text-sm font-medium text-muted-foreground hover:text-foreground hover:no-underline">
                              What&apos;s new in {release.version}
                            </AccordionTrigger>
                            <AccordionContent>
                              <div className="pt-2">
                                <ReleaseNotes version={release.version} notes={release.notes} />
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
