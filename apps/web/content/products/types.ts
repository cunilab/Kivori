export type ProductStatus = 'in-development' | 'coming-soon' | 'available';

export type CompatibilityStatus = 'supported-beta' | 'supported' | 'later' | 'not-planned';

/** Brand art drawn in code or taken from the shared brand assets (never a photo, there are none yet). */
export interface ProductHero {
  art: 'device' | 'mascot';
  alt: string;
}

export interface GalleryImage {
  /** Public path of the 1600 px WebP built by `scripts/build-assets.mjs` (the 800 px twin sits beside it). */
  src: string;
  /** Intrinsic size of `src`, so the layout never shifts. */
  width: number;
  height: number;
  alt: string;
  caption: string;
}

export interface ProductFeature {
  title: string;
  body: string;
  /** Key into the icon set in `components/feature-icon.tsx`. */
  icon?: string;
}

export interface SpecRow {
  label: string;
  value: string;
}

export interface SpecGroup {
  group: string;
  rows: SpecRow[];
}

export interface Compatibility {
  os: string;
  status: CompatibilityStatus;
}

export interface FaqItem {
  question: string;
  answer: string;
}

export interface Product {
  slug: string;
  name: string;
  edition: string;
  status: ProductStatus;
  tagline: string;
  summary: string;
  hero: ProductHero;
  gallery: GalleryImage[];
  features: ProductFeature[];
  specs: SpecGroup[];
  compatibility: Compatibility[];
  /** Items that ship with the unit. A `TBC` entry means the contents are not decided yet. */
  inTheBox: string[];
  /** Absent means the product has no price yet and the page says "Coming soon". */
  price?: string;
  /** Absent means "Coming soon": no buy button is wired. */
  buyUrl?: string;
  faq: FaqItem[];
}
