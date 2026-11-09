import type { ReactElement, ReactNode } from 'react';
import { cn } from '@kivori/ui/lib/utils';
import { format, strings } from '@/lib/i18n/strings';
import { DeviceScreen } from './device-screen';
import './device-visual.css';

export type DeviceControl = 'knob' | 'button1' | 'button2' | 'button3';

const controls: DeviceControl[] = ['knob', 'button1', 'button2', 'button3'];

/** Catalog-declared physical layout. Interactions select controls; they never run device actions. */
export function DeviceVisual({
  selectedControl,
  onSelectControl,
  onPreviewControl,
  availableControls = controls,
  className,
  screen,
}: {
  selectedControl?: DeviceControl | undefined;
  onSelectControl?: ((control: DeviceControl) => void) | undefined;
  /** Pointer or keyboard focus is on a control (`undefined` = left it). Shows it on the screen. */
  onPreviewControl?: ((control: DeviceControl | undefined) => void) | undefined;
  availableControls?: DeviceControl[] | undefined;
  className?: string | undefined;
  /** What the display shows; the Kivori name and the time when omitted. */
  screen?: ReactNode | undefined;
}): ReactElement {
  const names: Record<DeviceControl, string> = {
    knob: strings.controls.groups.knob,
    button1: strings.controls.gestures.button1.name,
    button2: strings.controls.gestures.button2.name,
    button3: strings.controls.gestures.button3.name,
  };
  const button = (control: DeviceControl): ReactElement => {
    const className = control === 'knob' ? 'device-visual__knob' : 'device-visual__key';
    const contents = control === 'knob' ? <span className="device-visual__dial" /> : null;
    return onSelectControl ? (
      <button
        key={control}
        type="button"
        className={className}
        aria-label={format(strings.controls.selectControl, { control: names[control] })}
        aria-pressed={selectedControl === control}
        disabled={!availableControls.includes(control)}
        data-selected={selectedControl === control}
        onClick={() => onSelectControl(control)}
        onPointerEnter={() => onPreviewControl?.(control)}
        onPointerLeave={() => onPreviewControl?.(undefined)}
        onFocus={() => onPreviewControl?.(control)}
        onBlur={() => onPreviewControl?.(undefined)}
      >
        {contents}
      </button>
    ) : (
      <div key={control} className={className} aria-hidden="true">
        {contents}
      </div>
    );
  };

  return (
    <div className={cn('device-visual', className)} data-testid="device-visual">
      <div className="device-visual__well">{button('knob')}</div>
      <div className="device-visual__visor" aria-hidden="true" />
      <div className="device-visual__bezel" aria-hidden="true">
        <div className="device-visual__screen">{screen ?? <DeviceScreen />}</div>
      </div>
      <div className="device-visual__keys">{controls.slice(1).map(button)}</div>
    </div>
  );
}
