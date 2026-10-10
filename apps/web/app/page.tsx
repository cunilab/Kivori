import Image from 'next/image';
import Link from 'next/link';
import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import mascot from '@brand/mascot.svg';
import { Bento } from '@/components/bento';
import { FaqList } from '@/components/faq-list';
import { JsonLd } from '@/components/json-ld';
import { ProductShot } from '@/components/product-shot';
import { ScreensShowcase } from '@/components/screens-showcase';
import { ScrollStory } from '@/components/scroll-story';
import { Eyebrow, Section, SectionHeading } from '@/components/section';
import { WaitlistSection } from '@/components/waitlist-section';
import { getProduct } from '@/content/products';
import { faqJsonLd } from '@/lib/jsonld';
import { renders } from '@/lib/media';
import { pageMetadata } from '@/lib/metadata';
import { DESCRIPTION } from '@/lib/site';

export const metadata: Metadata = pageMetadata({ description: DESCRIPTION, path: '/' });

const textLink = 'text-lg font-medium text-primary hover:underline sm:text-xl';

interface HomePageProps {
  searchParams: Promise<{ product?: string | string[] }>;
}

export default async function HomePage({ searchParams }: HomePageProps): Promise<ReactElement> {
  const { product: productParam } = await searchParams;
  const preselected = Array.isArray(productParam) ? productParam[0] : productParam;
  const faq = getProduct('kivori')?.faq ?? [];

  return (
    <>
      <section aria-labelledby="hero-title" className="overflow-hidden px-5 pt-16 sm:px-8 sm:pt-24">
        <div className="mx-auto max-w-[1200px] text-center">
          <Eyebrow>Coming soon</Eyebrow>
          <h1 id="hero-title" className="display-1 mt-3">
            Meet Kivori.
          </h1>
          <p className="lead mx-auto mt-5 max-w-xl text-balance">The desk buddy that works.</p>
          <p className="mt-7 flex flex-wrap items-center justify-center gap-x-8 gap-y-2">
            <Link href="/products/kivori" className={textLink}>
              Learn more &rsaquo;
            </Link>
            <a href="#waitlist" className={textLink}>
              Notify me &rsaquo;
            </a>
          </p>
          <div className="shot-stage hero-shot mx-auto mt-10 max-w-[1100px] sm:mt-14">
            <ProductShot
              image={renders.hero}
              large
              priority
              sizes="(min-width: 1100px) 1100px, 100vw"
            />
          </div>
        </div>
      </section>

      <section aria-label="In a sentence" className="px-5 py-24 sm:px-8 sm:py-36">
        <p className="display-2 reveal mx-auto max-w-4xl text-center text-balance">
          Control your desktop with your hands.{' '}
          <span className="text-muted-foreground">See it at a glance.</span>
        </p>
      </section>

      <section
        id="story"
        aria-labelledby="story-title"
        className="bg-surface px-5 pt-24 pb-12 sm:px-8 sm:pt-32"
      >
        <div className="mx-auto max-w-[1200px]">
          <SectionHeading
            id="story-title"
            eyebrow="How it feels"
            title="Turn. Press. Glance."
            align="center"
          />
          <div className="mt-12 md:mt-20">
            <ScrollStory />
          </div>
        </div>
      </section>

      <Section
        id="screens"
        tone="dark"
        eyebrow="The screen"
        title="Your desktop at a glance."
        intro="Five views, one double press apart."
        align="center"
      >
        <ScreensShowcase />
      </Section>

      <Section
        id="why"
        tone="surface"
        eyebrow="Why Kivori"
        title="Small on your desk. Big on getting things done."
      >
        <Bento />
      </Section>

      <Section
        id="fit"
        eyebrow="Design"
        title="Fits beside your keyboard."
        intro="A slim wedge that tilts the screen towards you, with a roomy knob and keys you can find by feel."
        align="center"
      >
        <div className="shot-stage reveal mx-auto max-w-4xl">
          <ProductShot image={renders.side} sizes="(min-width: 900px) 900px, 100vw" />
        </div>
      </Section>

      <section
        id="waitlist"
        aria-labelledby="waitlist-title"
        className="scroll-mt-14 bg-surface px-5 py-24 sm:px-8 sm:py-32"
      >
        <div className="mx-auto flex max-w-xl flex-col items-center text-center">
          <Image src={mascot} alt="" width={72} height={72} />
          <h2 id="waitlist-title" className="display-2 mt-5">
            Be the first to know.
          </h2>
          <p className="lead mt-4">
            There is no price or ship date yet. Leave your email and we will tell you when there is.
          </p>
          <div className="mt-10 w-full">
            <WaitlistSection initialProduct={preselected} idPrefix="home" />
          </div>
        </div>
      </section>

      <Section id="faq" eyebrow="Questions" title="Good to know.">
        <div className="max-w-3xl">
          <FaqList items={faq} />
        </div>
      </Section>

      <JsonLd data={faqJsonLd(faq)} />
    </>
  );
}
