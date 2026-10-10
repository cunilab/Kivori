'use client';

import { DownloadIcon } from 'lucide-react';
import { useEffect, useState, type ReactElement } from 'react';
import { Button } from '@kivori/ui/components/button';
import { detectOs, type DetectedOs } from '@/lib/os';

interface OsDownloadButtonProps {
  version: string;
  beta: boolean;
}

interface NavigatorWithHints extends Navigator {
  userAgentData?: { platform?: string };
}

/**
 * The big download button. It links straight to the installer route so the browser starts the download
 * with no extra page. Only Windows installers exist, so every visitor gets the Windows file; a Mac
 * visitor is told macOS is coming later.
 */
export function OsDownloadButton({ version, beta }: OsDownloadButtonProps): ReactElement {
  const [os, setOs] = useState<DetectedOs>('windows');
  useEffect(() => {
    const nav = navigator as NavigatorWithHints;
    setOs(detectOs(nav.userAgentData?.platform, nav.userAgent));
  }, []);

  return (
    <div className="flex flex-col items-center gap-4">
      <Button
        size="lg"
        className="h-14 rounded-full px-9 text-lg font-medium"
        nativeButton={false}
        render={<a href="/download/windows/latest" />}
      >
        <DownloadIcon className="size-5" aria-hidden="true" />
        Download for Windows
      </Button>
      <p className="text-sm text-muted-foreground">
        Version {version}
        {beta ? ' · Beta' : ''} &middot; for Windows 10 and 11
      </p>
      {os === 'macos' ? (
        <p className="text-sm text-muted-foreground">
          Looks like you are on a Mac. macOS is coming later.
        </p>
      ) : null}
      <a href="#platforms" className="text-sm font-medium text-primary hover:underline">
        Other platforms &rsaquo;
      </a>
    </div>
  );
}
