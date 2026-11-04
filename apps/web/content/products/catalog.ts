import { catalog } from './catalog.generated';

type CatalogProduct = {
  id: string;
  wire: number;
  model: string;
  layout: string | null;
  image: { src: string; width: number; height: number } | null;
};

export function catalogProduct(id: string): CatalogProduct {
  const product: CatalogProduct | undefined = catalog.find((entry) => entry.id === id);
  if (!product) throw new Error(`Unknown catalog product: ${id}`);
  if (!product.image && product.layout !== 'rotary-three-button') {
    throw new Error(`No website visual configured for catalog product: ${id}`);
  }
  return product;
}
