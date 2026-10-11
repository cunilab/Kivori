# Cutting a release

The pipeline is `.github/workflows/release.yml`. It builds an unsigned Windows installer and a draft
GitHub release. Nothing is published until you publish the draft. Signing waits on owner certificates
(#36); see [architecture.md](./architecture.md#release-pipeline-m3-s5) for how it fits together.

## Windows

1. Bump the version once, in `Cargo.toml` under `[workspace.package]` (it is the only app version;
   `tauri.conf.json` has none). Update `Cargo.lock`, merge the bump to `main`.
2. Tag that commit with the same number and push the tag:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

   The workflow fails at the first step if the tag and the workspace version differ.
3. Watch the `release` workflow. It builds the firmware, builds the installer, installs and
   uninstalls it on a clean runner, then creates a **draft** release named `Kivori vX.Y.Z` with the
   installer and `SHA256SUMS`.
4. Open Releases on GitHub, find the draft, check the files, and publish it when ready. Do the
   [validation.md](./validation.md) Phase 10 rows on a real machine first for a beta build.

To try the pipeline without a tag, run the `release` workflow by hand (Actions, Run workflow); it
uploads the installer as a workflow artifact and creates no release. Pull requests that touch
packaging files run it too.

## Planned: M2.7 release flow

Not built yet ([roadmap.md](./roadmap.md#m27-product-identity-versioning-and-updates), ADR-0014 and
ADR-0015). Once it lands:

1. Merge pull requests with conventional commit titles (`feat:`, `fix:`, scoped by component).
2. release-please keeps one release PR open with the next app version, each changed firmware's next
   version and their changelogs. Merge it when you want a release.
3. That tags `vX.Y.Z` for the app (and `fw-<product>-vX.Y.Z` for each firmware that changed). The
   release workflow builds the installer, signs it for the updater, writes `latest.json` and
   attaches them to a draft release titled with the bundled firmware.
4. Check the draft and publish it as a normal release (not a pre-release): installed apps see it
   within a day.

Keep the updater signing key offline in two places. Losing it means installed apps can never
update themselves again.

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
