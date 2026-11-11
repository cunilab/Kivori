import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Toaster } from '@kivori/ui/components/sonner';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import { useConfig, useConnectionStatus, useUpdateAvailable } from '@/hooks/use-kivori';
import * as ipc from '@/lib/ipc';
import { PROFILE_LIMITS } from '@/lib/ipc/types';
import { format, strings } from '@/lib/i18n/strings';
import { Onboarding } from '../Onboarding';
import { USE_CASES } from '../steps/UseCase';

function Harness({ onComplete }: { onComplete: () => Promise<void> }): ReactElement {
  const connection = useConnectionStatus();
  const config = useConfig();
  const updateAvailable = useUpdateAvailable(connection);
  return (
    <TooltipProvider>
      <Onboarding
        connection={connection}
        config={config}
        updateAvailable={updateAvailable}
        onComplete={onComplete}
      />
      <Toaster />
    </TooltipProvider>
  );
}

const useCase = (key: (typeof USE_CASES)[number]['key']): (typeof USE_CASES)[number] =>
  USE_CASES.find((item) => item.key === key)!;

/** Connected setup, stopped on the use-case step (after the tour, before customize). */
async function openUseCase(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  history.replaceState({}, '', '/?mock=connected&onboarding');
  render(<Harness onComplete={() => Promise.resolve()} />);
  await user.click(await screen.findByRole('button', { name: 'Get started' }));
  await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());
  await user.click(screen.getByRole('button', { name: 'Continue' }));
  expect(
    await screen.findByRole('heading', { level: 1, name: 'Try your Kivori' }),
  ).toBeInTheDocument();
  expect(screen.getByText('Step 3 of 6')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Continue' }));
  expect(
    await screen.findByRole('heading', { level: 1, name: 'How will you use Kivori?' }),
  ).toBeInTheDocument();
  await waitFor(() => expect(screen.getByTestId('usecase-everyday')).toBeEnabled());
}

beforeEach(async () => {
  localStorage.clear();
  await ipc.resetConfig();
});
afterEach(() => {
  history.replaceState({}, '', '/');
  vi.restoreAllMocks();
});

describe('use-case step', () => {
  it('sits after the tour and before customize, and the progress total is one higher', async () => {
    const user = userEvent.setup();
    await openUseCase(user);
    expect(screen.getByText('Step 4 of 6')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Skip this step' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled();

    const config = await ipc.getConfig();
    for (const item of USE_CASES) {
      const card = screen.getByTestId(`usecase-${item.key}`);
      expect(card).toHaveTextContent(item.name);
      const listed = item.profiles
        .map((id) => config.profiles.find((profile) => profile.id === id)?.name)
        .join(', ');
      expect(card).toHaveTextContent(listed);
    }
    const names = config.profiles
      .filter((profile) => profile.inPinCycle)
      .map((profile) => profile.name)
      .join(', ');
    expect(
      screen.getByText(format(strings.onboarding.usecase.onDevice, { names })),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Continue' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Make it yours' }),
    ).toBeInTheDocument();
    expect(screen.getByText('Step 5 of 6')).toBeInTheDocument();
  });

  it('turns on only the profiles in the card that are not on the device yet', async () => {
    const user = userEvent.setup();
    const coding = useCase('coding');
    const before = await ipc.getConfig();
    const already = coding.profiles.filter((id) =>
      before.profiles.some((profile) => profile.id === id && profile.inPinCycle),
    );
    const missing = coding.profiles.filter((id) => !already.includes(id));
    expect(already.length).toBeGreaterThan(0);
    expect(missing.length).toBeGreaterThan(0);

    await openUseCase(user);
    const pin = vi.spyOn(ipc, 'setProfilePinCycle');
    await user.click(screen.getByTestId('usecase-coding'));
    await waitFor(() => expect(pin).toHaveBeenCalledTimes(missing.length));
    expect(pin.mock.calls.map((call) => call[0])).toEqual([...missing]);
    expect(pin.mock.calls.every((call) => call[1] === true)).toBe(true);
    await waitFor(() =>
      expect(screen.getByTestId('usecase-coding')).toHaveAttribute('aria-pressed', 'true'),
    );
  });

  it('shows a card pressed when every profile is on, and turns them off', async () => {
    const user = userEvent.setup();
    const everyday = useCase('everyday');
    const before = await ipc.getConfig();
    expect(
      everyday.profiles.every((id) =>
        before.profiles.some((profile) => profile.id === id && profile.inPinCycle),
      ),
    ).toBe(true);

    await openUseCase(user);
    const card = screen.getByTestId('usecase-everyday');
    expect(card).toHaveAttribute('aria-pressed', 'true');
    const pin = vi.spyOn(ipc, 'setProfilePinCycle');
    await user.click(card);
    await waitFor(() => expect(pin).toHaveBeenCalledTimes(everyday.profiles.length));
    expect(pin.mock.calls).toEqual(everyday.profiles.map((id) => [id, false]));
    await waitFor(() => expect(card).toHaveAttribute('aria-pressed', 'false'));
  });

  it('does nothing and explains the limit when the device cannot hold the card', async () => {
    const video = useCase('video');
    const videoIds = new Set<string>(video.profiles);
    let config = await ipc.getConfig();
    const adding = video.profiles.filter(
      (id) => !config.profiles.some((profile) => profile.id === id && profile.inPinCycle),
    ).length;
    for (const profile of config.profiles) {
      const pinned = config.profiles.filter((item) => item.inPinCycle).length;
      if (pinned + adding > PROFILE_LIMITS.pinned) break;
      if (profile.inPinCycle || videoIds.has(profile.id)) continue;
      config = await ipc.setProfilePinCycle(profile.id, true);
    }
    const pinned = config.profiles.filter((profile) => profile.inPinCycle).length;
    expect(pinned + adding).toBeGreaterThan(PROFILE_LIMITS.pinned);

    const user = userEvent.setup();
    await openUseCase(user);
    const pin = vi.spyOn(ipc, 'setProfilePinCycle');
    const card = screen.getByTestId('usecase-video');
    expect(card).toHaveAttribute('aria-pressed', 'false');
    await user.click(card);
    expect(pin).not.toHaveBeenCalled();
    expect(card).toHaveAttribute('aria-pressed', 'false');
    expect(
      await screen.findByText(
        format(strings.onboarding.usecase.limit, { max: PROFILE_LIMITS.pinned }),
      ),
    ).toBeInTheDocument();
  });
});

describe('buddy personality', () => {
  it('saves the chosen intensity and marks that card', async () => {
    const user = userEvent.setup();
    await openUseCase(user);
    await user.click(screen.getByRole('button', { name: 'Continue' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Make it yours' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Buddy personality' })).toBeInTheDocument();
    expect(screen.getByTestId('personality-normal')).toHaveAttribute('aria-checked', 'true');

    const save = vi.spyOn(ipc, 'setBuddySettings');
    const playful = screen.getByTestId('personality-high');
    expect(playful).toHaveAttribute('aria-checked', 'false');
    await user.click(playful);
    await waitFor(() => expect(playful).toHaveAttribute('aria-checked', 'true'));
    expect(save).toHaveBeenCalledWith(true, 'high');

    playful.focus();
    await user.keyboard('{ArrowLeft}');
    await waitFor(() =>
      expect(screen.getByTestId('personality-normal')).toHaveAttribute('aria-checked', 'true'),
    );
    expect(save).toHaveBeenLastCalledWith(true, 'normal');
  });
});
