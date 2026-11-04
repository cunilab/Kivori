import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { kivori } from '@/content/products/kivori';
import { getProducts } from '@/content/products';
import ProductsPage from './page';

vi.mock('@/content/products', () => ({ getProducts: vi.fn() }));
vi.mock('@/components/product-card', () => ({ ProductCard: () => null }));
vi.mock('@/components/comparison-table', () => ({ ComparisonTable: () => 'Product comparison' }));

describe('products page', () => {
  it('shows comparison only when there are multiple products', () => {
    vi.mocked(getProducts).mockReturnValue([kivori]);
    expect(renderToStaticMarkup(createElement(ProductsPage))).not.toContain('Product comparison');

    vi.mocked(getProducts).mockReturnValue([kivori, { ...kivori, slug: 'second' }]);
    expect(renderToStaticMarkup(createElement(ProductsPage))).toContain('Product comparison');
  });
});
