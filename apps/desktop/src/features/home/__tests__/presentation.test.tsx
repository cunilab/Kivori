import { render, screen, within } from '@testing-library/react';
import axe from 'axe-core';
import { beforeEach, describe, expect, it } from 'vitest';
import { TooltipProvider } from '@kivori/ui/components/tooltip';
import { ProductArt } from '../../../components/kivori/product-art';
import { setDevMode } from '../../../lib/dev-mode';
import { mockDeskStatus } from '../../../lib/ipc/mock';
import { parseConnectionStatus } from '../../../lib/ipc/validate';
import { connectionFixture } from '../../../test/connection';
import { Welcome } from '../../onboarding/steps/Welcome';
import { Tour } from '../../onboarding/steps/Tour';
import { HomePage } from '../HomePage';

function fakeProduct() {
  const connection = connectionFixture(['productIdentityV1'], { connectionGeneration: 2 });
  connection.device!.identity!.product = 'Test dial';
  connection.device!.presentation = { displayName: 'Test dial', model: 'KV-T2' };
  return connection;
}

beforeEach(() => setDevMode(false));

describe('catalog product presentation', () => {
  it('uses a screenless product’s own labels and a neutral figure without borrowing another picture', async () => {
    const page = render(
      <TooltipProvider>
        <HomePage connection={fakeProduct()} desk={mockDeskStatus()} onNavigate={() => {}} />
      </TooltipProvider>,
    );
    const art = screen.getByRole('img', { name: 'Test dial · KV-T2' });
    expect(within(art).getByText('Test dial')).toBeInTheDocument();
    expect(within(art).getByText('KV-T2')).toBeInTheDocument();
    expect(within(art).getByTestId('product-art-neutral')).toBeInTheDocument();
    expect(screen.queryByText('Kivori Pip')).not.toBeInTheDocument();
    expect(screen.queryByTestId('buddy-mascot')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Change view' })).not.toBeInTheDocument();
    expect(
      (await axe.run(page.container, { rules: { 'color-contrast': { enabled: false } } }))
        .violations,
    ).toEqual([]);
  });

  it('never presents retained metadata while identity is pending, unknown or the connection ends', () => {
    const pending = fakeProduct();
    pending.device!.identity = null;
    const page = render(<ProductArt connection={pending} />);
    expect(screen.getByRole('img', { name: 'Kivori device' })).toBeInTheDocument();
    expect(screen.queryByText('Test dial')).not.toBeInTheDocument();
    const unknown = fakeProduct();
    unknown.device!.identity!.known = false;
    page.rerender(<ProductArt connection={unknown} />);
    expect(screen.queryByText('KV-T2')).not.toBeInTheDocument();
    page.rerender(<ProductArt connection={connectionFixture()} />);
    expect(screen.getByText('KV-P1')).toBeInTheDocument();
    page.rerender(<ProductArt connection={fakeProduct()} />);
    expect(screen.getByText('KV-T2')).toBeInTheDocument();
    expect(screen.queryByText('KV-P1')).not.toBeInTheDocument();
    page.rerender(<ProductArt connection={{ ...fakeProduct(), connection: 'disconnected' }} />);
    expect(screen.queryByText('Test dial')).not.toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'Kivori device' })).toBeInTheDocument();
  });

  it('keeps Welcome and Tour free of assumed hardware shapes and uses resolved metadata', () => {
    const page = render(<Welcome connection={null} />);
    expect(screen.getByRole('img', { name: 'Kivori device' })).toBeInTheDocument();
    expect(page.container.querySelector('[data-highlight]')).toBeNull();
    expect(screen.queryByTestId('buddy-mascot')).not.toBeInTheDocument();
    page.rerender(<Tour connection={connectionFixture()} done={0} connected />);
    expect(screen.getByRole('img', { name: 'Kivori Pip · KV-P1' })).toBeInTheDocument();
    expect(page.container.querySelector('[data-highlight]')).toBeNull();
    expect(screen.queryByTestId('buddy-mascot')).not.toBeInTheDocument();
  });

  it('validates presentation metadata and drops unsupported image fields at the IPC boundary', () => {
    const connection = fakeProduct();
    const parsed = parseConnectionStatus({
      ...connection,
      device: {
        ...connection.device,
        presentation: {
          displayName: 'Test dial',
          model: 'KV-T2',
          image: 'https://example.invalid/unbundled.png',
        },
      },
    });
    expect(parsed.device?.presentation).toEqual({ displayName: 'Test dial', model: 'KV-T2' });
    for (const presentation of [
      { displayName: 7, model: 'KV-T2' },
      { displayName: 'Dial', model: null },
      { displayName: 'Dial', model: 'X'.repeat(97) },
    ]) {
      expect(() =>
        parseConnectionStatus({ ...connection, device: { ...connection.device, presentation } }),
      ).toThrow();
    }
    expect(
      parseConnectionStatus({ ...connection, device: { ...connection.device, presentation: null } })
        .device?.presentation,
    ).toBeNull();
  });
});
