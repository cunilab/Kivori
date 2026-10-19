// Run with: node --test scripts/release-tools.test.mjs
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { test } from 'node:test';
import {
  bumpVersion,
  commitBody,
  latestTagVersion,
  nextVersion,
  parseSemver,
  planRelease,
  setTomlVersion,
  tomlVersion,
} from './release-bump.mjs';
import {
  firmwareDirs,
  firstReleaseNotes,
  groupCommits,
  isAppCommit,
  isFirmwareCommit,
  packageVersion,
  parseConventional,
  parseGitLog,
  productDisplayName,
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
  assert.equal(bumpVersion('0.1.0', 'none'), '0.1.0');
  assert.throws(() => bumpVersion('0.1.0', 'huge'), /unknown bump/);
});

test('versions must be plain X.Y.Z', () => {
  assert.deepEqual(parseSemver('10.0.7'), [10, 0, 7]);
  for (const bad of ['1.2', '1.2.3-beta.1', '01.2.3', '1.2.3.4', 'x', '']) {
    assert.throws(() => parseSemver(bad), /X\.Y\.Z/, bad);
  }
});

test('the latest tag of a component is the highest X.Y.Z with its prefix', () => {
  const tags = [
    'v0.1.0',
    'v0.10.0',
    'v0.9.3',
    'fw-pala-v0.3.0',
    'fw-lamp-v9.0.0',
    'v1.0.0-rc1',
    'x',
  ];
  assert.equal(latestTagVersion(tags, 'v'), '0.10.0');
  assert.equal(latestTagVersion(tags, 'fw-pala-v'), '0.3.0');
  assert.equal(latestTagVersion(tags, 'fw-desk-v'), '');
  assert.equal(latestTagVersion([], 'v'), '');
  assert.equal(latestTagVersion(['pre-hardware-sim-green'], 'v'), '');
});

test('a new version must be greater than the latest tag, and never below Cargo.toml', () => {
  // No tag yet: the Cargo version itself may be released, as a bump or an exact version may be.
  assert.equal(nextVersion('0.1.0', { bump: 'none', exact: '' }), '0.1.0');
  assert.equal(nextVersion('0.1.0', { bump: 'minor', exact: '' }), '0.2.0');
  assert.equal(nextVersion('0.1.0', { bump: 'minor', exact: ' 0.1.0 ' }), '0.1.0');
  assert.equal(nextVersion('0.1.0', { bump: 'minor', exact: ' 0.1.5 ' }), '0.1.5');
  assert.equal(nextVersion('0.1.0', { bump: 'patch', exact: 'v1.0.0' }), '1.0.0');
  assert.throws(() => nextVersion('0.2.0', { bump: 'none', exact: '0.1.9' }), /below the current/);
  assert.throws(() => nextVersion('0.2.0', { bump: 'minor', exact: '0.3' }), /X\.Y\.Z/);
  // With a tag, the version must pass it.
  const latest = '0.2.0';
  assert.equal(nextVersion('0.2.0', { bump: 'patch', exact: '', latest }), '0.2.1');
  assert.equal(nextVersion('0.2.0', { bump: 'none', exact: '0.3.0', latest }), '0.3.0');
  assert.throws(() => nextVersion('0.2.0', { bump: 'none', exact: '', latest }), /not greater/);
  assert.throws(
    () => nextVersion('0.2.0', { bump: 'none', exact: '0.2.0', latest }),
    /not greater/,
  );
  // Cargo.toml bumped by hand past the tag: releasing it as it is is fine.
  assert.equal(nextVersion('0.3.0', { bump: 'none', exact: '', latest }), '0.3.0');
  assert.throws(() => nextVersion('0.1.0', { bump: 'patch', exact: '', latest }), /not greater/);
});

const first = {
  appCurrent: '0.1.0',
  firmwareCurrent: '0.1.0',
  product: 'pala',
  appBump: 'none',
  appExact: '',
  firmwareBump: 'none',
  firmwareExact: '',
  tags: ['pre-hardware-sim-green'],
};

