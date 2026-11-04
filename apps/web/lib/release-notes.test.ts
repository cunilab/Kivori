import { describe, expect, it } from 'vitest';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { ReleaseNotes } from '@/components/release-notes';
import { FALLBACK_NOTE, notesFor, parseNotes } from './release-notes';

describe('release notes', () => {
  it('parses paragraphs and bullet runs', () => {
    expect(parseNotes('Intro line\ncontinues.\n\n- one\n- two\n\nOutro')).toEqual([
      { kind: 'paragraph', text: 'Intro line continues.' },
      { kind: 'list', items: ['one', 'two'] },
      { kind: 'paragraph', text: 'Outro' },
    ]);
  });

  it('returns the curated notes for a version that has a file', () => {
    const blocks = notesFor('0.1.0');
    expect(blocks.some((block) => block.kind === 'list')).toBe(true);
    expect(JSON.stringify(blocks)).not.toContain(FALLBACK_NOTE);
  });

  it('falls back to a one-line note for a version without a file', () => {
    expect(notesFor('9.9.9')).toEqual([{ kind: 'paragraph', text: 'Improvements and fixes.' }]);
  });
});

it('removes source identifiers and renders warnings as text', () => {
  const rendered = JSON.stringify(
    parseNotes(
      '## Warning\n- Keep your settings. [details](https://github.com/cunilab/Kivori/issues/12) #12 @author\n<script>alert(1)</script> https://github.com/cunilab/Kivori/compare/a...b',
    ),
  );
  expect(rendered).toContain('Warning');
  expect(rendered).toContain('Keep your settings');
  expect(rendered).not.toMatch(/github|#12|@author|<script>/i);
});

it('sanitizes the published 0.3.1 warning and changelog links', () => {
  const body =
    '## Known limits\n- Windows builds are not publisher-signed. SmartScreen or antivirus warnings can still occur; the exact file/detection for the [reported security issue](https://github.com/cunilab/Kivori/issues/167) remains unconfirmed.\n- The macOS download is for Apple Silicon and is not Apple-notarized; first installation may require **Open Anyway**.\n\n- desktop: add interactive controls and held-turn shortcuts (#170)\n\n**Full changelog:** https://github.com/cunilab/Kivori/compare/v0.3.0...v0.3.1';
  const rendered = JSON.stringify(parseNotes(body));
  expect(rendered).toContain('SmartScreen');
  expect(rendered).toContain('Open Anyway');
  expect(rendered).not.toMatch(/github|#167|#170|changelog/i);
});

it('sanitizes dynamic text against public leak rules while retaining warnings', () => {
  const body = `## Updating and compatibility
- Settings migrate to schema v2, preserving the original v1 backup. Older builds cannot read v2 settings.
- A stationary hold starts recovery at two seconds and reboots at ten seconds.
- Windows builds are not publisher-signed and the Mac app is not Apple-notarized.

See github.com/cunilab/Kivori; API.GITHUB.COM/repos/cunilab/Kivori and [details](HTTPS://GITHUB.COM/cunilab/Kivori/issues/7) <https://github.com/test>.
<script>alert('hi')</script> ESP32 ST7789 HW-040 SuperMini blueprint 12.5mm 4 mm TBC`;
  const rendered = renderToStaticMarkup(
    createElement(ReleaseNotes, { version: '0.3.1', notes: body + ' git**hub.com ES**P32 12**mm' }),
  );
  expect(rendered).not.toMatch(
    /github\.com|cunilab\/kivori|ESP32|ST7789|HW-040|SuperMini|blueprint|\b\d+(\.\d+)?\s?mm\b|\bTBC\b|<script>/i,
  );
  expect(rendered).toContain('schema v2');
  expect(rendered).toContain('recovery');
  expect(rendered).toContain('publisher-signed');
  expect(rendered).toContain('Apple-notarized');
});
