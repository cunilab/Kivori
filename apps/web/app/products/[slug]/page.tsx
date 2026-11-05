import Link from 'next/link';
import { notFound } from 'next/navigation';
import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { FaqList } from '@/components/faq-list';
import { BuyBar } from '@/components/buy-bar';
import { ProductStage } from '@/components/product-shot';
import { SpecsTable } from '@/components/specs-table';
import { JsonLd } from '@/components/json-ld';
import { Section } from '@/components/section';
import { Badge } from '@kivori/ui/components/badge';
import { WaitlistSection } from '@/components/waitlist-section';
import { getProduct } from '@/content/products';
import { faqJsonLd, productJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { COMPATIBILITY_LABEL, PRODUCT_STATUS_LABEL } from '@/lib/product-labels';

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
      <section aria-labelledby="product-title" className="px-5 pt-10 pb-12 sm:px-8 sm:pt-14">
        <div className="mx-auto max-w-[1180px]">
          <nav aria-label="Breadcrumb" className="text-sm text-muted-foreground">
            <Link href="/products" className="hover:underline">
              Products
            </Link>
            <span aria-hidden="true"> / </span>
            <span aria-current="page">{product.name}</span>
          </nav>
          <div className="mt-8 grid items-center gap-8 md:grid-cols-2 md:gap-16">
            <div>
              <Badge variant="secondary">{PRODUCT_STATUS_LABEL[product.status]}</Badge>
              <h1 id="product-title" className="display-1 mt-4">
                {product.name}
              </h1>
              <p className="lead mt-5 max-w-lg">{product.summary}</p>
              <p className="mt-5 text-sm text-muted-foreground">
                {product.edition} edition · {product.price ?? 'Price and ship date to come'}
              </p>
            </div>
            <div className="rounded-lg bg-surface p-6 sm:p-10">
              <ProductStage
                image={product.hero.image}
                large
                priority
                sizes="(min-width: 768px) 550px, 100vw"
              />
            </div>
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
        <ul className="grid gap-x-10 md:grid-cols-2">
          {product.features.map((feature) => (
            <li key={feature.title} className="border-t border-border py-6">
              <h3 className="text-xl font-semibold">{feature.title}</h3>
              <p className="mt-2 max-w-xl leading-relaxed text-muted-foreground">{feature.body}</p>
            </li>
          ))}
        </ul>
      </Section>

      <Section id="specs" eyebrow="Specs" title="Tech specs.">
        <div className="mx-auto max-w-3xl">
          <SpecsTable groups={product.specs} name={product.name} />
        </div>
      </Section>

      <Section id="compatibility" eyebrow="Compatibility" title="Works with.">
        <ul className="grid gap-4 sm:grid-cols-3">
          {product.compatibility.map((item) => (
            <li key={item.os} className="border-t border-border py-5">
              <p className="text-xl font-semibold">{item.os}</p>
              <p className="mt-2 text-sm text-muted-foreground">
                {COMPATIBILITY_LABEL[item.status]}
              </p>
            </li>
          ))}
        </ul>
      </Section>

      {product.inTheBox && product.inTheBox.length > 0 ? (
        <Section id="in-the-box" eyebrow="In the box" title="What you get.">
          <ul className="max-w-xl divide-y divide-border overflow-hidden rounded-2xl bg-card ring-1 ring-border">
            {product.inTheBox.map((item) => (
              <li key={item} className="px-6 py-4 text-base">
                {item}
              </li>
            ))}
          </ul>
        </Section>
      ) : null}

      <Section id="faq" eyebrow="Questions" title="Good to know.">
        <div className="max-w-3xl">
          <FaqList items={product.faq} />
        </div>
      </Section>

      <JsonLd data={productJsonLd(product)} />
      <JsonLd data={faqJsonLd(product.faq)} />
    </>
  );
}
