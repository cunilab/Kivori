import { describe, expect, it } from 'vitest';
import { kivori } from '@/content/products/kivori';
import type { Product } from '@/content/products/types';
import { buildComparison } from './comparison';

const other: Product = {
  ...kivori,
  slug: 'kivori-mini',
  name: 'Kivori Mini',
  price: '$99',
  specs: [
    { group: 'Display', rows: [{ label: 'Display', value: 'Smaller colour screen' }] },
    { group: 'Extras', rows: [{ label: 'Stand', value: 'Yes' }] },
  ],
};

describe('buildComparison', () => {
  it('renders a single product cleanly', () => {
    const c = buildComparison([kivori]);
    expect(c.columns).toEqual([{ slug: 'kivori', name: 'Kivori', edition: 'Beta' }]);
    expect(c.groups[0]).toEqual({
      group: 'Overview',
      rows: [
        { label: 'Status', values: ['Coming soon'] },
        { label: 'Price', values: ['Coming soon'] },
      ],
    });
    const display = c.groups.find((g) => g.group === 'Display');
    expect(display?.rows[0]).toEqual({ label: 'Display', values: ['Bright colour screen'] });
    for (const g of c.groups) for (const r of g.rows) expect(r.values).toHaveLength(1);
  });

  it('lines shared rows up and leaves gaps for rows only one product has', () => {
    const c = buildComparison([kivori, other]);
    const display = c.groups.find((g) => g.group === 'Display');
    expect(display?.rows.find((r) => r.label === 'Display')?.values).toEqual([
      'Bright colour screen',
      'Smaller colour screen',
    ]);
    expect(display?.rows.find((r) => r.label === 'Views')?.values[1]).toBeNull();
    expect(c.groups.find((g) => g.group === 'Extras')?.rows[0].values).toEqual([null, 'Yes']);
    expect(c.groups[0].rows[1].values).toEqual(['Coming soon', '$99']);
  });

  it('handles an empty lineup', () => {
    expect(buildComparison([]).columns).toEqual([]);
  });
});
