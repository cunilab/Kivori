import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { Eyebrow } from '@/components/section';
import { contactEmail } from '@/lib/contact';
import { pageMetadata } from '@/lib/metadata';

export const metadata: Metadata = pageMetadata({
  title: 'Privacy',
  description: 'What the Kivori website collects, why, and how to have it deleted.',
  path: '/privacy',
});

// Rendered per request so CONTACT_EMAIL is read from the Worker env at runtime.
export const dynamic = 'force-dynamic';

export default function PrivacyPage(): ReactElement {
  const email = contactEmail();
  return (
    <div className="px-5 pt-20 pb-24 sm:px-8 sm:pt-28 sm:pb-32">
      <div className="mx-auto max-w-3xl">
        <Eyebrow>Privacy</Eyebrow>
        <h1 className="display-1 mt-3">Your data, plainly.</h1>
        <p className="lead mt-6">
          This page covers this website. The Kivori desktop app works offline, needs no account and
          sends no telemetry.
        </p>

        <h2 className="mt-14 font-display text-2xl font-bold tracking-tight sm:text-3xl">
          Visiting the site
        </h2>
        <p className="mt-3 text-base text-muted-foreground sm:text-lg">
          We use Cloudflare Web Analytics to count page views. It is cookieless: it sets no cookies,
          does not track you across sites and does not build a profile of you. (It is being added to
          the site; this is how it will work.) Download clicks are counted in Cloudflare Analytics
          Engine as totals by operating system and version, with no personal data.
        </p>

        <h2 className="mt-14 font-display text-2xl font-bold tracking-tight sm:text-3xl">
          The waitlist
        </h2>
        <p className="mt-3 text-base text-muted-foreground sm:text-lg">
          If you join the waitlist we store:
        </p>
        <ul className="mt-3 list-disc space-y-1.5 pl-6 text-base text-muted-foreground marker:text-primary sm:text-lg">
          <li>your email address</li>
          <li>the product you chose</li>
          <li>your answer to &ldquo;What would you use Kivori for?&rdquo;, if you wrote one</li>
          <li>
            where you came from: the UTM tags in the link you used (source, medium, campaign) and
            the referring page
          </li>
          <li>the time you signed up</li>
        </ul>
        <p className="mt-3 text-base text-muted-foreground sm:text-lg">
          We use this only to contact you about Kivori availability. We never sell it or share it
          for advertising. It is stored in Cloudflare D1.
        </p>

        <h2 className="mt-14 font-display text-2xl font-bold tracking-tight sm:text-3xl">
          Your IP address
        </h2>
        <p className="mt-3 text-base text-muted-foreground sm:text-lg">
          Your IP address is not stored. It is used only for a moment, to limit how many sign-ups
          one network can send per minute, and is passed to Cloudflare Turnstile, the bot check on
          the form.
        </p>

        <h2 className="mt-14 font-display text-2xl font-bold tracking-tight sm:text-3xl">
          Deleting your data
        </h2>
        <p className="mt-3 text-base text-muted-foreground sm:text-lg">
          To have your waitlist entry deleted, email us and ask for deletion.{' '}
          {email ? (
            <a href={`mailto:${email}`} className="text-primary underline">
              {email}
            </a>
          ) : (
            'Contact details will be published here before launch.'
          )}
        </p>
      </div>
    </div>
  );
}
