import { useEffect, useState } from 'react';
import {
  getAppInfo,
  getConnectionStatus,
  getDeskStatus,
  onConnectionStatus,
  onDeskStatus,
  type Unlisten,
} from '@/lib/ipc';
import type { AppInfoDto, ConnectionStatusDto, DeskStatusDto } from '@/lib/ipc/types';

/**
 * Seeds from a snapshot, then follows the native event. A pushed event always wins over a late
 * snapshot, and nothing is applied after unmount. `null` means "not known yet" (render a skeleton).
 */
function useNative<T>(
  snapshot: () => Promise<T>,
  subscribe: (handler: (value: T) => void) => Promise<Unlisten>,
): T | null {
  const [value, setValue] = useState<T | null>(null);
  useEffect(() => {
    let active = true;
    let unlisten: Unlisten = () => {};
    void snapshot()
      .then((first) => {
        if (active) setValue((current) => current ?? first);
      })
      .catch(() => {});
    void subscribe((next) => {
      if (active) setValue(next);
    })
      .then((handle) => {
        if (active) unlisten = handle;
        else handle();
      })
      .catch(() => {});
    return () => {
      active = false;
      unlisten();
    };
  }, [snapshot, subscribe]);
  return value;
}

export function useConnectionStatus(): ConnectionStatusDto | null {
  return useNative(getConnectionStatus, onConnectionStatus);
}

export function useDeskStatus(): DeskStatusDto | null {
  return useNative(getDeskStatus, onDeskStatus);
}

export function useAppInfo(): AppInfoDto | null {
  const [info, setInfo] = useState<AppInfoDto | null>(null);
  useEffect(() => {
    let active = true;
    void getAppInfo()
      .then((next) => {
        if (active) setInfo(next);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);
  return info;
}

/// The UI connection states (derived from the connection axis + retry count, FR-008).
export type UiConnection =
  'loading' | 'disconnected' | 'connecting' | 'connected' | 'reconnecting' | 'incompatible';

export function uiConnection(status: ConnectionStatusDto | null): UiConnection {
  if (!status) return 'loading';
  switch (status.connection) {
    case 'connected':
      return 'connected';
    case 'connecting':
      return status.retryCount > 0 ? 'reconnecting' : 'connecting';
    case 'incompatible':
      return 'incompatible';
    case 'error':
      return 'reconnecting';
    default:
      return status.retryCount > 0 ? 'reconnecting' : 'disconnected';
  }
}