test('the first release ships the current versions as they are and starts both tag lineages', () => {
  const plan = planRelease(first);
  assert.deepEqual(plan, {
    appCurrent: '0.1.0',
    appPrevious: '',
    app: '0.1.0',
    appChanged: false,
    appTag: 'v0.1.0',
    firmwareCurrent: '0.1.0',
    firmwarePrevious: '',
    firmware: '0.1.0',
    firmwareChanged: false,
    firmwareTag: 'fw-pala-v0.1.0',
    commitNeeded: false,
  });
  assert.equal(commitBody(plan, 'Pip'), 'Pip firmware 0.1.0, first tagged as fw-pala-v0.1.0.');

  // A bump on the first release works from the Cargo versions, and needs a version commit.
  const bumped = planRelease({ ...first, appBump: 'minor', firmwareBump: 'patch' });
  assert.deepEqual(bumped, {
    appCurrent: '0.1.0',
    appPrevious: '',
    app: '0.2.0',
    appChanged: true,
    appTag: 'v0.2.0',
    firmwareCurrent: '0.1.0',
    firmwarePrevious: '',
    firmware: '0.1.1',
    firmwareChanged: true,
    firmwareTag: 'fw-pala-v0.1.1',
    commitNeeded: true,
  });
  assert.equal(commitBody(bumped, 'Pip'), 'Pip firmware 0.1.1 (was 0.1.0), tag fw-pala-v0.1.1.');

  // Only the firmware moves: still a commit, and the app keeps its first, untagged version.
  const fwOnly = planRelease({ ...first, firmwareExact: '0.2.0' });
  assert.equal(fwOnly.app, '0.1.0');
  assert.equal(fwOnly.firmwareTag, 'fw-pala-v0.2.0');
  assert.equal(fwOnly.commitNeeded, true);
});

test('after the first release, none is refused and every version must pass the latest tag', () => {
  const later = { ...first, tags: ['v0.1.0', 'fw-pala-v0.1.0'] };
  assert.throws(
    () => planRelease(later),
    /tag v0\.1\.0 already exists; pick patch, minor or major/,
  );
  assert.throws(() => planRelease({ ...later, appExact: '0.1.0' }), /tag v0\.1\.0 already exists/);

  const same = planRelease({ ...later, appBump: 'minor' });
  assert.deepEqual(same, {
    appCurrent: '0.1.0',
    appPrevious: '0.1.0',
    app: '0.2.0',
    appChanged: true,
    appTag: 'v0.2.0',
    firmwareCurrent: '0.1.0',
    firmwarePrevious: '0.1.0',
    firmware: '0.1.0',
    firmwareChanged: false,
    firmwareTag: '',
    commitNeeded: true,
  });
  assert.equal(commitBody(same, 'Pip'), 'Pip firmware 0.1.0, unchanged.');
  // The firmware tag already there: an exact firmware version equal to it is just unchanged.
  assert.equal(planRelease({ ...later, appBump: 'patch', firmwareExact: '0.1.0' }).firmwareTag, '');

  const fw = planRelease({ ...later, appBump: 'minor', firmwareBump: 'patch' });
  assert.equal(fw.firmwareTag, 'fw-pala-v0.1.1');
  assert.equal(commitBody(fw, 'Pip'), 'Pip firmware 0.1.1 (was 0.1.0), tag fw-pala-v0.1.1.');

  // The latest tag wins over a stale Cargo.toml, and an existing tag is never reused.
  const ahead = { ...later, tags: ['v0.1.0', 'v0.3.0', 'fw-pala-v0.1.0'] };
  assert.throws(
    () => planRelease({ ...ahead, appBump: 'minor' }),
    /0\.2\.0 is not greater than .* 0\.3\.0/,
  );
  assert.equal(planRelease({ ...ahead, appBump: 'minor', appExact: '0.4.0' }).app, '0.4.0');
  assert.throws(
    () => planRelease({ ...later, appBump: 'minor', tags: [...later.tags, 'v0.2.0'] }),
    /tag v0\.2\.0 already exists/,
  );
  assert.throws(
    () =>
      planRelease({
        ...later,
        appBump: 'minor',
        firmwareBump: 'patch',
        tags: [...later.tags, 'fw-pala-v0.1.1'],
      }),
    /fw-pala-v0\.1\.1 already exists/,
  );
  // Firmware that was bumped by hand past its tag gets released and tagged under `none`.
  const handBumped = planRelease({ ...later, appBump: 'patch', firmwareCurrent: '0.2.0' });
  assert.equal(handBumped.firmwareTag, 'fw-pala-v0.2.0');
  assert.equal(handBumped.firmwareChanged, false);
  assert.equal(commitBody(handBumped, 'Pip'), 'Pip firmware 0.2.0, tag fw-pala-v0.2.0.');
  // ...but never an untagged one below its latest tag.
  assert.throws(
    () =>
      planRelease({
        ...later,
        appBump: 'patch',
        firmwareCurrent: '0.2.0',
        tags: [...later.tags, 'fw-pala-v0.4.0'],
      }),
    /0\.2\.0 is not greater than .* 0\.4\.0/,
  );
  assert.throws(() => planRelease({ ...later, appBump: 'huge' }), /app bump must be none/);
  assert.throws(() => planRelease({ ...later, firmwareBump: '' }), /firmware bump must be none/);
});

