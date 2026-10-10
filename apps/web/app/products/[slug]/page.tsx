import Image from 'next/image';
import Link from 'next/link';
import { notFound } from 'next/navigation';
import type { Metadata } from 'next';
import type { ReactElement } from 'react';
import mascot from '@brand/mascot.svg';
import { DeviceIllustration } from '@/components/device-illustration';
import { FaqList } from '@/components/faq-list';
import { FeatureIcon } from '@/components/feature-icon';
import { JsonLd } from '@/components/json-ld';
import { Section } from '@/components/section';
import { StatusPill } from '@/components/status-pill';
import { getProduct, getProducts } from '@/content/products';
import { faqJsonLd, productJsonLd } from '@/lib/jsonld';
import { pageMetadata } from '@/lib/metadata';
import { COMPATIBILITY_LABEL, notifyHref } from '@/lib/product-labels';
import { BTN_DISABLED, BTN_PRIMARY, BTN_SECONDARY } from '@/lib/ui';

interface ProductPageProps {
  params: Promise<{ slug: string }>;
}

// Only the slugs in the registry exist; anything else is a 404.
export const dynamicParams = false;

export function generateStaticParams(): { slug: string }[] {
  return getProducts().map(({ slug }) => ({ slug }));
}

export async function generateMetadata({ params }: ProductPageProps): Promise<Metadata> {
  const { slug } = await params;
  const product = getProduct(slug);
  if (!product) return {};
  return pageMetadata({
    title: product.name,
    description: `${product.tagline} ${product.summary}`,
    path: `/products/${product.slug}`,
    ogSlug: product.slug,
  });
}

/** The 800 px twin of a gallery image, used for the thumbnail. */
function thumbnail(src: string): string {
  return src.replace(/-1600\.webp$/, '-800.webp');
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
              <StatusPill status={product.status} />
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
                <a href={product.buyUrl} className={BTN_PRIMARY}>
                  Buy {product.name}
                  {product.price ? ` · ${product.price}` : ''}
                </a>
              ) : (
                <button type="button" disabled className={BTN_DISABLED}>
                  Buy, coming soon
                </button>
              )}
              <Link href={notifyHref(product.slug)} className={BTN_SECONDARY}>
                Notify me
              </Link>
            </div>
            <p className="mt-3 text-sm text-muted-foreground">
              {product.price
                ? `Price: ${product.price}`
                : 'No price or ship date yet. Notify me and we will tell you.'}
            </p>
          </div>
          <div className="relative mx-auto w-full max-w-md">
            <DeviceIllustration label={product.hero.alt} className="w-full drop-shadow-xl" />
            <Image
              src={mascot}
              alt=""
              width={96}
              height={96}
              className="animate-float absolute -top-6 right-2 sm:-top-8 sm:right-0"
            />
          </div>
        </div>
      </section>

      {product.gallery.length > 0 ? (
        <Section
          id="gallery"
          tone="muted"
          eyebrow="Gallery"
          title="Drawn before it is built"
          intro="There are no product photos yet. These are the enclosure blueprints the prototype follows."
        >
          <ul className="grid gap-6 md:grid-cols-2">
            {product.gallery.map((image) => (
              <li key={image.src}>
                <figure className="overflow-hidden rounded-2xl border border-border bg-card">
                  <a href={image.src} aria-label={`Open full size: ${image.alt}`}>
                    <Image
                      src={thumbnail(image.src)}
                      alt={image.alt}
                      width={800}
                      height={Math.round((image.height / image.width) * 800)}
                      sizes="(min-width: 768px) 50vw, 100vw"
                      className="h-auto w-full"
                    />
                  </a>
                  <figcaption className="p-4 text-sm text-muted-foreground">
                    {image.caption}
                  </figcaption>
                </figure>
              </li>
            ))}
          </ul>
        </Section>
      ) : null}

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

      <Section
        id="in-the-box"
        tone="muted"
        eyebrow="In the box"
        title="What you get"
        intro="The beta kit is not final, so the contents are still to be confirmed."
      >
        <ul className="max-w-xl divide-y divide-border rounded-2xl border border-border bg-card">
          {product.inTheBox.map((item) => (
            <li key={item} className="px-5 py-3 text-sm">
              {item}
            </li>
          ))}
        </ul>
      </Section>

      <Section id="faq" eyebrow="Questions" title="Good to know">
        <div className="max-w-3xl">
          <FaqList items={product.faq} />
        </div>
        <div className="mt-10 flex flex-wrap items-center gap-3">
          {product.buyUrl ? (
            <a href={product.buyUrl} className={BTN_PRIMARY}>
              Buy {product.name}
            </a>
          ) : (
            <button type="button" disabled className={BTN_DISABLED}>
              Buy, coming soon
            </button>
          )}
          <Link href={notifyHref(product.slug)} className={BTN_SECONDARY}>
            Notify me
          </Link>
        </div>
      </Section>

      <JsonLd data={productJsonLd(product)} />
      <JsonLd data={faqJsonLd(product.faq)} />
    </>
  );
}
