import Image from 'next/image';
import Link from 'next/link';
import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import mascot from '@brand/mascot.svg';
import { ConfirmationBadges } from '@/components/confirmation-badges';
import { DeviceIllustration } from '@/components/device-illustration';
import { FaqList } from '@/components/faq-list';
import { FeatureIcon } from '@/components/feature-icon';
import { JsonLd } from '@/components/json-ld';
import { ProductCard } from '@/components/product-card';
import { Section } from '@/components/section';
import { WaitlistSection } from '@/components/waitlist-section';
import { getProduct, getProducts } from '@/content/products';
import { faqJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { DESCRIPTION, TAGLINE } from '@/lib/site';
import { Button } from '@kivori/ui/components/button';

export const metadata: Metadata = pageMetadata({ description: DESCRIPTION, path: '/' });

const KNOB = [
  { gesture: 'Turn', does: 'Volume. Faster turns take bigger steps.' },
  { gesture: 'Press', does: 'Play / Pause.' },
  { gesture: 'Hold', does: 'Mute (about a second, then let go).' },
  { gesture: 'Double press', does: 'Switch the screen view.' },
] as const;

const BUTTONS = [
  { name: 'Left', does: 'Previous track' },
  { name: 'Middle', does: 'Play / Pause. Hold it to pin a profile.' },
  { name: 'Right', does: 'Next track' },
] as const;

const VIEWS = [
  { name: 'Buddy', does: 'Your buddy, the profile name and the control labels.' },
  { name: 'Clock', does: 'The time.' },
  { name: 'Volume', does: 'The level, and whether you are muted.' },
  { name: 'Media', does: 'What is playing now.' },
  { name: 'System', does: 'CPU and memory.' },
] as const;

interface HomePageProps {
  searchParams: Promise<{ product?: string | string[] }>;
}

export default async function HomePage({ searchParams }: HomePageProps): Promise<ReactElement> {
  const { product: productParam } = await searchParams;
  const preselected = Array.isArray(productParam) ? productParam[0] : productParam;
  const featured = getProduct('kivori');
  const products = getProducts();
  const features = featured?.features ?? [];
  const faq = featured?.faq ?? [];

  return (
    <>
      <section aria-labelledby="hero-title" className="px-5 py-14 sm:py-20">
        <div className="mx-auto grid max-w-5xl items-center gap-12 md:grid-cols-2">
          <div>
            <p className="inline-block rounded-full border border-border bg-accent px-3 py-1 text-xs font-medium tracking-wide text-accent-foreground uppercase">
              Coming soon &middot; Windows beta
            </p>
            <h1
              id="hero-title"
              className="mt-5 font-display text-4xl leading-tight font-semibold sm:text-5xl"
            >
              {TAGLINE}
            </h1>
            <p className="mt-5 max-w-xl text-lg text-muted-foreground">
              Kivori is a desk buddy you control your computer with. The buddy is why you want one;
              the physical knob and button are why you keep using it.
            </p>
            <div className="mt-8 flex flex-wrap gap-3">
              <Button size="lg" nativeButton={false} render={<a href="#waitlist" />}>
                Join the waitlist
              </Button>
              <Button
                size="lg"
                variant="outline"
                nativeButton={false}
                render={<Link href="/products/kivori" />}
              >
                See Kivori
              </Button>
            </div>
          </div>
          <div className="relative mx-auto w-full max-w-md">
            <DeviceIllustration
              label="Illustration of Kivori: a wedge-shaped desk device with a big knob, a small square screen and three buttons."
              className="w-full drop-shadow-xl"
            />
            <Image
              src={mascot}
              alt="The Kivori keycap buddy"
              width={120}
              height={120}
              priority
              className="animate-float absolute -top-8 right-2 sm:-top-10 sm:right-0"
            />
          </div>
        </div>
      </section>

      <Section
        id="pillars"
        tone="muted"
        eyebrow="Two pillars, equal weight"
        title="A buddy you want. A controller you keep."
        intro="Kivori is two things at once, and neither is an afterthought."
      >
        <div className="grid gap-6 md:grid-cols-2">
          <article className="rounded-2xl border border-border bg-card p-7">
            <Image src={mascot} alt="" width={64} height={64} />
            <h3 className="mt-4 font-display text-2xl font-semibold">The buddy</h3>
            <p className="mt-1 text-sm font-medium text-primary">Why you want one</p>
            <p className="mt-3 text-muted-foreground">
              The buddy is always on screen. It shows what your computer is really doing, and it
              tells you what the controls do right now. It has personality, but it only reports your
              desktop: it is not a virtual pet with goals of its own.
            </p>
          </article>
          <article className="rounded-2xl border border-border bg-card p-7">
            <span className="flex size-16 items-center justify-center rounded-2xl bg-accent text-accent-foreground">
              <FeatureIcon name="knob" />
            </span>
            <h3 className="mt-4 font-display text-2xl font-semibold">The controller</h3>
            <p className="mt-1 text-sm font-medium text-primary">Why you keep using it</p>
            <p className="mt-3 text-muted-foreground">
              A real knob and real buttons for the things you do all day: volume, media, mute,
              shortcuts, launching apps. Every input is acknowledged at once, and the screen says
              honestly how it turned out.
            </p>
          </article>
        </div>
      </Section>

      <Section
        id="how-it-works"
        eyebrow="How it works"
        title="Turn it, press it, glance at it"
        intro="These are the defaults. Every control can be changed per profile in Kivori Desktop."
      >
        <div className="grid gap-6 lg:grid-cols-2">
          <div className="rounded-2xl border border-border bg-card p-6">
            <h3 className="font-display text-xl font-semibold">The knob</h3>
            <dl className="mt-4 divide-y divide-border">
              {KNOB.map((item) => (
                <div key={item.gesture} className="flex gap-4 py-3">
                  <dt className="w-28 shrink-0 font-medium">{item.gesture}</dt>
                  <dd className="text-muted-foreground">{item.does}</dd>
                </div>
              ))}
            </dl>
          </div>
          <div className="rounded-2xl border border-border bg-card p-6">
            <h3 className="font-display text-xl font-semibold">Three buttons</h3>
            <dl className="mt-4 divide-y divide-border">
              {BUTTONS.map((item) => (
                <div key={item.name} className="flex gap-4 py-3">
                  <dt className="w-28 shrink-0 font-medium">{item.name}</dt>
                  <dd className="text-muted-foreground">{item.does}</dd>
                </div>
              ))}
            </dl>
          </div>
        </div>
        <h3 className="mt-10 font-display text-xl font-semibold">Five display views</h3>
        <ul className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
          {VIEWS.map((view) => (
            <li key={view.name} className="rounded-xl bg-panel p-4 text-[#e8ecf5]">
              <p className="font-display font-semibold text-[#7df2c4]">{view.name}</p>
              <p className="mt-1 text-sm text-[#a9b3c9]">{view.does}</p>
            </li>
          ))}
        </ul>
      </Section>

      <Section
        id="features"
        tone="muted"
        eyebrow="What it does"
        title="Built to be useful on a real desk"
      >
        <ul className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((feature) => (
            <li key={feature.title} className="rounded-2xl border border-border bg-card p-6">
              <span className="flex size-11 items-center justify-center rounded-xl bg-accent text-accent-foreground">
                <FeatureIcon name={feature.icon} />
              </span>
              <h3 className="mt-4 font-display text-lg font-semibold">{feature.title}</h3>
              <p className="mt-2 text-sm text-muted-foreground">{feature.body}</p>
            </li>
          ))}
        </ul>
      </Section>

      <Section
        id="honest"
        eyebrow="Honest by design"
        title="Kivori never claims more than it knows"
        intro="After every action the screen shows one of four badges, in the colours the device itself uses. A timeout alone is never shown as an error, and a shortcut is never shown as a success it cannot prove."
      >
        <ConfirmationBadges />
      </Section>

      <Section id="private" tone="muted" eyebrow="Private by default" title="Your desk stays yours">
        <div className="grid gap-5 sm:grid-cols-3">
          {[
            ['Works offline', 'Nothing in the app needs the internet.'],
            ['No account', 'There is nothing to sign up for or sign in to.'],
            ['No telemetry', 'No cloud. Your settings stay on your computer, per user.'],
          ].map(([title, body]) => (
            <div key={title} className="rounded-2xl border border-border bg-card p-6">
              <h3 className="font-display text-lg font-semibold">{title}</h3>
              <p className="mt-2 text-sm text-muted-foreground">{body}</p>
            </div>
          ))}
        </div>
      </Section>

      <Section
        id="lineup"
        eyebrow="Lineup"
        title="Meet Kivori"
        intro="The beta is for Windows. macOS comes after it."
      >
        <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {products.map((product) => (
            <ProductCard key={product.slug} product={product} />
          ))}
        </div>
        <p className="mt-6">
          <Link href="/products" className="text-sm font-medium text-primary hover:underline">
            Compare the lineup
          </Link>
        </p>
      </Section>

      <Section id="faq" tone="muted" eyebrow="Questions" title="Good to know">
        <div className="max-w-3xl">
          <FaqList items={faq} />
        </div>
      </Section>

      <section id="waitlist" aria-labelledby="waitlist-title" className="scroll-mt-20 px-5 py-20">
        <div className="mx-auto flex max-w-2xl flex-col items-center text-center">
          <Image src={mascot} alt="" width={96} height={96} />
          <h2 id="waitlist-title" className="mt-4 font-display text-3xl font-semibold sm:text-4xl">
            Join the waitlist
          </h2>
          <p className="mt-4 text-lg text-muted-foreground">
            There is no price or ship date yet. Put your name down and we will tell you when there
            is.
          </p>
          <div className="mt-8 w-full">
            <WaitlistSection initialProduct={preselected} idPrefix="home" />
          </div>
          <Button
            size="lg"
            variant="outline"
            className="mt-6"
            nativeButton={false}
            render={<Link href="/products/kivori" />}
          >
            See Kivori
          </Button>
        </div>
      </section>

      <JsonLd data={faqJsonLd(faq)} />
    </>
  );
}
