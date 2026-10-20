import { describe, expect, it } from 'vitest';
import { meterTone } from '../meter-tone';

describe('meterTone', () => {
  it('is normal below 75, warning from 75, danger from 90', () => {
    expect(meterTone(0)).toBe('normal');
    expect(meterTone(74)).toBe('normal');
    expect(meterTone(75)).toBe('warning');
    expect(meterTone(89)).toBe('warning');
    expect(meterTone(90)).toBe('danger');
    expect(meterTone(100)).toBe('danger');
  });

  it('treats unknown as normal and lets the high-load flag force danger', () => {
    expect(meterTone(null)).toBe('normal');
    expect(meterTone(undefined)).toBe('normal');
    expect(meterTone(20, true)).toBe('danger');
  });
});
