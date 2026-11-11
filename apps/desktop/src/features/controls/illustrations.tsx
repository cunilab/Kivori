import type { ReactElement } from 'react';

/** A desk with nothing matched to this profile. */
export function NoAppsArt(): ReactElement {
  return (
    <svg aria-hidden="true" viewBox="0 0 48 48" className="size-12 shrink-0 text-muted-foreground">
      <rect
        x="7"
        y="8"
        width="34"
        height="22"
        rx="3"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <path
        d="M16 30v5M32 30v5M12 35h24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <circle
        cx="24"
        cy="19"
        r="3.5"
        className="text-primary"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
      />
    </svg>
  );
}

/** A blank card for a profile that does not exist yet. */
export function NewProfileArt(): ReactElement {
  return (
    <svg aria-hidden="true" viewBox="0 0 48 48" className="size-12 shrink-0 text-muted-foreground">
      <rect
        x="8"
        y="10"
        width="32"
        height="28"
        rx="4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeDasharray="3 3"
      />
      <path
        d="M24 19v10M19 24h10"
        className="text-primary"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}
