import type { ReactElement, ReactNode } from 'react';

// The four outcomes the device screen shows, in the screen's own colours (always on the panel navy).
const BADGES: { name: string; color: string; glyph: ReactNode; meaning: string }[] = [
  {
    name: 'Confirmed',
    color: 'var(--panel-green)',
    glyph: <path d="m5 12.5 4.5 4.5L19 7.5" />,
    meaning:
      'Kivori read the new state back from your computer. The volume really is where you set it.',
  },
  {
    name: 'Started',
    color: 'var(--panel-blue)',
    glyph: <path d="M5 12h13m-5-5 5 5-5 5" />,
    meaning:
      'The computer accepted it and started it, like launching an app, with no lasting state to check.',
  },
  {
    name: 'Unverified',
    color: 'var(--panel-amber)',
    glyph: <path d="M9.5 9a2.6 2.6 0 1 1 3.6 2.4c-.8.4-1.1 1-1.1 1.8M12 17h.01" />,
    meaning:
      'Sent, but your computer cannot say what happened. Keyboard shortcuts and media keys are always this.',
  },
  {
    name: 'Error',
    color: 'var(--panel-red)',
    glyph: <path d="m6.5 6.5 11 11m0-11-11 11" />,
    meaning: 'It did not work, and nothing was changed.',
  },
];

export function ConfirmationBadges(): ReactElement {
  return (
    <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      {BADGES.map((badge) => (
        <li key={badge.name} className="rounded-2xl bg-panel p-5 text-[#e8ecf5]">
          <span
            className="flex size-12 items-center justify-center rounded-full border-2"
            style={{ borderColor: badge.color, color: badge.color }}
          >
            <svg
              viewBox="0 0 24 24"
              width="26"
              height="26"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.4"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
            >
              {badge.glyph}
            </svg>
          </span>
          <h3 className="mt-4 font-display text-xl font-semibold" style={{ color: badge.color }}>
            {badge.name}
          </h3>
          <p className="mt-2 text-sm text-[#a9b3c9]">{badge.meaning}</p>
        </li>
      ))}
    </ul>
  );
}
