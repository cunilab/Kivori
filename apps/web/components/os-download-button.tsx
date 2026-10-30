'use client';

import { DownloadIcon } from 'lucide-react';
import { useEffect, useState, type ReactElement } from 'react';
import { Button } from '@kivori/ui/components/button';
import { detectOs, type DetectedOs } from '@/lib/os';

interface OsDownloadButtonProps {
  version: string;
  beta: boolean;
  macAvailable?: boolean;
  windowsAvailable?: boolean;
}

interface NavigatorWithHints extends Navigator {
  userAgentData?: { platform?: string };
}

/**
 * The big download button. It links straight to the installer route so the browser starts the download
 * with no extra page. Mac visitors are never sent a Windows installer.
 */
export function OsDownloadButton({
  version,
  beta,
  macAvailable = false,
  windowsAvailable = true,
}: OsDownloadButtonProps): ReactElement {
  const [os, setOs] = useState<DetectedOs | null>(null);
  useEffect(() => {
    const nav = navigator as NavigatorWithHints;
    setOs(detectOs(nav.userAgentData?.platform, nav.userAgent));
  }, []);

  const available = os === 'macos' ? macAvailable : os === 'windows' && windowsAvailable;
  const label = os === 'macos' ? 'macOS' : 'Windows';

  return (
    <div className="flex flex-col items-center gap-4">
      <Button
        size="lg"
        className="keycap h-14 px-9 text-lg"
        nativeButton={false}
        render={<a href={available ? `/download/${os}/latest` : '#platforms'} />}
      >
        <DownloadIcon className="size-5" aria-hidden="true" />
        {available ? `Download for ${label}` : 'Choose your platform'}
      </Button>
      <p className="text-sm text-muted-foreground">
        Version {version}
        {beta ? ' · Beta' : ''} &middot;{' '}
        {os === 'macos'
          ? 'for Apple Silicon Macs'
          : os === 'windows'
            ? 'for Windows 10 and 11'
            : 'Windows and Apple Silicon macOS'}
      </p>
      {os === 'macos' ? (
        <p className="text-sm text-muted-foreground">
          {macAvailable
            ? 'Apple Silicon build. Intel Macs are not included.'
            : 'Mac build not published yet.'}
        </p>
      ) : null}
      <a href="#platforms" className="text-sm font-medium text-primary hover:underline">
        Other platforms &rsaquo;
      </a>
    </div>
  );
}
