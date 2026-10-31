import { describe, expect, it } from 'vitest';
import { recover, type RecoveryEvent, type RecoveryPhase } from '../accessibility-recovery';

const run = (start: RecoveryPhase, ...events: RecoveryEvent[]): RecoveryPhase =>
  events.reduce(recover, start);

describe('accessibility recovery', () => {
  it('stays quiet until the user has asked', () => {
    expect(run('fresh', { type: 'returned', state: 'missing' })).toBe('fresh');
    expect(run('fresh', { type: 'read', state: 'missing' })).toBe('fresh');
  });

  it('explains the stale approval when they come back and macOS still says missing', () => {
    expect(run('fresh', { type: 'asked' }, { type: 'returned', state: 'missing' })).toBe('stale');
  });

  it('does not raise it from the poll alone, only from the window coming back', () => {
    expect(run('fresh', { type: 'asked' }, { type: 'read', state: 'missing' })).toBe('tried');
  });

  it('clears once macOS reports it on (or not applicable)', () => {
    expect(run('tried', { type: 'returned', state: 'granted' })).toBe('fresh');
    expect(run('stale', { type: 'read', state: 'granted' })).toBe('fresh');
    expect(run('stale', { type: 'returned', state: 'notApplicable' })).toBe('fresh');
  });

  it('keeps explaining while it stays missing, and asking again does not move it back', () => {
    expect(
      run(
        'stale',
        { type: 'asked' },
        { type: 'returned', state: 'missing' },
        { type: 'read', state: 'missing' },
      ),
    ).toBe('stale');
  });

  it('a reset starts over, so a second explanation needs a second ask and return', () => {
    const afterReset = run('stale', { type: 'reset' });
    expect(afterReset).toBe('fresh');
    expect(run(afterReset, { type: 'returned', state: 'missing' })).toBe('fresh');
    expect(run(afterReset, { type: 'asked' }, { type: 'returned', state: 'missing' })).toBe(
      'stale',
    );
  });
});
