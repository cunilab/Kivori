// Run with: node --test scripts/release-tools.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  bumpVersion,
  commitBody,
  nextVersion,
  parseSemver,
  planRelease,
  setTomlVersion,
  tomlVersion,
} from './release-bump.mjs';
import {
  groupCommits,
  isAppCommit,
  isFirmwareCommit,
  packageVersion,
  parseConventional,
  parseGitLog,
  productName,
  releaseNotes,
  releaseTitle,
} from './release-info.mjs';
import { buildManifest } from './updater-manifest.mjs';

// --- release-bump.mjs ---------------------------------------------------------------------------

test('semver bumps, including a real major below 1.0.0', () => {
  assert.equal(bumpVersion('0.1.0', 'patch'), '0.1.1');
  assert.equal(bumpVersion('0.1.3', 'minor'), '0.2.0');
  assert.equal(bumpVersion('0.4.2', 'major'), '1.0.0');
  assert.equal(bumpVersion('1.4.0', 'major'), '2.0.0');
  assert.equal(bumpVersion('1.4.0', 'none'), '1.4.0');
  assert.throws(() => bumpVersion('1.4.0', 'huge'), /unknown bump/);
});

test('versions must be plain X.Y.Z', () => {
  assert.deepEqual(parseSemver('10.0.7'), [10, 0, 7]);
  for (const bad of ['1.2', '1.2.3-beta.1', '01.2.3', '1.2.3.4', 'x', '']) {
    assert.throws(() => parseSemver(bad), /X\.Y\.Z/, bad);
  }
});

test('an exact version overrides the bump and must be greater', () => {
  assert.equal(nextVersion('0.1.0', { bump: 'minor', exact: '' }), '0.2.0');
  assert.equal(nextVersion('0.1.0', { bump: 'minor', exact: ' 0.1.5 ' }), '0.1.5');
  assert.equal(nextVersion('0.1.0', { bump: 'patch', exact: 'v1.0.0' }), '1.0.0');
  assert.equal(nextVersion('1.4.0', { bump: 'none', exact: '1.4.1' }), '1.4.1');
  assert.throws(() => nextVersion('0.2.0', { bump: 'minor', exact: '0.2.0' }), /not greater/);
  assert.throws(() => nextVersion('0.2.0', { bump: 'minor', exact: '0.1.9' }), /not greater/);
  assert.throws(() => nextVersion('0.2.0', { bump: 'minor', exact: '0.3' }), /X\.Y\.Z/);
});

test('the release plan names both tags and refuses existing ones', () => {
  const base = {
    appCurrent: '0.1.0',
    firmwareCurrent: '1.4.0',
    product: 'pala',
    appBump: 'minor',
    appExact: '',
    firmwareBump: 'none',
    firmwareExact: '',
    tagExists: (t) => t === 'v0.1.0' || t === 'fw-pala-v1.4.0',
  };
  const same = planRelease(base);
  assert.deepEqual(same, {
    appCurrent: '0.1.0',
    app: '0.2.0',
    appTag: 'v0.2.0',
    firmwareCurrent: '1.4.0',
    firmware: '1.4.0',
    firmwareChanged: false,
    firmwareTag: '',
  });
  assert.equal(commitBody(same, 'Pip'), 'Pip firmware 1.4.0, unchanged.');

  const fw = planRelease({ ...base, firmwareBump: 'patch' });
  assert.equal(fw.firmwareTag, 'fw-pala-v1.4.1');
  assert.equal(commitBody(fw, 'Pip'), 'Pip firmware 1.4.1 (was 1.4.0), tag fw-pala-v1.4.1.');

  assert.throws(() => planRelease({ ...base, appExact: '0.1.0' }), /not greater/);
  assert.throws(
    () => planRelease({ ...base, tagExists: (t) => t === 'v0.2.0' }),
    /tag v0\.2\.0 already exists/,
  );
  assert.throws(
    () => planRelease({ ...base, firmwareBump: 'patch', tagExists: (t) => t.startsWith('fw-') }),
    /fw-pala-v1\.4\.1 already exists/,
  );
  assert.throws(() => planRelease({ ...base, appBump: 'none' }), /app bump/);
});

