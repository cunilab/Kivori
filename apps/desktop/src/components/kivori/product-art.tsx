import { useEffect, useState, type ReactElement } from 'react';
import { Usb } from 'lucide-react';
import { cn } from '@kivori/ui/lib/utils';
import type { ConnectionStatusDto } from '@/lib/ipc/types';
import { getProductArt } from '@/lib/ipc';
import { controlSupported } from '@/lib/device-support';
import { format, strings } from '@/lib/i18n/strings';
import { DeviceVisual, type DeviceControl } from './device-visual';

/** Current catalog artwork, or a neutral product figure. Never a device preview. */
export function ProductArt({
  connection,
  className,
  selectedControl,
  onSelectControl,
  profileName,
  showCaption = true,
}: {
  connection: ConnectionStatusDto | null;
  className?: string;
  selectedControl?: DeviceControl;
  onSelectControl?: (control: DeviceControl) => void;
  profileName?: string;
  showCaption?: boolean;
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
  const layout = presentation?.controlLayout === 'rotary-three-button';
  const interactive = layout && !!onSelectControl;
  const availableControls: DeviceControl[] = [
    ...(controlSupported(connection, 'rotate') ? (['knob'] as const) : []),
    ...(controlSupported(connection, 'button1') ? (['button1'] as const) : []),
    ...(controlSupported(connection, 'button2') ? (['button2'] as const) : []),
    ...(controlSupported(connection, 'button3') ? (['button3'] as const) : []),
  ];
  return (
    <div
      role={interactive ? 'group' : 'img'}
      aria-label={
        presentation
          ? format(strings.productArt.label, {
              product: presentation.displayName,
              model: presentation.model,
            })
          : strings.productArt.neutral
      }
      className={cn(
        'flex w-full flex-col items-center justify-center gap-4 rounded-2xl p-4',
        !layout &&
          'aspect-square bg-[radial-gradient(circle_at_50%_40%,var(--accent),var(--muted)_70%)]',
        className,
      )}
    >
      {layout ? (
        <DeviceVisual
          selectedControl={selectedControl}
          onSelectControl={onSelectControl}
          availableControls={availableControls}
          profileName={profileName}
        />
      ) : url ? (
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
      {presentation && showCaption ? (
        <div className="space-y-1 text-center">
          <p className="font-medium">{presentation.displayName}</p>
          <p className="text-sm text-muted-foreground">{presentation.model}</p>
        </div>
      ) : null}
    </div>
  );
}
