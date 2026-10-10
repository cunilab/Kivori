import type { ReactElement } from 'react';
import { notesFor } from '@/lib/release-notes';

/** Curated, user-facing notes for one version (or "Improvements and fixes."). */
export function ReleaseNotes({ version }: { version: string }): ReactElement {
  return (
    <div className="space-y-3 pb-2 text-base text-muted-foreground">
      {notesFor(version).map((block, index) =>
        block.kind === 'paragraph' ? (
          <p key={index}>{block.text}</p>
        ) : (
          <ul key={index} className="list-disc space-y-1.5 pl-5 marker:text-primary">
            {block.items.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        ),
      )}
    </div>
  );
}
