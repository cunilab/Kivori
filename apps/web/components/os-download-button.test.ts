import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { OsDownloadButton } from './os-download-button';

describe('download button before platform detection', () => {
  it('offers platform selection without sending a Mac visitor to the Windows installer', () => {
    const html = renderToStaticMarkup(
      createElement(OsDownloadButton, { version: '0.2.1', beta: true, macAvailable: true }),
    );
    expect(html).toContain('Choose your platform');
    expect(html).toContain('href="#platforms"');
    expect(html).not.toContain('/download/windows/');
    expect(html).not.toContain('/download/macos/');
  });
});
