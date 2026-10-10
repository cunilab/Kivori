import { Marked } from 'marked';

const SAFE_URL = /^(https?:|mailto:|#|\/)/i;

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// Release notes come from GitHub and are untrusted: raw HTML is shown as text, and only
// http(s)/mailto/relative URLs survive in links and images.
const markdown = new Marked({
  gfm: true,
  renderer: {
    html({ text }) {
      return escapeHtml(text);
    },
    link({ href, title, tokens }) {
      const inner = this.parser.parseInline(tokens);
      if (!SAFE_URL.test(href)) return inner;
      const titleAttr = title ? ` title="${escapeHtml(title)}"` : '';
      return `<a href="${escapeHtml(href)}"${titleAttr} rel="noopener noreferrer">${inner}</a>`;
    },
    image({ href, text }) {
      if (!/^https?:/i.test(href)) return escapeHtml(text);
      return `<img src="${escapeHtml(href)}" alt="${escapeHtml(text)}" loading="lazy" />`;
    },
  },
});

/** Markdown to an HTML string that is safe to inject: no raw HTML, no script/data URLs. */
export function renderMarkdown(source: string): string {
  return markdown.parse(source, { async: false });
}
