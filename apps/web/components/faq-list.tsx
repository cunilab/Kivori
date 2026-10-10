import type { ReactElement } from 'react';
import type { FaqItem } from '@/content/products/types';

/** Native disclosure widgets: keyboard accessible and no client JS. */
export function FaqList({ items }: { items: readonly FaqItem[] }): ReactElement {
  return (
    <div className="divide-y divide-border rounded-xl border border-border bg-card">
      {items.map((item) => (
        <details key={item.question} className="group px-5 py-4">
          <summary className="flex cursor-pointer list-none items-center justify-between gap-4 font-medium [&::-webkit-details-marker]:hidden">
            {item.question}
            <svg
              viewBox="0 0 24 24"
              width="18"
              height="18"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
              className="shrink-0 text-muted-foreground transition-transform group-open:rotate-180"
            >
              <path d="m6 9 6 6 6-6" />
            </svg>
          </summary>
          <p className="mt-3 text-muted-foreground">{item.answer}</p>
        </details>
      ))}
    </div>
  );
}