// The CLI parses TOML in Bun; retain Node's test/mocking semantics for the surrounding suites.
function tomlTest(name, check) {
  test(name, () => {
    execFileSync(
      'bun',
      [
        '--eval',
        `
      import assert from 'node:assert/strict';
      import { setTomlVersion, tomlVersion } from ${JSON.stringify(new URL('./release-bump.mjs', import.meta.url).href)};
      import { packageVersion } from ${JSON.stringify(new URL('./release-info.mjs', import.meta.url).href)};
      (${check.toString()})();
    `,
      ],
      { stdio: 'pipe' },
    );
  });
}

tomlTest('only the version line of the named table changes', () => {
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
    '[workspace]\nresolver = "2"\n\n[package]\nname = "kivori-firmware"\nversion = "0.1.0"\n';
  assert.equal(tomlVersion(fw, 'package'), '0.1.0');
  assert.equal(tomlVersion(setTomlVersion(fw, 'package', '0.2.0'), 'package'), '0.2.0');
  assert.throws(() => tomlVersion(fw, 'workspace.package'), /no \[workspace\.package\]/);
});

tomlTest(
  'TOML versions accept literal strings, header comments and CRLF without reformatting',
  () => {
    for (const table of ['package', 'workspace.package']) {
      const text = `[ ${table.replace('.', ' . ')} ] # release version\r\n  version = '0.1.0' # keep me\r\n[dependencies]\r\nserde = { version = "1.0.0" }\r\n`;
      assert.equal(tomlVersion(text, table), '0.1.0');
      const updated = setTomlVersion(text, table, '0.2.0');
      assert.equal(updated, text.replace("'0.1.0'", "'0.2.0'"));
      assert.equal(tomlVersion(updated, table), '0.2.0');
      if (table === 'package') assert.equal(packageVersion(text), '0.1.0');
    }
  },
);

tomlTest('version readers accept parsed TOML forms and reject inherited versions', () => {
  assert.equal(tomlVersion("package.version = '0.3.0'\n", 'package'), '0.3.0');
  assert.equal(packageVersion('[package]\nauthors = ["a"]\nversion = "0.1.0"\n'), '0.1.0');
  assert.throws(
    () => tomlVersion('[package]\nversion.workspace = true\n', 'package'),
    /no version/,
  );
  assert.throws(
    () => setTomlVersion("package = { version = '0.1.0' }\n", 'package', '0.2.0'),
    /Cannot safely edit/,
  );
  assert.throws(
    () => setTomlVersion('[package]\nversion = """0.1.0"""\n', 'package', '0.2.0'),
    /Cannot safely edit/,
  );
  const both = '[workspace.package]\nversion = "0.1.0"\n[package]\nversion = "0.4.0"\n';
  assert.equal(setTomlVersion(both, 'package', '0.5.0'), both.replace('"0.4.0"', '"0.5.0"'));
});

