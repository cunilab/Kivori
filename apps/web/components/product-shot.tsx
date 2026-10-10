import Image from 'next/image';
import type { ReactElement } from 'react';
import { cn } from '@kivori/ui/lib/utils';
import { smallRender, type MediaImage } from '@/lib/media';

interface ProductShotProps {
  image: MediaImage;
  /** Use the full 2400 px file (hero shots); otherwise the 1200 px twin. */
  large?: boolean;
  priority?: boolean;
  sizes?: string | undefined;
  className?: string;
}

/**
 * A studio render from public/media. Use the transparent hero render on the dark stage, inside a
 * `ProductStage` so the mint glow sits under it.
 */
export function ProductShot({
  image,
  large = false,
  priority = false,
  sizes = '(min-width: 1180px) 1180px, 100vw',
  className,
}: ProductShotProps): ReactElement {
  const shown = large ? image : smallRender(image);
  return (
    <Image
      src={shown.src}
      alt={image.alt}
      width={shown.width}
      height={shown.height}
      sizes={sizes}
      priority={priority}
      loading={priority ? 'eager' : 'lazy'}
      unoptimized
      className={cn('relative h-auto w-full', className)}
    />
  );
}

/** The transparent device render lit from below by a soft mint glow. */
export function ProductStage({
  image,
  large = false,
  priority = false,
  sizes,
  className,
}: ProductShotProps): ReactElement {
  return (
    <div className={cn('relative isolate mx-auto w-full', className)}>
      <div
        aria-hidden="true"
        className="glow stage-glow absolute inset-x-0 -top-[6%] -bottom-[14%] -z-10 blur-xl"
      />
      <ProductShot image={image} large={large} priority={priority} sizes={sizes} />
    </div>
  );
}
