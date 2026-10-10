import type { ReactElement } from 'react';

/** The keycap buddy as a tiny CSS mark: a mint key with two dark eyes. Decorative. */
export function BrandMark({ size = 22 }: { size?: number }): ReactElement {
  const eye = 'absolute rounded-[2px] bg-stage';
  return (
    <span
      aria-hidden="true"
      className="relative inline-block rounded-md bg-mint shadow-[inset_0_-4px_0_var(--mint-deep)]"
      style={{ width: size, height: size }}
    >
      <span
        className={eye}
        style={{ left: size * 0.27, top: size * 0.32, width: size * 0.14, height: size * 0.27 }}
      />
      <span
        className={eye}
        style={{ right: size * 0.27, top: size * 0.32, width: size * 0.14, height: size * 0.27 }}
      />
    </span>
  );
}
