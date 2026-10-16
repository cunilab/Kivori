import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { connectionFixture } from '@/test/connection';
import { ProductArt } from './product-art';

const ipc = vi.hoisted(() => ({ getProductArt: vi.fn() }));
vi.mock('@/lib/ipc', () => ipc);

function product(art: string | null) {
  const connection = connectionFixture();
  connection.device!.presentation = { displayName: `Product ${art}`, model: 'KV-T1', art };
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
});
