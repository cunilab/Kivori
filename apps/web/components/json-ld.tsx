import type { ReactElement } from 'react';
import { serializeJsonLd, type JsonLd as JsonLdData } from '@/lib/jsonld';

export function JsonLd({ data }: { data: JsonLdData }): ReactElement {
  return (
    <script
      type="application/ld+json"
      dangerouslySetInnerHTML={{ __html: serializeJsonLd(data) }}
    />
  );
}
