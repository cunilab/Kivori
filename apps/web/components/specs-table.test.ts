import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { kivori } from '@/content/products/kivori';
import { SpecsTable } from './specs-table';

describe('product specifications', () => {
  it('renders every group, label and value from the product', () => {
    const html = renderToStaticMarkup(
      createElement(SpecsTable, { groups: kivori.specs, name: kivori.name }),
    );
    expect(html).toContain(`${kivori.name} tech specs`);
    for (const group of kivori.specs) {
      expect(html).toContain(group.group);
      for (const row of group.rows) {
        expect(html).toContain(row.label);
        expect(html).toContain(row.value);
      }
    }
  });
});
