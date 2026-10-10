import Image from 'next/image';
import type { ReactElement } from 'react';
import { cn } from '@kivori/ui/lib/utils';
import { smallRender, type MediaImage } from '@/lib/media';

interface ProductShotProps {
  image: MediaImage;
  /** Use the full 2400 px file (hero shots); otherwise the 1200 px twin. */
  large?: boolean;
  priority?: boolean;
  sizes?: string;
  className?: string;
}

/**
 * A studio render from public/media. The renders are shot on white, so the image multiplies onto light
 * surfaces; in dark mode it sits on a white stage (see `.shot` in globals.css).
 */
export function ProductShot({
  image,
  large = false,
  priority = false,
  sizes = '(min-width: 1200px) 1200px, 100vw',
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
      className={cn('shot h-auto w-full', className)}
    />
  );
}
