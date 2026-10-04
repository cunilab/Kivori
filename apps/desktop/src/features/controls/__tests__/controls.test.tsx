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
  profile: null,
  pinned: false,
  rotateLabel: 'Volume',
  buttonLabels: ['Previous', '', 'Next'],
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
    expect(screen.getByTestId('gesture-rotate')).toHaveTextContent('System volume');
    expect(screen.getByTestId('active-profile')).toHaveTextContent('General');
    expect(screen.getByTestId('active-profile')).not.toHaveTextContent('Pinned');
  });

  it('names the active profile, the pin, the knob and the shortcut buttons', () => {
    render(
      <ControlsPage
        desk={{
          ...desk,
          profile: 'Browser',
          pinned: true,
          rotateLabel: 'Tabs',
          buttonActions: ['shortcut', 'shortcut', 'shortcut'],
          buttonLabels: ['Back', 'Reload', 'New tab'],
        }}
      />,
    );
    expect(screen.getByTestId('active-profile')).toHaveTextContent('Browser');
    expect(screen.getByTestId('active-profile')).toHaveTextContent('Pinned on the device');
    expect(screen.getByTestId('gesture-rotate')).toHaveTextContent('Tabs');
    expect(screen.getByTestId('gesture-button2')).toHaveTextContent('Reload');
  });

  it('says when a control is suspended in a protected window', () => {
    render(
      <ControlsPage
        desk={{
          ...desk,
          profile: 'Protected',
          rotateLabel: '',
          buttonActions: ['shortcut', null, 'previousTrack'],
          buttonLabels: ['', '', 'Previous'],
        }}
      />,
    );
    expect(screen.getByTestId('active-profile')).toHaveTextContent('Protected');
    expect(screen.getByTestId('gesture-rotate')).toHaveTextContent('Paused in a protected window');
    expect(screen.getByTestId('gesture-button1')).toHaveTextContent('Paused in a protected window');
    expect(screen.getByTestId('gesture-button2')).toHaveTextContent('Not set');
  });
});
