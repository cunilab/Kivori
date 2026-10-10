import type { ReactElement } from 'react';
import { renderMarkdown } from '@/lib/markdown';

/** Release notes. The HTML is produced by `renderMarkdown`, which never emits raw HTML or script URLs. */
export function Markdown({ source }: { source: string }): ReactElement {
  return (
    <div
      className="space-y-3 text-sm leading-relaxed [&_a]:text-primary [&_a]:underline [&_code]:rounded [&_code]:bg-secondary [&_code]:px-1 [&_h1]:text-lg [&_h1]:font-semibold [&_h2]:mt-4 [&_h2]:text-base [&_h2]:font-semibold [&_h3]:mt-3 [&_h3]:font-medium [&_img]:max-w-full [&_ol]:list-decimal [&_ol]:pl-5 [&_ul]:list-disc [&_ul]:pl-5"
      dangerouslySetInnerHTML={{ __html: renderMarkdown(source) }}
    />
  );
}
