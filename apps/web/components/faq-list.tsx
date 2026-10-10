import type { ReactElement } from 'react';
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@kivori/ui/components/accordion';
import type { FaqItem } from '@/content/products/types';

/** Shared Accordion (Base UI): keyboard accessible, one answer open at a time by default. */
export function FaqList({ items }: { items: readonly FaqItem[] }): ReactElement {
  return (
    <Accordion className="border-t border-border">
      {items.map((item) => (
        <AccordionItem key={item.question} value={item.question} className="border-b">
          <AccordionTrigger className="py-5 font-display text-lg font-bold tracking-tight hover:no-underline sm:text-xl">
            {item.question}
          </AccordionTrigger>
          <AccordionContent className="text-base text-muted-foreground sm:text-lg">
            <p className="max-w-3xl pb-3">{item.answer}</p>
          </AccordionContent>
        </AccordionItem>
      ))}
    </Accordion>
  );
}
