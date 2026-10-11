// Run with: node --test scripts/release-tools.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  changelogSection,
  packageVersion,
  productName,
  releaseNotes,
  releaseTitle,
} from './release-info.mjs';
import { buildManifest } from './updater-manifest.mjs';

const changelog = `# Changelog

## [0.2.0](https://example.test/compare/v0.1.0...v0.2.0) (2026-10-20)

### Features

* **m2.7:** identity

## [0.1.0](https://example.test) (2026-09-01)

### Features

* first
`;

test('changelogSection picks one version', () => {
  assert.equal(changelogSection(changelog, '0.2.0').split('\n')[0], '### Features');
  assert.ok(!changelogSection(changelog, '0.2.0').includes('first'));
  assert.equal(changelogSection(changelog, '9.9.9'), '');
});

test('title and names come from the files', () => {
  assert.equal(
    packageVersion('[package]\nname = "x"\nversion = "1.4.0"\n[dependencies]\n'),
    '1.4.0',
  );
  assert.equal(productName('display_name = "Kivori Desk Buddy"\n'), 'Desk Buddy');
  assert.equal(
    releaseTitle('0.2.0', 'Desk Buddy', '1.4.0'),
    'Kivori 0.2.0 · Desk Buddy firmware 1.4.0',
  );
});

test('notes carry the firmware line, and its changelog only when given', () => {
  const base = {
    appSection: '### Features',
    productDisplay: 'Desk Buddy',
    firmwareVersion: '1.4.0',
  };
  assert.match(
    releaseNotes({ ...base, firmwareSection: '' }),
    /Desk Buddy firmware 1\.4\.0 is bundled/,
  );
  assert.match(releaseNotes({ ...base, firmwareSection: '* fw fix' }), /\* fw fix/);
});

test('latest.json has the Tauri 2 shape', () => {
  const m = buildManifest({
    version: '0.2.0',
    notes: 'n',
    pubDate: '2026-10-12T01:02:03Z',
    signature: 'SIG\n',
    installerName: 'Kivori_0.2.0_x64-setup.exe',
    repo: 'cunilab/Kivori',
  });
  assert.deepEqual(m, {
    version: '0.2.0',
    notes: 'n',
    pub_date: '2026-10-12T01:02:03.000Z',
    platforms: {
      'windows-x86_64': {
        signature: 'SIG',
        url: 'https://github.com/cunilab/Kivori/releases/download/v0.2.0/Kivori_0.2.0_x64-setup.exe',
      },
    },
  });
  assert.throws(() =>
    buildManifest({ ...m, version: 'x', signature: 's', installerName: 'a', repo: 'r' }),
  );
});
