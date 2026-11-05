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
 * `ProductStage` for consistent sizing.
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

/** The product render at a responsive size. */
export function ProductStage({
  image,
  large = false,
  priority = false,
  sizes,
  className,
}: ProductShotProps): ReactElement {
  return (
    <div className={cn('relative isolate mx-auto w-full', className)}>
      <ProductShot image={image} large={large} priority={priority} sizes={sizes} />
    </div>
  );
}
