import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '@kivori/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@kivori/ui/components/card';
import { isTauri } from '@/lib/ipc';

type Status = 'notApplicable' | 'disabled' | 'starting' | 'enabled' | 'denied' | 'disconnected';

export function KeyboardShortcutsStatus({
  hideWhenEnabled = false,
}: {
  hideWhenEnabled?: boolean;
}): React.JSX.Element | null {
  const [status, setStatus] = useState<Status>('notApplicable');
  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    const refresh = (): void => {
      void invoke<Status>('keyboard_shortcuts_status')
        .then((value) => {
          if (active) setStatus(value);
        })
        .catch(() => {});
    };
    refresh();
    const timer = setInterval(refresh, 1000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, []);
  if (status === 'notApplicable' || (hideWhenEnabled && status === 'enabled')) return null;
  const message: Record<Exclude<Status, 'notApplicable'>, string> = {
    disabled:
      'To use your shortcut controls, enable keyboard access and approve the system prompt.',
    starting: 'Choose Allow in the system prompt to finish setup.',
    enabled: 'Keyboard shortcuts are enabled.',
    denied:
      'Keyboard access was not granted. Select Enable to try again, then approve the system prompt.',
    disconnected: 'Keyboard access is unavailable or was disconnected. You can try again.',
  };
  return (
    <Card size="sm">
      <CardHeader>
        <CardTitle>Keyboard shortcuts</CardTitle>
        <CardDescription>{message[status]}</CardDescription>
      </CardHeader>
      {status !== 'enabled' && (
        <CardContent className="space-y-2">
          <Button
            size="sm"
            disabled={status === 'starting'}
            onClick={() => {
              setStatus('starting');
              void invoke('enable_keyboard_shortcuts').catch(() => setStatus('disconnected'));
            }}
          >
            Enable keyboard shortcuts
          </Button>
          <p className="text-xs text-muted-foreground">
            You'll need to enable this again after restarting Kivori.
          </p>
        </CardContent>
      )}
    </Card>
  );
}
