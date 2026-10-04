import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { DeskStatusDto } from '../../../lib/ipc/types';
import { ControlsPage } from '../ControlsPage';

const desk: DeskStatusDto = {
  mode: 'buddy',
  volumePercent: null,
  muted: null,
  media: null,
  cpuPercent: null,
  ramPercent: null,
  highLoad: false,
  pressAction: 'playPause',
  holdAction: 'mute',
  doublePressAction: 'nextView',
  buttonActions: ['previousTrack', null, 'nextTrack'],
  mediaTitle: null,
  mediaArtist: null,
  lastAction: null,
};

describe('ControlsPage', () => {
  it('shows what each contextual button does, and says when one is not set', () => {
    render(<ControlsPage desk={desk} />);
    expect(screen.getByTestId('gesture-button1')).toHaveTextContent('Previous track');
    expect(screen.getByTestId('gesture-button2')).toHaveTextContent('Not set');
    expect(screen.getByTestId('gesture-button3')).toHaveTextContent('Next track');
  });
});
