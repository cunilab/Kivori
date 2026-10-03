import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';

import { App } from '../App';

describe('App shell', () => {
  it('offers the product sections, plus Device Studio in the dev build', async () => {
    render(<App />);
    const nav = screen.getByRole('navigation', { name: 'Main navigation' });
    await waitFor(() =>
      expect(within(nav).getByRole('button', { name: 'Device Studio' })).toBeInTheDocument(),
    );
    expect(
      within(nav)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['Home', 'Controls', 'Display', 'Activity', 'Device', 'Device Studio']);
  });

  it('navigates between sections and marks the current one', async () => {
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByRole('heading', { level: 1, name: 'Home' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Controls' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Controls' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Controls' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    await user.click(screen.getByRole('button', { name: 'Activity' }));
    expect(screen.getByRole('heading', { level: 1, name: 'Activity' })).toBeInTheDocument();
  });

  it('shows the plug-in guidance when no Kivori is connected', async () => {
    render(<App />);
    expect(await screen.findByText('Plug in your Kivori')).toBeInTheDocument();
  });
});
