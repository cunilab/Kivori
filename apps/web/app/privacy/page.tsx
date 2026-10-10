import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { pageMetadata } from '@/lib/metadata';
import { GITHUB_URL } from '@/lib/site';

export const metadata: Metadata = pageMetadata({
  title: 'Privacy',
  description: 'What the Kivori website collects, why, and how to have it deleted.',
  path: '/privacy',
});

export default function PrivacyPage(): ReactElement {
  return (
    <div className="px-5 py-14 sm:py-20">
      <div className="mx-auto max-w-3xl">
        <h1 className="font-display text-4xl font-semibold sm:text-5xl">Privacy</h1>
        <p className="mt-4 text-lg text-muted-foreground">
          This page covers this website. The Kivori desktop app works offline, needs no account and
          sends no telemetry.
        </p>

        <h2 className="mt-10 font-display text-2xl font-semibold">Visiting the site</h2>
        <p className="mt-3 text-muted-foreground">
          We use Cloudflare Web Analytics to count page views. It is cookieless: it sets no cookies,
          does not track you across sites and does not build a profile of you. (It is being added to
          the site; this is how it will work.) Download clicks are counted in Cloudflare Analytics
          Engine as totals by operating system and version, with no personal data.
        </p>

        <h2 className="mt-10 font-display text-2xl font-semibold">The waitlist</h2>
        <p className="mt-3 text-muted-foreground">If you join the waitlist we store:</p>
        <ul className="mt-3 list-disc space-y-1 pl-6 text-muted-foreground">
          <li>your email address</li>
          <li>the product you chose</li>
          <li>your answer to &ldquo;What would you use Kivori for?&rdquo;, if you wrote one</li>
          <li>
            where you came from: the UTM tags in the link you used (source, medium, campaign) and
            the referring page
          </li>
          <li>the time you signed up</li>
        </ul>
        <p className="mt-3 text-muted-foreground">
          We use this only to contact you about Kivori availability. We never sell it or share it
          for advertising. It is stored in Cloudflare D1.
        </p>

        <h2 className="mt-10 font-display text-2xl font-semibold">Your IP address</h2>
        <p className="mt-3 text-muted-foreground">
          Your IP address is not stored. It is used only for a moment, to limit how many sign-ups
          one network can send per minute, and is passed to Cloudflare Turnstile, the bot check on
          the form.
        </p>

        <h2 className="mt-10 font-display text-2xl font-semibold">Deleting your data</h2>
        <p className="mt-3 text-muted-foreground">
          To have your waitlist entry deleted,{' '}
          <a href={`${GITHUB_URL}/issues`} className="text-primary underline">
            open a GitHub issue
          </a>{' '}
          (do not post your email in it; ask for deletion and we will arrange a private way to
          confirm). {/* TODO(owner): replace with a privacy@ mailbox once the domain exists. */}A
          privacy contact email will be added here once the site has its own domain.
        </p>
      </div>
    </div>
  );
}