test('only the version line of the named table changes', () => {
  const root = `[workspace]
members = ["a"]

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
serde = { version = "1.0.0" }
`;
  assert.equal(tomlVersion(root, 'workspace.package'), '0.1.0');
  const bumped = setTomlVersion(root, 'workspace.package', '0.2.0');
  assert.equal(bumped, root.replace('version = "0.1.0"', 'version = "0.2.0"'));
  assert.ok(bumped.includes('serde = { version = "1.0.0" }'));

  const fw =
    '[workspace]\nresolver = "2"\n\n[package]\nname = "kivori-firmware"\nversion = "1.4.0"\n';
  assert.equal(tomlVersion(fw, 'package'), '1.4.0');
  assert.equal(tomlVersion(setTomlVersion(fw, 'package', '1.5.0'), 'package'), '1.5.0');
  assert.throws(() => tomlVersion(fw, 'workspace.package'), /no \[workspace\.package\]/);
});

// --- release-info.mjs ---------------------------------------------------------------------------

/** What `git log --no-merges --reverse --format=GIT_LOG_FORMAT --name-only` prints. */
function fakeLog(commits) {
  return commits
    .map(
      ([hash, subject, body, files]) =>
        `\x1e${hash}\x1f${subject}\x1f${body}\x1f\n\n${files.join('\n')}\n`,
    )
    .join('');
}

const log = fakeLog([
  ['a1', 'feat(m2.7): in-app updater', '', ['apps/desktop/src-tauri/src/updater/mod.rs']],
  ['a2', 'fix(mic): say "can\'t tell"', 'Longer body.\n\nCo-Authored-By: x', ['crates/x.rs']],
  ['a3', 'docs: roadmap', '', ['docs/roadmap.md']],
  ['a4', 'chore(release): v0.1.0', '', ['Cargo.toml', 'Cargo.lock']],
  ['a5', 'feat(firmware): idle life', '', ['firmware/pala/src/idle.rs']],
  ['a6', 'fix(firmware): marker log', '', ['firmware/esp32-c3/src/log.rs']],
  ['a7', 'perf(desktop): batch detents', '', ['apps/desktop/src-tauri/src/volume.rs']],
  ['a8', 'feat(web): new home page', '', ['apps/web/app/page.tsx', 'bun.lock']],
  ['a9', 'feat(protocol)!: drop v0 frames', '', ['crates/kivori-protocol/src/lib.rs']],
  ['b1', 'refactor: tidy', '', ['apps/desktop/src/x.ts']],
  ['b2', 'test: more', '', ['tests/a.rs']],
  ['b3', 'ci: cache', '', ['.github/workflows/host.yml']],
  ['b4', 'wip', '', ['apps/desktop/src/y.ts']],
  [
    'b5',
    'feat(protocol): mic state on the wire',
    'BREAKING CHANGE: none really',
    ['crates/kivori-protocol/src/m.rs', 'firmware/pala/src/m.rs'],
  ],
]);

test('git log records parse with body and files', () => {
  const commits = parseGitLog(log);
  assert.equal(commits.length, 14);
  assert.deepEqual(commits[1], {
    hash: 'a2',
    subject: 'fix(mic): say "can\'t tell"',
    body: 'Longer body.\n\nCo-Authored-By: x',
    files: ['crates/x.rs'],
  });
  assert.deepEqual(parseConventional(commits[8]), {
    type: 'feat',
    scope: 'protocol',
    breaking: true,
    description: 'drop v0 frames',
  });
  assert.equal(parseConventional(commits[12]), null);
  assert.ok(!isAppCommit(commits[4]), 'firmware-only commit');
  assert.ok(!isAppCommit(commits[7]), 'website commit');
  assert.ok(isAppCommit(commits[13]), 'shared commit counts for both');
  assert.ok(isAppCommit({ subject: 'feat: x', files: [] }));
  for (const dir of ['firmware/pala', 'firmware/desk-buddy', 'firmware/esp32-c3']) {
    assert.ok(isFirmwareCommit({ files: [`${dir}/src/a.rs`] }, 'pala'), `${dir} is pala firmware`);
  }
  assert.ok(!isFirmwareCommit({ files: ['firmware/lada/src/a.rs'] }, 'pala'));
});

