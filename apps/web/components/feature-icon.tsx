import {
  CheckIcon,
  CircleDotIcon,
  LayersIcon,
  LifeBuoyIcon,
  ListChecksIcon,
  MonitorIcon,
  ShieldCheckIcon,
  SmileIcon,
  WifiOffIcon,
  type LucideIcon,
} from 'lucide-react';
import type { ReactElement } from 'react';

const ICONS: Record<string, LucideIcon> = {
  layers: LayersIcon,
  'shield-check': ShieldCheckIcon,
  list: ListChecksIcon,
  monitor: MonitorIcon,
  smile: SmileIcon,
  'life-buoy': LifeBuoyIcon,
  'wifi-off': WifiOffIcon,
  knob: CircleDotIcon,
  check: CheckIcon,
};

/** Product feature icon (lucide), keyed by the name in the product registry. */
export function FeatureIcon({ name }: { name?: string | undefined }): ReactElement {
  const Icon = (name && ICONS[name]) || CheckIcon;
  return <Icon className="size-5" aria-hidden="true" />;
}
