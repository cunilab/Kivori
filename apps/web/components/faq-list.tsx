import type { ReactElement } from 'react';
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@kivori/ui/components/accordion';
import type { FaqItem } from '@/content/products/types';

/** Shared Accordion (Base UI): keyboard accessible, one question open at a time by default. */
export function FaqList({ items }: { items: readonly FaqItem[] }): ReactElement {
  return (
    <Accordion className="rounded-xl border border-border bg-card px-5">
      {items.map((item) => (
        <AccordionItem key={item.question} value={item.question}>
          <AccordionTrigger className="py-4 text-base">{item.question}</AccordionTrigger>
          <AccordionContent className="text-base text-muted-foreground">
            {item.answer}
          </AccordionContent>
        </AccordionItem>
      ))}
    </Accordion>
  );
}
