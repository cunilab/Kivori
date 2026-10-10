import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { FaqList } from '@/components/faq-list';
import { Hero } from '@/components/hero';
import {
  CustomizeSection,
  FinalBand,
  LightsSection,
  MoodsSection,
  PrivacySection,
  ProductLookSection,
  ProfilesSection,
} from '@/components/home-sections';
import { JsonLd } from '@/components/json-ld';
import { Section } from '@/components/section';
import { ScreensShowcase } from '@/components/screens-showcase';
import { getProduct } from '@/content/products';
import { faqJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { DESCRIPTION } from '@/lib/site';

export const metadata: Metadata = pageMetadata({ description: DESCRIPTION, path: '/' });

interface HomePageProps {
  searchParams: Promise<{ product?: string | string[] }>;
}

export default async function HomePage({ searchParams }: HomePageProps): Promise<ReactElement> {
  const { product: productParam } = await searchParams;
  const preselected = Array.isArray(productParam) ? productParam[0] : productParam;
  const faq = getProduct('kivori')?.faq ?? [];

  return (
    <>
      <Hero initialProduct={preselected} />
      <MoodsSection />
      <ProfilesSection />
      <Section
        id="screens"
        tone="surface"
        eyebrow="The screen"
        title="Five screens, one double press."
        intro="Buddy, clock, volume, media and system. Double-press the knob to flip between them."
        align="center"
      >
        <ScreensShowcase />
      </Section>
      <LightsSection />
      <CustomizeSection />
      <PrivacySection />
      <ProductLookSection />
      <Section id="faq" tone="surface" eyebrow="Questions" title="Good to know.">
        <div className="max-w-3xl">
          <FaqList items={faq} />
        </div>
      </Section>
      <FinalBand />
      <JsonLd data={faqJsonLd(faq)} />
    </>
  );
}
