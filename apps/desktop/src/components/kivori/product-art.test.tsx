import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { connectionFixture } from '@/test/connection';
import css from './device-visual.css?raw';
import { DeviceScreen } from './device-screen';
import { ProductArt } from './product-art';

const ipc = vi.hoisted(() => ({ getProductArt: vi.fn() }));
vi.mock('@/lib/ipc', () => ipc);

function product(art: string | null) {
  const connection = connectionFixture();
  connection.device!.presentation = {
    displayName: `Product ${art}`,
    model: 'KV-T1',
    art,
    controlLayout: null,
  };
  return connection;
}

function deferred() {
  let resolve!: (url: string | null) => void;
  const promise = new Promise<string | null>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => ipc.getProductArt.mockReset());

describe('bundled product artwork', () => {
  it('selects only supported controls for a catalog-declared physical layout', async () => {
    const user = userEvent.setup();
    const connection = connectionFixture(['physicalInputV1', 'buttonInputV1']);
    connection.device!.presentation!.controlLayout = 'rotary-three-button';
    const select = vi.fn();
    render(<ProductArt connection={connection} selectedControl="knob" onSelectControl={select} />);

    const knob = screen.getByRole('button', { name: 'Select Knob' });
    expect(knob).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Select Left button' })).toBeDisabled();
    knob.focus();
    await user.keyboard('{Enter}');
    expect(select).toHaveBeenCalledOnce();
    expect(select).toHaveBeenCalledWith('knob');
  });

  it('does not draw a declared layout for an unknown or disconnected product', () => {
    const connection = connectionFixture();
    connection.device!.presentation!.controlLayout = 'rotary-three-button';
    const page = render(<ProductArt connection={connection} />);
    expect(screen.getByTestId('device-visual')).toBeInTheDocument();
    connection.device!.identity!.known = false;
    page.rerender(<ProductArt connection={connection} />);
    expect(screen.queryByTestId('device-visual')).not.toBeInTheDocument();
    expect(screen.getByTestId('product-art-neutral')).toBeInTheDocument();
    connection.device!.identity!.known = true;
    page.rerender(<ProductArt connection={{ ...connection, connection: 'disconnected' }} />);
    expect(screen.queryByTestId('device-visual')).not.toBeInTheDocument();
  });

  it('does not request art without a current known product and an art key', () => {
    const page = render(<ProductArt connection={product(null)} />);
    expect(screen.getByTestId('product-art-neutral')).toBeInTheDocument();
    const unknown = product('a');
    unknown.device!.identity!.known = false;
    page.rerender(<ProductArt connection={unknown} />);
    page.rerender(<ProductArt connection={{ ...product('a'), connection: 'disconnected' }} />);
    expect(ipc.getProductArt).not.toHaveBeenCalled();
  });

  it('keeps B’s image when A resolves late and goes neutral on disconnect', async () => {
    const a = deferred();
    const b = deferred();
    ipc.getProductArt.mockImplementation((key: string) => (key === 'a' ? a.promise : b.promise));
    const page = render(<ProductArt connection={product('a')} />);
    page.rerender(<ProductArt connection={product('b')} />);
    await act(async () => b.resolve('data:image/png;base64,Qg=='));
    expect(page.container.querySelector('img')).toHaveAttribute(
      'src',
      'data:image/png;base64,Qg==',
    );
    await act(async () => a.resolve('data:image/png;base64,QQ=='));
    expect(page.container.querySelector('img')).toHaveAttribute(
      'src',
      'data:image/png;base64,Qg==',
    );
    expect(screen.getByRole('img', { name: 'Product b · KV-T1' })).toBeInTheDocument();
    page.rerender(<ProductArt connection={{ ...product('b'), connection: 'disconnected' }} />);
    expect(page.container.querySelector('img')).toBeNull();
    expect(screen.getByTestId('product-art-neutral')).toBeInTheDocument();
  });

  it('hides the previous image immediately while the next one loads', async () => {
    ipc.getProductArt
      .mockResolvedValueOnce('data:image/png;base64,QQ==')
      .mockReturnValueOnce(new Promise(() => {}));
    const page = render(<ProductArt connection={product('a')} />);
    await act(async () => {});
    expect(page.container.querySelector('img')).not.toBeNull();
    page.rerender(<ProductArt connection={product('b')} />);
    expect(page.container.querySelector('img')).toBeNull();
    expect(screen.getByTestId('product-art-neutral')).toBeInTheDocument();
  });

  it('falls back for absent art and browser image errors without losing catalog captions', async () => {
    ipc.getProductArt
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce('data:image/png;base64,QQ==');
    const page = render(<ProductArt connection={product('missing')} />);
    await act(async () => {});
    expect(screen.getByTestId('product-art-neutral')).toBeInTheDocument();
    page.rerender(<ProductArt connection={product('a')} />);
    await act(async () => {});
    fireEvent.error(page.container.querySelector('img')!);
    expect(screen.getByTestId('product-art-neutral')).toBeInTheDocument();
    expect(screen.getByText('Product a')).toBeInTheDocument();
    expect(screen.getByText('KV-T1')).toBeInTheDocument();
  });

  it('draws a default screen when the caller gives none, and a custom one when it does', () => {
    const connection = connectionFixture();
    connection.device!.presentation!.controlLayout = 'rotary-three-button';
    const page = render(<ProductArt connection={connection} />);
    expect(screen.getByTestId('device-screen')).toHaveTextContent('Kivori');
    expect(screen.getByTestId('device-screen')).toHaveTextContent(/\d{2}:\d{2}/);
    page.rerender(<ProductArt connection={connection} screen={<p>Custom screen</p>} />);
    expect(screen.queryByTestId('device-screen')).not.toBeInTheDocument();
    expect(screen.getByText('Custom screen')).toBeInTheDocument();
  });

  it('reports hover and focus on a control, and marks selection only with aria-pressed', async () => {
    const user = userEvent.setup();
    const connection = connectionFixture();
    connection.device!.presentation!.controlLayout = 'rotary-three-button';
    const preview = vi.fn();
    render(
      <ProductArt
        connection={connection}
        selectedControl="knob"
        onSelectControl={vi.fn()}
        onPreviewControl={preview}
      />,
    );
    const middle = screen.getByRole('button', { name: 'Select Middle button' });
    await user.hover(middle);
    expect(preview).toHaveBeenLastCalledWith('button2');
    await user.unhover(middle);
    expect(preview).toHaveBeenLastCalledWith(undefined);
    // Selection is exposed to assistive tech, not drawn as the old coloured ring.
    expect(screen.getByRole('button', { name: 'Select Knob' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(middle).toHaveAttribute('aria-pressed', 'false');
    expect(css).not.toMatch(/outline:[^;]*#67dca7/);
  });

  it('shows the idle screen with a hint and ellipsis-ready lines', () => {
    render(<DeviceScreen profile="Editing" hint="Select a control" />);
    expect(screen.getByTestId('device-screen')).toHaveTextContent('Select a control');
    render(
      <DeviceScreen
        profile="General"
        title="Knob"
        lines={[
          { gesture: 'Turn', action: 'Volume' },
          { gesture: 'Press', action: 'Mute' },
          { gesture: 'Hold', action: 'A' },
          { gesture: 'Extra', action: 'Dropped' },
        ]}
      />,
    );
    expect(screen.queryByText('Dropped')).not.toBeInTheDocument();
    expect(screen.getByText('Mute')).toBeInTheDocument();
  });
});