test('notes group like a changelog and skip chores', () => {
  const grouped = groupCommits(parseGitLog(log));
  assert.equal(
    grouped,
    `### Breaking changes

- protocol: drop v0 frames
- protocol: mic state on the wire

### Features

- in-app updater
- firmware: idle life
- web: new home page

### Fixes

- mic: say "can't tell"
- firmware: marker log

### Performance

- desktop: batch detents`,
  );
  assert.equal(groupCommits(parseGitLog(fakeLog([['c', 'chore: x', '', []]]))), '');
});

test('release notes: app sections, then the firmware, unchanged or with its commits', () => {
  const commits = parseGitLog(log);
  const base = {
    commits,
    product: 'pala',
    productDisplay: 'Pip',
    prevVersion: '0.1.0',
  };

  const unchanged = releaseNotes({ ...base, firmwareVersion: '1.4.0', previousFirmware: '1.4.0' });
  assert.equal(unchanged.notes, unchanged.short);
  assert.ok(unchanged.short.startsWith('### Breaking changes\n'));
  assert.ok(!unchanged.short.includes('idle life'), 'firmware-only commits stay out of the app');
  assert.ok(!unchanged.short.includes('new home page'), 'website commits stay out');
  assert.ok(
    unchanged.short.endsWith(
      '### Firmware\n\nPip firmware 1.4.0 is bundled with this release, unchanged since Kivori 0.1.0.\n',
    ),
  );

  const changed = releaseNotes({
    ...base,
    firmwareVersion: '1.5.0',
    previousFirmware: '1.4.0',
    compareUrl: 'https://github.com/cunilab/Kivori/compare/v0.1.0...v0.2.0',
  });
  const firmware = changed.short.slice(changed.short.indexOf('### Firmware'));
  assert.equal(
    firmware,
    `### Firmware

Pip firmware 1.5.0 is bundled with this release (was 1.4.0).

#### Breaking changes

- protocol: mic state on the wire

#### Features

- firmware: idle life

#### Fixes

- firmware: marker log
`,
  );
  assert.equal(
    changed.notes,
    `${changed.short}\n**Full changelog:** https://github.com/cunilab/Kivori/compare/v0.1.0...v0.2.0\n`,
  );

  const quiet = releaseNotes({
    ...base,
    commits: parseGitLog(fakeLog([['c', 'chore: x', '', ['Cargo.toml']]])),
    firmwareVersion: '1.4.1',
    previousFirmware: '1.4.0',
  });
  assert.equal(
    quiet.short,
    `No user-facing changes in the app.

### Firmware

Pip firmware 1.4.1 is bundled with this release (was 1.4.0).

No firmware features or fixes are listed since Kivori 0.1.0.
`,
  );
});

test('title and names come from the files', () => {
  assert.equal(
    packageVersion('[package]\nname = "x"\nversion = "1.4.0"\n[dependencies]\n'),
    '1.4.0',
  );
  assert.equal(
    productName('display_name = "Kivori Pip"\nshort_name = "Pip"\nmodel = "KV-P1"\n'),
    'Pip',
  );
  assert.throws(() => productName('display_name = "Kivori Pip"\n'), /short_name/);
  assert.throws(() => productName('short_name = ""\n'), /short_name/);
  assert.equal(releaseTitle('0.2.0', 'Pip', '1.4.0'), 'Kivori 0.2.0 · Pip firmware 1.4.0');
});

// --- updater-manifest.mjs -----------------------------------------------------------------------

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
