import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { Chapters } from '@/components/home/chapters';
import { FaqSection } from '@/components/home/faq-section';
import { Hero } from '@/components/home/hero';
import { Moods } from '@/components/home/moods';
import { OfflinePromise } from '@/components/home/promise';
import { TryBand } from '@/components/home/try-band';
import { WaitlistClose } from '@/components/home/waitlist-close';
import { JsonLd } from '@/components/json-ld';
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
  const faq = getProduct('kv-p1')?.faq ?? [];
  const selectedProduct = preselected ? getProduct(preselected)?.slug : undefined;

  return (
    <>
      <Hero />
      <Chapters />
      <TryBand />
      <Moods />
      <OfflinePromise />
      <FaqSection items={faq} />
      <WaitlistClose initialProduct={selectedProduct} />
      <JsonLd data={faqJsonLd(faq)} />
    </>
  );
}
