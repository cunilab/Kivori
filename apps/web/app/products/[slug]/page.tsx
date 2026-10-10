import Link from 'next/link';
import { notFound } from 'next/navigation';
import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { renders } from '@/lib/media';
import { FaqList } from '@/components/faq-list';
import { BuyBar } from '@/components/buy-bar';
import { FeatureIcon } from '@/components/feature-icon';
import { ProductShot } from '@/components/product-shot';
import { SpecsTable } from '@/components/specs-table';
import { JsonLd } from '@/components/json-ld';
import { Eyebrow, Section } from '@/components/section';
import { Badge } from '@kivori/ui/components/badge';
import { WaitlistSection } from '@/components/waitlist-section';
import { getProduct } from '@/content/products';
import { faqJsonLd, productJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { COMPATIBILITY_LABEL, PRODUCT_STATUS_LABEL } from '@/lib/product-labels';
import { Card } from '@kivori/ui/components/card';

interface ProductPageProps {
  params: Promise<{ slug: string }>;
}

// Rendered per request so the waitlist form gets the Turnstile site key from the Worker env at
// runtime. Only the slugs in the registry exist; anything else is a 404 (see `notFound` below).
export const dynamic = 'force-dynamic';

export async function generateMetadata({ params }: ProductPageProps): Promise<Metadata> {
  const { slug } = await params;
  const product = getProduct(slug);
  if (!product) return {};
  return pageMetadata({
    title: { absolute: `${product.name} ${product.edition} edition: specs and details` },
    description: `${product.tagline} ${product.summary}`,
    path: `/products/${product.slug}`,
    ogSlug: product.slug,
  });
}

export default async function ProductPage({ params }: ProductPageProps): Promise<ReactElement> {
  const { slug } = await params;
  const product = getProduct(slug);
  if (!product) notFound();

  return (
    <>
      <section
        aria-labelledby="product-title"
        className="overflow-hidden px-5 pt-14 sm:px-8 sm:pt-20"
      >
        <div className="mx-auto max-w-[1200px] text-center">
          <nav aria-label="Breadcrumb" className="text-sm text-muted-foreground">
            <Link href="/products" className="hover:text-foreground hover:underline">
              Products
            </Link>
            <span aria-hidden="true"> / </span>
            <span aria-current="page">{product.name}</span>
          </nav>
          <div className="mt-8 flex items-center justify-center gap-3">
            <Badge variant="secondary">{PRODUCT_STATUS_LABEL[product.status]}</Badge>
            <Eyebrow>{product.edition} edition</Eyebrow>
          </div>
          <h1 id="product-title" className="display-1 mt-4">
            {product.name}
          </h1>
          <p className="lead mx-auto mt-5 max-w-2xl text-balance">{product.tagline}</p>
          <div className="shot-stage mx-auto mt-10 max-w-[1000px] sm:mt-14">
            <ProductShot
              image={renders.hero}
              large
              priority
              sizes="(min-width: 1000px) 1000px, 100vw"
            />
          </div>
        </div>
      </section>

      <BuyBar
        name={product.name}
        price={product.price}
        buyUrl={product.buyUrl}
        waitlist={<WaitlistSection initialProduct={product.slug} idPrefix="buybar" />}
      />

      <Section
        id="features"
        eyebrow="Highlights"
        title={`What ${product.name} does.`}
        intro={product.summary}
      >
        <ul className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {product.features.map((feature) => (
            <li key={feature.title} className="reveal flex">
              <Card className="w-full gap-0 rounded-3xl p-7 shadow-sm ring-1 ring-foreground/8">
                <span className="flex size-11 items-center justify-center rounded-2xl bg-accent text-accent-foreground">
                  <FeatureIcon name={feature.icon} />
                </span>
                <h3 className="mt-5 text-xl font-semibold tracking-tight">{feature.title}</h3>
                <p className="mt-2 text-base text-muted-foreground">{feature.body}</p>
              </Card>
            </li>
          ))}
        </ul>
      </Section>

      <Section id="specs" tone="surface" eyebrow="Specs" title="Tech specs.">
        <div className="mx-auto max-w-3xl">
          <SpecsTable groups={product.specs} name={product.name} />
        </div>
      </Section>

      <Section id="compatibility" eyebrow="Compatibility" title="Works with.">
        <ul className="grid gap-4 sm:grid-cols-3">
          {product.compatibility.map((item) => (
            <li key={item.os}>
              <Card className="gap-1 rounded-3xl p-7 shadow-sm ring-1 ring-foreground/8">
                <p className="text-2xl font-semibold tracking-tight">{item.os}</p>
                <p
                  className={`text-sm ${item.status === 'supported' || item.status === 'supported-beta' ? 'font-medium text-success' : 'text-muted-foreground'}`}
                >
                  {COMPATIBILITY_LABEL[item.status]}
                </p>
              </Card>
            </li>
          ))}
        </ul>
      </Section>

      {product.inTheBox && product.inTheBox.length > 0 ? (
        <Section id="in-the-box" tone="surface" eyebrow="In the box" title="What you get.">
          <ul className="max-w-xl divide-y divide-border overflow-hidden rounded-3xl bg-card ring-1 ring-foreground/10">
            {product.inTheBox.map((item) => (
              <li key={item} className="px-6 py-4 text-base">
                {item}
              </li>
            ))}
          </ul>
        </Section>
      ) : null}

      <Section
        id="faq"
        tone={product.inTheBox?.length ? 'plain' : 'surface'}
        eyebrow="Questions"
        title="Good to know."
      >
        <div className="max-w-3xl">
          <FaqList items={product.faq} />
        </div>
      </Section>

      <JsonLd data={productJsonLd(product)} />
      <JsonLd data={faqJsonLd(product.faq)} />
    </>
  );
}
