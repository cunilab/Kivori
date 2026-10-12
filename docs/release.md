# Cutting a release

Releases are driven by conventional commits (`feat:`, `fix:`, `perf:`, scoped like `feat(mic): ...`).
Two workflows do the work: `.github/workflows/release-please.yml` (version, changelog, tag, draft) and
`.github/workflows/release.yml` (build, smoke test, sign for the updater, attach files). Nothing is
published until you publish the draft. Windows code signing still waits on owner certificates (#36);
see [architecture.md](./architecture.md#release-pipeline-m3-s5) and ADR-0014/0015.

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
4. Give Claude the **public** key (contents of `~/.tauri/kivori-updater.key.pub`).
   TODO(owner): the public key goes into `plugins.updater.pubkey` in
   `apps/desktop/src-tauri/tauri.release.conf.json`, which is still `""`. Until then every build's
   updater is off (Device page: "it cannot verify them yet") and no installed build can install an
   update. Once the `TAURI_SIGNING_PRIVATE_KEY` secret exists, `release.yml` refuses to build a release
   while that key is empty ("Updater public key missing"), so add both together.
5. Settings, Actions, General: tick "Allow GitHub Actions to create and approve pull requests" (release-please
   needs it to open the release PR).

## Windows

1. Merge pull requests with conventional commit titles. release-please keeps **one release PR** open
   ("chore: release ...") with the next app version, the next firmware version if firmware changed,
   and both changelogs (`CHANGELOG.md`, `firmware/desk-buddy/CHANGELOG.md`). Which commits count:
   - app: everything except `firmware/`, `hardware/`, `sim/` and the Wokwi tools; shared `crates/`
     count for the app only;
   - firmware: only `firmware/desk-buddy/`. A change to a shared crate that really alters the
     firmware needs a `fix(firmware): ...` (or `feat(firmware): ...`) commit touching the firmware
     folder, or a manual `Release-As` / version bump, to bump the firmware.
2. Merge the release PR when you want a release. The `release-please` workflow then:
   - tags `vX.Y.Z` and creates the app's **draft** release (not a pre-release);
   - tags `fw-desk-buddy-vA.B.C` if the firmware version changed (no GitHub Release for it);
   - calls `release.yml` with the tag.
3. `release.yml` builds the firmware, builds the installer, installs and uninstalls it on a clean
   runner, then fills the draft: title `Kivori X.Y.Z · Desk Buddy firmware A.B.C`, notes (the app
   changelog section plus a Firmware section), and the files `Kivori_X.Y.Z_x64-setup.exe`, its `.sig`,
   `latest.json` and `SHA256SUMS`.
4. The release PR branch gets a `chore: sync Cargo.lock ...` commit from CI (the lockfile holds the
   workspace versions). Its checks do not start by themselves: close and reopen the PR to run them
   before merging.
5. Open Releases on GitHub, check the draft, and publish it as a normal release (never tick
   pre-release; the updater ignores pre-releases). Installed apps see it within a day. Do the
   [validation.md](./validation.md) Phase 10 rows on a real machine first for a beta build.

Without release-please (hotfix by hand): bump `version` in `Cargo.toml` under `[workspace.package]` and
`.release-please-manifest.json`, update `Cargo.lock`, merge, tag `vX.Y.Z` and push the tag. The
workflow fails at the first step if the tag and the workspace version differ, creates the draft itself
and fills it the same way.

To try the pipeline without a tag, run the `release` workflow by hand (Actions, Run workflow); it
uploads the installer as a workflow artifact and creates no release. Pull requests that touch
packaging files run it too, and never read the signing secrets.

The first release PR only looks at history after `bootstrap-sha` in `release-please-config.json`;
remove that key after the first release PR is merged.

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
