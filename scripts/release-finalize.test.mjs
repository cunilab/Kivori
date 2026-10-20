import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { releaseCommit, requireMainCi, pushReleaseTags } from './release-finalize.mjs';

const sha = 'a'.repeat(40);
const pr = {
  user: { login: 'github-actions[bot]' },
  merged_at: '2026-10-20T00:00:00Z',
  title: 'chore(release): v0.2.0',
  merge_commit_sha: sha,
  head: { ref: 'release/v0.2.0', repo: { full_name: 'test/repo' } },
  base: { ref: 'main', repo: { full_name: 'test/repo' } },
};
test('only merged same-repository version PRs name a release commit', () => {
  assert.deepEqual(releaseCommit(pr, 'test/repo'), { sha, tag: 'v0.2.0' });
  for (const patch of [
    { user: { login: 'someone' } },
    { merged_at: null },
    { merge_commit_sha: 'main' },
    { title: 'release me' },
    { head: { ...pr.head, ref: 'feat/anything' } },
    { head: { ...pr.head, repo: { full_name: 'fork/repo' } } },
    { base: { ...pr.base, ref: 'develop' } },
  ])
    assert.throws(() => releaseCommit({ ...pr, ...patch }, 'test/repo'), /merged, same-repository/);
});

test('all three latest push workflows must succeed on the exact merged main SHA', () => {
  const good = {
    head_sha: sha,
    head_branch: 'main',
    event: 'push',
    status: 'completed',
    conclusion: 'success',
  };
  const workflows = Object.fromEntries(
    ['host', 'firmware', 'determinism'].map((name) => [name, [good]]),
  );
  requireMainCi(sha, workflows);
  for (const name of Object.keys(workflows)) {
    for (const patch of [
      { status: 'in_progress', conclusion: null },
      { conclusion: 'failure' },
      { head_sha: 'b'.repeat(40) },
      { head_branch: 'release/v0.2.0' },
      { event: 'workflow_dispatch' },
    ])
      assert.throws(
        () => requireMainCi(sha, { ...workflows, [name]: [{ ...good, ...patch }] }),
        /push CI must pass/,
      );
    assert.throws(() => requireMainCi(sha, { ...workflows, [name]: [] }), /push CI must pass/);
    assert.throws(
      () =>
        requireMainCi(sha, { ...workflows, [name]: [{ ...good, conclusion: 'failure' }, good] }),
      /push CI must pass/,
    );
  }
});

const git = (cwd, ...args) =>
  execFileSync('git', args, { cwd, encoding: 'utf8', stdio: 'pipe' }).trim();
function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'kivori-release-tags-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const remote = join(root, 'remote.git');
  git(root, 'init', '--quiet', '--initial-branch=main');
  git(root, 'config', 'user.name', 'Release test');
  git(root, 'config', 'user.email', 'test@example.invalid');
  git(root, 'config', 'commit.gpgsign', 'false');
  git(root, 'config', 'tag.gpgsign', 'false');
  writeFileSync(join(root, 'version'), '0.1.0');
  git(root, 'add', 'version');
  git(root, 'commit', '--quiet', '-m', 'initial');
  git(root, 'init', '--bare', '--quiet', remote);
  git(root, 'remote', 'add', 'origin', remote);
  git(root, 'push', '--quiet', 'origin', 'main');
  const before = git(root, 'rev-parse', 'HEAD');
  git(root, 'checkout', '--quiet', '-b', 'release/v0.2.0');
  writeFileSync(join(root, 'version'), '0.2.0');
  git(root, 'commit', '--quiet', '-am', 'chore(release): v0.2.0');
  return { root, remote, before, current: git(root, 'rev-parse', 'HEAD') };
}
test('finalize pushes only immutable tags, preserves main and retries the same SHA', (t) => {
  const { root, remote, before, current } = fixture(t);
  const firmware = [{ tag: 'fw-dial-v0.1.1' }, { tag: 'fw-lamp-v0.3.0' }, { tag: '' }];
  pushReleaseTags(root, 'v0.2.0', firmware);
  for (const tag of ['v0.2.0', 'fw-dial-v0.1.1', 'fw-lamp-v0.3.0'])
    assert.equal(git(remote, 'rev-parse', `${tag}^{commit}`), current);
  assert.equal(git(remote, 'rev-parse', 'main'), before);
  pushReleaseTags(root, 'v0.2.0', firmware);
  assert.equal(git(root, 'config', 'user.name'), 'Release test');
  git(root, 'checkout', '--quiet', 'main');
  assert.throws(() => pushReleaseTags(root, 'v0.2.0', firmware), /immutable/);
});
test('a remote firmware tag race atomically refuses all new tags', (t) => {
  const { root, remote, before } = fixture(t);
  git(remote, 'update-ref', 'refs/tags/fw-dial-v0.1.1', before);
  assert.throws(() => pushReleaseTags(root, 'v0.2.0', [{ tag: 'fw-dial-v0.1.1' }]));
  assert.equal(git(remote, 'tag', '--list', 'v0.2.0'), '');
  assert.equal(git(remote, 'rev-parse', 'main'), before);
});

test('retry pushes tags left local by a previously failed atomic push', (t) => {
  const { root, remote } = fixture(t);
  git(root, 'tag', 'v0.2.0');
  git(root, 'tag', 'fw-dial-v0.1.1');
  pushReleaseTags(root, 'v0.2.0', [{ tag: 'fw-dial-v0.1.1' }]);
  assert.equal(git(remote, 'rev-parse', 'v0.2.0^{commit}'), git(root, 'rev-parse', 'HEAD'));
});
