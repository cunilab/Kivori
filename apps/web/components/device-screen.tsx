import Image from 'next/image';
import type { ReactElement } from 'react';
import { cn } from '@kivori/ui/lib/utils';
import type { MediaImage } from '@/lib/media';

interface DeviceScreenProps {
  image: MediaImage;
  /** Mint glow behind the bezel, for dark sections. */
  glow?: boolean;
  priority?: boolean;
  className?: string;
}

/** A real screen frame inside a bezel, optionally lit by a soft green glow. */
export function DeviceScreen({
  image,
  glow = false,
  priority = false,
  className,
}: DeviceScreenProps): ReactElement {
  return (
    <div className={cn('relative isolate mx-auto w-full max-w-md', className)}>
      {glow ? (
        <div
          aria-hidden="true"
          className="glow absolute -inset-10 -z-10 rounded-full bg-[radial-gradient(closest-side,rgb(125_242_196/0.45),transparent)] blur-2xl"
        />
      ) : null}
      <div className="rounded-[2.25rem] bg-linear-to-b from-[#222a40] to-[#0a0d17] p-3 shadow-[0_30px_80px_-20px_rgb(0_0_0/0.7)] ring-1 ring-white/10 sm:p-4">
        <Image
          src={image.src}
          alt={image.alt}
          width={image.width}
          height={image.height}
          priority={priority}
          unoptimized
          sizes="(min-width: 640px) 448px, 90vw"
          className="aspect-square w-full rounded-[1.5rem] bg-[#0c101c]"
        />
      </div>
    </div>
  );
}
