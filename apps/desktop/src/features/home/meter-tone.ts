export type MeterTone = 'normal' | 'warning' | 'danger';

/** Warning from 75%, danger from 90%. `danger` forces the top tone (the device's own high-load flag). */
export function meterTone(value: number | null | undefined, danger = false): MeterTone {
  if (danger) return 'danger';
  if (value === null || value === undefined) return 'normal';
  return value >= 90 ? 'danger' : value >= 75 ? 'warning' : 'normal';
}

export const TONE_TEXT: Record<MeterTone, string> = {
  normal: '',
  warning: 'text-warning',
  danger: 'text-destructive',
};

export const TONE_BAR: Record<MeterTone, string> = {
  normal: '',
  warning: '[&_[data-slot=progress-indicator]]:bg-warning',
  danger: '[&_[data-slot=progress-indicator]]:bg-destructive',
};
