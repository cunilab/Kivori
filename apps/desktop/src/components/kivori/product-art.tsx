import type { ReactElement } from 'react';
import { Usb } from 'lucide-react';
import { cn } from '@kivori/ui/lib/utils';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';

/** A neutral product figure until catalog artwork delivery is available. Never a device preview. */
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
      <Usb
        className="size-16 text-muted-foreground"
        aria-hidden="true"
        data-testid="product-art-neutral"
      />
      {presentation ? (
        <figcaption className="space-y-1 text-center">
          <p className="font-medium">{presentation.displayName}</p>
          <p className="text-sm text-muted-foreground">{presentation.model}</p>
        </figcaption>
      ) : null}
    </figure>
  );
}
