import { describe, expect, it } from 'vitest';
import { renderMarkdown } from './markdown';

describe('renderMarkdown', () => {
  it('renders basic markdown', () => {
    const html = renderMarkdown('## Fixed\n\n- one\n- **two**');
    expect(html).toContain('<h2>Fixed</h2>');
    expect(html).toContain('<li>one</li>');
    expect(html).toContain('<strong>two</strong>');
  });

  it('escapes raw HTML instead of passing it through', () => {
    const html = renderMarkdown('<script>alert(1)</script>\n\nhi <img src=x onerror=alert(1)>');
    expect(html).not.toContain('<script');
    expect(html).not.toContain('<img');
    expect(html).toContain('&lt;script&gt;');
  });

  it('drops unsafe link and image URLs', () => {
    const html = renderMarkdown('[x](javascript:alert(1)) ![y](data:text/html,hi)');
    expect(html).not.toContain('javascript:');
    expect(html).not.toContain('data:');
  });

  it('keeps https links with a safe rel', () => {
    expect(renderMarkdown('[ok](https://example.test)')).toContain(
      '<a href="https://example.test" rel="noopener noreferrer">ok</a>',
    );
  });
});
