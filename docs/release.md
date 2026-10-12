# Cutting a release

Releases start only when you decide: there is no standing release PR and nothing happens on a merge.
Two workflows do the work: `.github/workflows/release-start.yml` (the **Release** button: versions,
lockfiles, release commit, tags) and `.github/workflows/release.yml` (build, smoke test, sign for the
updater, attach files to a draft). Nothing is published until you publish the draft. Windows code
signing still waits on owner certificates (#36); see
[architecture.md](./architecture.md#release-pipeline-m3-s5) and ADR-0014/0015.

## One-time owner setup

Until this is done, releases still build but are **not updatable**: the build warns "Release is not
updatable" and no `.sig` or `latest.json` is produced.

1. Generate the updater key pair (from the repo root, after `bun install`):

   ```bash
   bunx tauri signer generate -w ~/.tauri/kivori-updater.key
   ```

   It asks for a password and writes the private key to `~/.tauri/kivori-updater.key` and the public
   key to `~/.tauri/kivori-updater.key.pub`.

2. Store the private key file and the password in a password manager, in **two** places. Losing either
   means installed apps can never update themselves again (ADR-0015). Never commit the key.
3. In GitHub, Settings, Secrets and variables, Actions, add two repository secrets:
   - `TAURI_SIGNING_PRIVATE_KEY`: the full contents of `~/.tauri/kivori-updater.key`;
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the password.
4. Put the **public** key (contents of `~/.tauri/kivori-updater.key.pub`) into
   `plugins.updater.pubkey` in `apps/desktop/src-tauri/tauri.release.conf.json`. Done on 2026-10-12
   (key id `B53242CE79304D17`). With the `TAURI_SIGNING_PRIVATE_KEY` secret set, `release.yml` refuses to
   build a release while that key is empty ("Updater public key missing"), so a rotated key and its
   secret always change together.
5. The Release button pushes a commit and tags to `main` as `github-actions[bot]`. Nothing protects
   `main` today; if you add a branch protection rule or ruleset later, let GitHub Actions bypass it,
   or the push step fails with "Push to main rejected". The "Allow GitHub Actions to create and
   approve pull requests" setting is no longer needed (leaving it on is harmless).

## Windows

1. Merge pull requests with conventional commit titles (`feat:`, `fix:`, `perf:`, scoped like
   `feat(mic): ...`); they become the release notes.
2. When you want a release: Actions, **Release**, Run workflow, on `main`. Pick:
   - `app_bump`: `none`, `patch`, `minor` (default) or `major`. A bump starts from the version in
     `Cargo.toml`; below 1.0.0 a major is a real major (0.4.2 becomes 1.0.0). `none` releases the
     `Cargo.toml` version as it is, which is only possible while it has no `v` tag: that is how the
     first release ships 0.1.0. `app_version`: an exact `X.Y.Z` instead. Whatever you pick must be
     greater than the latest `v*` tag (with no tag yet, the current version itself is allowed) and is
     never below the version in `Cargo.toml`;
   - `firmware_bump`: `none` (default, the firmware keeps its version), `patch`, `minor` or `major`;
     `firmware_version`: an exact `A.B.C` instead, under the same rule against the latest
     `fw-pala-v*` tag. Bump it when the Kivori Pip firmware changed since the last release (look for
     commits touching `firmware/pala/` or a shared crate it uses). While no `fw-pala-v*` tag exists
     yet, `none` also tags the current firmware (`fw-pala-v0.1.0` with the first release), so the
     firmware lineage starts tagged;
   - `dry_run`: tick it first to see the versions and the notes without changing anything. The run's
     summary page shows both.
3. The `start` job then:
   - refuses to run anywhere but `main`, and refuses a version that is not greater than its latest
     tag, below its `Cargo.toml` version, or whose tag (`vX.Y.Z`, `fw-pala-vA.B.C`) already exists
     (`scripts/release-bump.mjs`);
   - writes `[workspace.package] version` in `Cargo.toml` (and `version` in
     `firmware/pala/Cargo.toml` when bumped), refreshes both `Cargo.lock` files with
     `cargo update --workspace`, and fails if anything else changed;
   - commits `chore(release): vX.Y.Z` (the body names the firmware version) as `github-actions[bot]`,
     tags `vX.Y.Z` (and `fw-pala-vA.B.C`, which gets no GitHub Release), and pushes commit and
     tags to `main` in one atomic push. If `main` moved after you pressed the button, the push is
     rejected and nothing lands: run it again. When no version changes (the first release with
     `none`), there is no version commit: the current `main` commit is tagged as it is and only the
     tags are pushed.
4. It then calls `release.yml` with the tag, which builds the firmware, builds the installer, installs
   and uninstalls it on a clean runner, and creates the **draft** release (never a pre-release): title
   `Kivori X.Y.Z · Pip firmware A.B.C`, notes, and the files `Kivori_X.Y.Z_x64-setup.exe`, its
   `.sig`, `latest.json` and `SHA256SUMS`.
5. Open Releases on GitHub, check the draft (edit the notes if a line reads badly), and publish it as a
   normal release (never tick pre-release; the updater ignores pre-releases). Installed apps see it
   within a day. Do the [validation.md](./validation.md) Phase 10 rows on a real machine first for a
   beta build.

**Release notes** come from `git log` between the previous `v*` tag and the new one
(`scripts/release-info.mjs`; no changelog files): `feat` commits under Features, `fix` under Fixes,
`perf` under Performance, and `!` or a `BREAKING CHANGE:` footer under Breaking changes; `chore`,
`docs`, `test`, `refactor`, `ci` and commits that are not conventional are left out, and so are
merge commits. The app list skips commits that only touch `firmware/`, `hardware/`, `apps/web/`,
`docs/` or `.github/`, and `web`-scoped commits. A Firmware section
follows: the bundled version and "unchanged", or, when the version changed, the firmware commits
(those touching `firmware/pala/`, or its earlier homes `firmware/desk-buddy/` and
`firmware/esp32-c3/`) grouped the same way. The release body ends with a compare
link; `latest.json` (shown in the app's update dialog) carries the same notes without it. The
first release has no previous `v*` tag, so instead of the whole history its body is a short "First
release" note: the app version, "Kivori Pip firmware 0.1.0", and a line pointing to the README for
the features.

**First time:** nobody has Kivori installed yet, so the first release (0.1.0) is installed by hand
from its installer; every release after it reaches it by itself.

**Versions reset (2026-10-12):** before the first release, the app and the firmware restarted at
0.1.0 and the protocol at 0.1. Boards flashed before the reset speak protocol 1.x; reflash them once
with `just fw-flash` (or Flash firmware). The app tells such a board to "update the Kivori app" (its
protocol major is newer than the app's) and never offers the flash by itself; the Device page's
Flash firmware button, after its confirmation, restores it as an incompatible device.

A `v*` tag pushed by hand still works (a hotfix from a branch, for example): bump `version` in
`Cargo.toml` under `[workspace.package]`, update `Cargo.lock` (and `firmware/pala/Cargo.lock`,
which records the shared crates' version), commit, then tag `vX.Y.Z` and push the tag. `release.yml`
fails at the first step if the tag and the workspace version differ, creates the draft itself and fills
it the same way. A re-run for the same tag updates the draft; a published release is never changed.

To try the pipeline without a tag, run the `release` workflow by hand (Actions, Run workflow); it
uploads the installer as a workflow artifact and creates no release. Pull requests that touch
packaging files run it too, and never read the signing secrets. The release scripts' tests:
`node --test scripts/release-tools.test.mjs` (the Release button runs them first).

## Verifying an app update end to end

Once the public key is in the release config and both secrets are set (validation.md rows 14.6 to
14.10, on a real Windows machine):

1. Release and publish version A (for example 0.2.0) as above, and install it from its draft's
   installer. Device page: the Updates rows show "Last checked" within 30 seconds of start.
2. Release and publish version B (0.2.1). Restart A, or press "Check now": the tray gains "Update to
   v0.2.1…", and the window shows "Kivori 0.2.1 is ready" with B's notes.
3. "Install & restart": the download bar, then the installer runs with a progress window only, and
   Kivori comes back as B (Device page, App version) and reconnects the device.
4. With A installed again: "Skip this version", restart, press "Check now": no offer. Publish C: it is
   offered. "Later" hides an offer until 3 days have passed (or "Check now").
5. Offline (or with "Check for updates automatically" off and a firewall log): no error, no wait, no
   request to github.com except after "Check now".
6. Developer test of the signature: the app only accepts the release endpoint, so build a test copy
   with `ENDPOINT` in `src-tauri/src/updater/policy.rs` and the release config's `endpoints` pointed at
   a server you control, then serve a `latest.json` whose `signature` belongs to another file (or edit
   one byte of the installer behind it): the dialog says the update could not be installed, and
   nothing changes.

The updater reads only `https://github.com/cunilab/Kivori/releases/latest/download/latest.json`, which
GitHub serves from the newest published, non-pre-release release; drafts are invisible to it.

## macOS

Run `just bundle-macos` (or `scripts/bundle-macos.sh`) on a Mac with Xcode Command Line Tools. The
`.app` and `.dmg` land in `target/release/bundle/`. Set `APPLE_SIGNING_IDENTITY` to sign. To build it
in CI, run the workflow by hand with `macos` ticked.

## Updating espflash

Change `ESPFLASH_VERSION` and the SHA-256 in `scripts/fetch-espflash.sh`, `scripts/fetch-espflash.ps1`
and `release.yml`, and `THIRD_PARTY_NOTICES.md`. Take the hashes from the official release page and
confirm them by downloading. Refresh the classifier fixtures in `tests/firmware_flash.rs` from the new
version's output.

Per-unit flashing and QA of new units: see [provisioning.md](./provisioning.md).

## Known limits

- Unsigned: SmartScreen warns, and Defender may block `espflash.exe` (the app then reports the tool
  as missing).
- The WebView2 bootstrapper needs a network connection on a PC that does not have WebView2 yet.
