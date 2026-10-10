'use client';

import { useState, type ReactElement } from 'react';
import { BTN_SECONDARY } from '@/lib/ui';

export function CopyButton({ value, label }: { value: string; label: string }): ReactElement {
  const [copied, setCopied] = useState(false);

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard blocked: the hash stays selectable in the page
    }
  }

  return (
    <button
      type="button"
      onClick={() => void copy()}
      aria-label={label}
      className={`${BTN_SECONDARY} px-3 py-1 text-xs`}
    >
      <span aria-live="polite">{copied ? 'Copied' : 'Copy'}</span>
    </button>
  );
}
