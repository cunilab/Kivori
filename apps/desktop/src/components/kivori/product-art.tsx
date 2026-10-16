import { useEffect, useState, type ReactElement } from 'react';
import { Usb } from 'lucide-react';
import { cn } from '@kivori/ui/lib/utils';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { getProductArt } from '@/lib/ipc';
import { format, strings } from '@/lib/i18n/strings';

/** Current catalog artwork, or a neutral product figure. Never a device preview. */
export function ProductArt({
  connection,
  className,
}: {
  connection: ConnectionStatusDto | null;
  className?: string;
}): ReactElement {
  const presentation =
    connection?.connection === 'connected' && connection.device?.identity?.known
      ? connection.device.presentation
      : null;
  const art = presentation?.art ?? null;
  const [loaded, setLoaded] = useState<{ art: string; url: string | null } | null>(null);
  useEffect(() => {
    let active = true;
    if (art)
      void getProductArt(art).then((url) => {
        if (active) setLoaded({ art, url });
      });
    return () => {
      active = false;
    };
  }, [art]);
  // A late lookup for another product can never appear in this session's figure.
  const url = art && loaded?.art === art ? loaded.url : null;
  return (
    <figure
      role="img"
      aria-label={
        presentation
          ? format(strings.productArt.label, {
              product: presentation.displayName,
              model: presentation.model,
            })
          : strings.productArt.neutral
      }
      className={cn(
        'flex aspect-square w-full flex-col items-center justify-center gap-4 rounded-2xl bg-muted p-4',
        className,
      )}
    >
      {url ? (
        <img
          key={art}
          src={url}
          alt=""
          className="h-48 w-full object-contain"
          onError={() => {
            if (art) setLoaded({ art, url: null });
          }}
        />
      ) : (
        <Usb
          className="size-16 text-muted-foreground"
          aria-hidden="true"
          data-testid="product-art-neutral"
        />
      )}
      {presentation ? (
        <figcaption className="space-y-1 text-center">
          <p className="font-medium">{presentation.displayName}</p>
          <p className="text-sm text-muted-foreground">{presentation.model}</p>
        </figcaption>
      ) : null}
    </figure>
  );
}
