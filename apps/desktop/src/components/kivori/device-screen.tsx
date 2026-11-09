import type { ReactElement } from 'react';
import { mascotUrl } from '@/lib/brand';
import type { DeviceControl } from './device-visual';

/** The buddy is the home screen; control details live beside the illustration. */
export function DeviceScreen({
  activeControl,
}: {
  activeControl?: DeviceControl | undefined;
}): ReactElement {
  return (
    <div
      className="device-screen"
      data-testid="device-screen"
      data-reaction={activeControl ?? 'idle'}
    >
      <div className="device-screen__pose">
        <img
          src={mascotUrl}
          alt=""
          draggable={false}
          className="device-screen__mascot"
          data-testid="device-screen-mascot"
        />
      </div>
    </div>
  );
}