tomlTest('TOML version edits refuse fake table headers inside multiline strings', () => {
  const text = `[metadata]\nnote = """\n[package]\nversion = "0.1.0"\n"""\n[package]\nversion = "0.1.0"\n`;
  assert.throws(() => setTomlVersion(text, 'package', '0.2.0'), /Cannot safely edit/);
  assert.equal(tomlVersion(text, 'package'), '0.1.0');
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

    prevVersion: '0.1.0',
  };

  const unchanged = releaseNotes({
    ...base,
    products: [{ name: 'Pip', version: '0.1.0', previous: '0.1.0', dirs: firmwareDirs('pala') }],
  });
  assert.equal(unchanged.notes, unchanged.short);
  assert.ok(unchanged.short.startsWith('### Breaking changes\n'));
  assert.ok(!unchanged.short.includes('idle life'), 'firmware-only commits stay out of the app');
  assert.ok(!unchanged.short.includes('new home page'), 'website commits stay out');
  assert.ok(
    unchanged.short.endsWith(
      '### Firmware\n\nPip firmware 0.1.0 is bundled with this release, unchanged since Kivori 0.1.0.\n',
    ),
  );

  const changed = releaseNotes({
    ...base,
    products: [{ name: 'Pip', version: '0.2.0', previous: '0.1.0', dirs: firmwareDirs('pala') }],
    compareUrl: 'https://github.com/cunilab/Kivori/compare/v0.1.0...v0.2.0',
  });
  const firmware = changed.short.slice(changed.short.indexOf('### Firmware'));
  assert.equal(
    firmware,
    `### Firmware

Pip firmware 0.2.0 is bundled with this release (was 0.1.0).

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
    products: [{ name: 'Pip', version: '0.1.1', previous: '0.1.0', dirs: firmwareDirs('pala') }],
  });
  assert.equal(
    quiet.short,
    `No user-facing changes in the app.

### Firmware

Pip firmware 0.1.1 is bundled with this release (was 0.1.0).

No firmware features or fixes are listed since Kivori 0.1.0.
`,
  );
});

test('a first release gets a short note, not the whole history', () => {
  const first = firstReleaseNotes({
    version: '0.1.0',
    products: [{ display: 'Kivori Pip', version: '0.1.0' }],
    repo: 'cunilab/Kivori',
  });
  assert.equal(
    first.short,
    `### First release

Kivori 0.1.0, with Kivori Pip firmware 0.1.0.

See the [README](https://github.com/cunilab/Kivori/blob/v0.1.0/README.md) for what Kivori does.
`,
  );
  assert.equal(first.notes, first.short, 'no compare link without a previous tag');
  const local = firstReleaseNotes({
    version: '0.1.0',
    products: [{ display: 'Kivori Pip', version: '0.1.0' }],
  });
  assert.ok(local.short.includes('See the README for what Kivori does.'));
});

test('title and names come from the files', () => {
  assert.equal(
    productName('display_name = "Kivori Pip"\nshort_name = "Pip"\nmodel = "KV-P1"\n'),
    'Pip',
  );
  assert.throws(() => productName('display_name = "Kivori Pip"\n'), /short_name/);
  assert.equal(
    productDisplayName('display_name = "Kivori Pip"\nshort_name = "Pip"\n'),
    'Kivori Pip',
  );
  assert.throws(() => productDisplayName('short_name = "Pip"\n'), /display_name/);
  assert.throws(() => productName('short_name = ""\n'), /short_name/);
  assert.equal(
    releaseTitle('0.2.0', [{ name: 'Pip', version: '0.1.0' }]),
    'Kivori 0.2.0 · Pip firmware 0.1.0',
  );
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
