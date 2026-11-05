import type { ReactElement } from 'react';
import { FaqList } from '@/components/faq-list';
import type { FaqItem } from '@/content/products/types';

export function FaqSection({ items }: { items: readonly FaqItem[] }): ReactElement {
  return (
    <section
      id="faq"
      aria-labelledby="faq-title"
      className="scroll-mt-20 overflow-x-clip border-t border-home-hairline bg-home-paper px-5 py-14 sm:py-20 sm:px-8"
    >
      <div className="mx-auto grid max-w-[1180px] gap-10 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)] lg:gap-16">
        <h2
          id="faq-title"
          className="home-display text-[clamp(2rem,4.4vw,3.25rem)] text-home-ink lg:pt-5"
        >
          Questions.
        </h2>
        <FaqList items={items} />
      </div>
    </section>
  );
}
