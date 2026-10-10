import Image from 'next/image';
import Link from 'next/link';
import { notFound } from 'next/navigation';
import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import { renders } from '@/lib/media';
import { FaqList } from '@/components/faq-list';
import { FeatureIcon } from '@/components/feature-icon';
import { JsonLd } from '@/components/json-ld';
import { Section } from '@/components/section';
import { Badge } from '@kivori/ui/components/badge';
import { WaitlistSection } from '@/components/waitlist-section';
import { getProduct } from '@/content/products';
import { faqJsonLd, productJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { COMPATIBILITY_LABEL } from '@/lib/product-labels';
import { PRODUCT_STATUS_LABEL } from '@/lib/product-labels';
import { Button } from '@kivori/ui/components/button';

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
      <section aria-labelledby="product-title" className="px-5 py-14 sm:py-20">
        <div className="mx-auto grid max-w-5xl items-center gap-12 md:grid-cols-2">
          <div>
            <nav aria-label="Breadcrumb" className="text-sm text-muted-foreground">
              <Link href="/products" className="hover:text-foreground hover:underline">
                Products
              </Link>
              <span aria-hidden="true"> / </span>
              <span aria-current="page">{product.name}</span>
            </nav>
            <div className="mt-5 flex items-center gap-3">
              <Badge variant="secondary">{PRODUCT_STATUS_LABEL[product.status]}</Badge>
              <span className="text-sm text-muted-foreground">{product.edition} edition</span>
            </div>
            <h1
              id="product-title"
              className="mt-4 font-display text-4xl leading-tight font-semibold sm:text-5xl"
            >
              {product.name}
            </h1>
            <p className="mt-3 font-display text-xl text-primary">{product.tagline}</p>
            <p className="mt-4 max-w-xl text-lg text-muted-foreground">{product.summary}</p>

            <div id="buy" className="mt-8 flex flex-wrap items-center gap-3">
              {product.buyUrl ? (
                <Button size="lg" nativeButton={false} render={<a href={product.buyUrl} />}>
                  Buy {product.name}
                  {product.price ? ` · ${product.price}` : ''}
                </Button>
              ) : (
                <Button size="lg" variant="outline" disabled>
                  Buy, coming soon
                </Button>
              )}
              <Button
                size="lg"
                variant="outline"
                nativeButton={false}
                render={<a href="#waitlist" />}
              >
                Notify me
              </Button>
            </div>
            <p className="mt-3 text-sm text-muted-foreground">
              {product.price
                ? `Price: ${product.price}`
                : 'No price or ship date yet. Notify me and we will tell you.'}
            </p>
            <div id="waitlist" className="mt-6 max-w-md scroll-mt-20">
              <WaitlistSection initialProduct={product.slug} idPrefix="product" />
            </div>
          </div>
          <div className="mx-auto w-full max-w-xl">
            <Image
              src={renders.heroTransparent.src}
              alt={product.hero.alt}
              width={renders.heroTransparent.width}
              height={renders.heroTransparent.height}
              priority
              className="h-auto w-full"
            />
          </div>
        </div>
      </section>

      <Section id="features" eyebrow="Features" title={`What ${product.name} does`}>
        <ul className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {product.features.map((feature) => (
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

      <Section id="specs" tone="muted" eyebrow="Specs" title="Technical specifications">
        <div className="grid gap-6 md:grid-cols-2">
          {product.specs.map((group) => (
            <div key={group.group} className="rounded-2xl border border-border bg-card p-6">
              <h3 className="font-display text-lg font-semibold">{group.group}</h3>
              <dl className="mt-3 divide-y divide-border text-sm">
                {group.rows.map((row) => (
                  <div key={row.label} className="flex justify-between gap-6 py-2.5">
                    <dt className="font-medium">{row.label}</dt>
                    <dd className="text-right text-muted-foreground">{row.value}</dd>
                  </div>
                ))}
              </dl>
            </div>
          ))}
        </div>
      </Section>

      <Section id="compatibility" eyebrow="Compatibility" title="Works with">
        <ul className="grid gap-4 sm:grid-cols-3">
          {product.compatibility.map((item) => (
            <li key={item.os} className="rounded-2xl border border-border bg-card p-5">
              <p className="font-display text-lg font-semibold">{item.os}</p>
              <p
                className={`mt-1 text-sm ${item.status === 'supported' || item.status === 'supported-beta' ? 'font-medium text-success' : 'text-muted-foreground'}`}
              >
                {COMPATIBILITY_LABEL[item.status]}
              </p>
            </li>
          ))}
        </ul>
      </Section>

      {product.inTheBox && product.inTheBox.length > 0 ? (
        <Section id="in-the-box" tone="muted" eyebrow="In the box" title="What you get">
          <ul className="max-w-xl divide-y divide-border rounded-2xl border border-border bg-card">
            {product.inTheBox.map((item) => (
              <li key={item} className="px-5 py-3 text-sm">
                {item}
              </li>
            ))}
          </ul>
        </Section>
      ) : null}

      <Section id="faq" eyebrow="Questions" title="Good to know">
        <div className="max-w-3xl">
          <FaqList items={product.faq} />
        </div>
        <div className="mt-10 flex flex-wrap items-center gap-3">
          {product.buyUrl ? (
            <Button size="lg" nativeButton={false} render={<a href={product.buyUrl} />}>
              Buy {product.name}
            </Button>
          ) : (
            <Button size="lg" variant="outline" disabled>
              Buy, coming soon
            </Button>
          )}
          <Button size="lg" variant="outline" nativeButton={false} render={<a href="#waitlist" />}>
            Notify me
          </Button>
        </div>
      </Section>

      <JsonLd data={productJsonLd(product)} />
      <JsonLd data={faqJsonLd(product.faq)} />
    </>
  );
}
