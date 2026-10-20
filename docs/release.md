# Cutting a release

Releases start only when you decide. There is no standing release PR, and merging ordinary work
never cuts a release. `.github/workflows/release-start.yml` (the **Release** button) has two stages:
`prepare` opens an on-demand version PR; `finalize` tags its exact merged, CI-green commit.
`.github/workflows/release.yml` builds, smoke-tests, signs for the updater and attaches files to a
draft. Nothing is published until you publish the draft. Windows code
signing still waits on owner certificates (#36); see
[architecture.md](./architecture.md#release-pipeline-m3-s5) and ADR-0014/0015.

## Release policy

**Version promises.** App 1.0.0 ships with M4/v1, after its acceptance gates pass. From 1.0.0,
forward config migrations preserve supported user configuration, the app maintains the current
and previous protocol-major support window, and breaking changes to supported app behavior require
an app major bump. Wire compatibility has its own protocol version and capability rules
([ADR-0002](./adr/0002-one-wire-protocol-crate.md)); firmware versions remain per product, never
lockstep with the app. Below 1.0, beta behavior and configuration may change; release notes must
call out breaking changes. These are release obligations, not a claim that future compatibility
has already been tested. Downgrade support is not promised.

**Beta cadence.** Release when there is a user-visible improvement or fix, not on every merge.
Update prompts cost attention. Keep the manual Release button, Later and Skip this version;
urgency of a safety or recovery fix can justify an earlier release.

**Firmware release guard (M2.8).** The Release button runs `scripts/firmware-release-guard.mjs`
through `release-bump.mjs` before writing versions or outputs. It needs Rust/Cargo and full Git
history/tags, including for a dry run; the workflow installs Rust before computing the plan.
The guard checks every active catalog product and every firmware crate used by its boards, including
products with no requested bump. Crate paths come from packaging's validated plans. Firmware crates
must be explicit standalone workspace roots with explicit `X.Y.Z` package versions, and all boards
of one product must share a version. Shared firmware crates must resolve to the same version for
all claiming products; conflicting requests fail before mutation. Each product retains its own tags.
Planned app/firmware manifest changes are checked too: an app bump requires firmware bumps when
shared crates reflect workspace version metadata. Catalog and packaging changes conservatively
count as inputs to every product, so this release-tool change itself requires firmware bumps.
Full `cargo metadata --locked` resolution validates root and every active firmware lockfile before
any release writes versions or outputs, including first releases and firmware bumps. Stale locks are
refused instead of being silently repaired afterward.
The resolved firmware graph includes local patches and build dependencies. Project Cargo config
`paths` overrides are refused; use supported path dependencies or manifest patches instead.
Cargo config directories from the product crate up to the repository root are checked and watched,
including `firmware/.cargo/`. With current lockfiles,
[Cargo's workspace update rule](https://doc.rust-lang.org/cargo/commands/cargo-update.html)
preserves existing external dependency versions while refreshing workspace version metadata.

Inspect firmware inputs since the latest released firmware tag
(`fw-<product>-vX.Y.Z`) on the release commit's ancestry. Do not use an unrelated branch's highest
version as the input-diff baseline. Refuse ambiguous/divergent firmware history rather than silently
using an older tag; retain the separate version-monotonicity checks against existing tags.
If inputs changed, the resolved firmware version must be greater than that released version. Check
the resolved version, not whether the bump selector says `none`: an exact
version override can satisfy the rule, and an exact unchanged version cannot bypass it. Refuse
missing baseline history; fetch all tags/history. First release applies only when no
`fw-<product>-v*` tag exists anywhere in the repository. If tags exist but none is an ancestor of
the release commit, refuse instead of treating it as a first release. For a genuine first release,
tag the current version under the existing version checks, without an invented diff baseline.

Enumerate Cargo path dependencies and build-dependencies from the product's firmware workspace,
plus explicit non-Cargo inputs checked against its build scripts. For Pip this includes `products/`,
`assets/`, `tools/asset-compiler/`, relevant shared crates, root and firmware `Cargo.toml`/lockfiles,
`rust-toolchain.toml`, and applicable `.cargo/` and packaging/build configuration. Tests must prove
catalog, asset and asset-compiler changes require a bump. A conservative scope may require an extra
bump; do not silently miss a firmware input. Explain failures with the product, baseline and commits.

Exclude harmless version bookkeeping by content, never by commit title or author: only workspace
package-version metadata and corresponding workspace path-crate lockfile version entries proven
not to affect firmware behavior. Dependency source/checksum changes and all other hunks still count,
including real changes mixed into a release commit. If firmware starts consuming shared-crate
version metadata, that metadata can no longer be exempt. The firmware's own version and image
identity/version validation remain subject to the existing release checks.

Exercise changed/unchanged inputs, exact overrides, first release, unreachable/divergent tags,
bookkeeping-only and mixed bookkeeping/real changes, and failure before mutation in release-tool
tests. The suites run in host CI and before the Release button computes versions. Unchanged-version
checks require clean committed inputs so local edits cannot be silently omitted from the Git diff.
Non-Cargo inputs are explicitly listed; the inventory test checks the current firmware's declared
build inputs, and new build/packaging inputs must extend that list. Hand-pushed app tags bypass the
Release button, so their author must still review firmware changes and bump/tag it when needed.

Release titles and the Firmware notes section must cover every bundled product using catalog names
and independent firmware versions. M2.8 must also replace the single-image bundle with validated
catalog-driven images per supported product/board and resolve update/flash targets by identity.
Test two isolated product fixtures through packaging selection and release metadata; refuse
mismatched images before flashing. Release validation must verify that the bundled image set equals
the shipping catalog's supported product/board set, with no test products or images. The pipeline below builds and names every active catalog product. There remains one app release, firmware tags have
no GitHub Release, and the owner reviews the draft before publication. See
[ADR-0017](./adr/0017-product-contract-and-capability-driven-desktop.md).

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
5. Enable Actions' permission to create pull requests (Settings, Actions, General). The prepare
   job needs contents and pull-request write access; finalize needs only tag writes plus
   PR/Actions reads. No personal token, GitHub App or bypass of `main` protection is required.
6. Protect `main`: require PRs and the always-running host, firmware and determinism checks; block
   force pushes and deletion. Keep squash merging enabled and automatically delete merged branches.
   Do not require path-filtered website or packaging jobs for every PR. With one maintainer, the
   owner's explicit merge decision is the review gate; requiring a GitHub approval also requires
   someone other than the PR author. Release preparation is bot-authored, so the owner can approve it.

## Trunk and version tags

`main` is the single development trunk. Small topic branches target it, pass CI and are squash-merged
frequently, then deleted. There is no permanent `develop` or release branch. App versions live in
`[workspace.package]`; product firmware versions live in the catalog firmware crates. A merge does
not bump them automatically. App tags `vX.Y.Z` and independent `fw-<product>-vA.B.C` tags identify
immutable commits; never move or reuse a tag. Release-request branches are temporary PR branches.

Prepare creates a draft version PR because a PR created with `GITHUB_TOKEN` does not trigger normal
PR CI, and manually dispatched job checks do not satisfy PR rulesets. The owner marks the draft ready
for review; this user-originated event starts normal host, firmware and determinism PR checks.
After the owner squash-merges, ordinary `main` push CI validates the exact squash commit. Finalize requires those three workflows to pass on that
SHA, not on the source branch or a newer `main` commit. It reruns version/firmware guards, tags that
SHA and pushes all new tags atomically without pushing a branch. A retry may rebuild the same draft;
an existing tag that names another commit is refused.

## Windows

1. Merge pull requests with conventional commit titles (`feat:`, `fix:`, `perf:`, scoped like
   `feat(mic): ...`); they become the release notes.
2. When you want a release: Actions, **Release**, Run workflow, on `main`, with `stage=prepare`. Pick:
   - `app_bump`: `none` (default), `patch`, `minor` or `major`. A bump starts from the version in
     `Cargo.toml`; below 1.0.0 a major is a real major (0.4.2 becomes 1.0.0). `none` releases the
     `Cargo.toml` version as it is, which is only possible while it has no `v` tag. Preparation
     requires at least one version edit to create a reviewable PR; a versionless request is refused.
     For the first release, choose an explicit bump if all versions are still unchanged. `app_version`: an exact `X.Y.Z` instead. Whatever you pick must be
     greater than the latest `v*` tag (with no tag yet, the current version itself is allowed) and is
     never below the version in `Cargo.toml`;
   - `product`: optional active product id for the single-product firmware controls. With one
     product it may be empty; with multiple products, a non-`none` bump or exact override requires
     a selection. `firmware_bump`: `none` (default), `patch`, `minor` or `major`;
     `firmware_version`: an exact `A.B.C` instead, under the same rule against that product's latest
     `fw-<product>-v*` tag. The input guard refuses an unchanged firmware version when firmware or build
     inputs changed, listing the baseline, paths and commits; choose a bump or newer exact version.
     While no `fw-<product>-v*` tag exists, `none` also tags its current version, starting its lineage;
   - `firmware_overrides`: for different changes in one run, a JSON map such as
     `{"product-a":"patch","product-b":"0.3.0"}`. Keys must be active product ids; values are
     `none`, `patch`, `minor`, `major`, or plain `X.Y.Z`. Omitted products keep their versions but
     still run the guard. This input cannot mix with `product`, a non-`none` `firmware_bump`, or
     `firmware_version`. Bumps start from each product's independent current version;
   - `dry_run`: tick it first to see the versions and the notes without changing anything. The run's
     summary page shows both.
3. The `start` job then:
   - refuses to run anywhere but `main`, and refuses a version that is not greater than its latest
     tag, below its `Cargo.toml` version, or whose new tag (`vX.Y.Z`, `fw-<product>-vA.B.C`) already exists
     (`scripts/release-bump.mjs`);
   - validates the entire plan before writing `[workspace.package] version` and the changed
     catalog firmware package versions. Refreshes root and every unique firmware workspace lockfile
     with `cargo update --workspace`, even for products whose firmware keeps its version (the app
     version can change shared-crate lock entries). Fails on changes outside the planned manifests
     and lockfiles;
   - commits `chore(release): vX.Y.Z` on `release/vX.Y.Z`, pushes only that branch, opens a version
     draft PR into `main`. Mark it ready for review to start CI. No tags or draft release are created. An existing `release/v*` branch
     blocks another request: finish or discard that PR first. If PR creation fails, the
     owner can open the existing branch's PR or close/reopen it to start missing PR CI; do not force-push it.
4. Mark the draft ready for review, review and squash-merge the version PR after required PR CI passes. Wait for host, firmware and determinism push CI on its
   merged commit to pass. Run **Release** again on `main` with `stage=finalize`, `release_pr` set to
   that PR number, and `dry_run=false`. Other version inputs apply only to prepare. Finalize requires
   a same-repository `release/vX.Y.Z` PR merged into `main` with only catalog manifest/lockfile edits.
   The PR must be bot-authored and squash-merged. Its commit must still be on `main`, with a matching
   workspace version and passing CI. Manifest contents may change only versions; lockfiles may
   change only the corresponding local-package versions and dependency references.
   Missing/failed CI, non-version edits, version mismatches, unexpected files and conflicting tags
   stop before tag push. The script runs in the disposable CI checkout, which it leaves at the
   approved SHA. Only one Release run executes at a time; wait for completion before another request.
5. Finalize then calls `release.yml` with the tag, which builds the firmware, builds the installer, installs
   and uninstalls it on a clean runner, and creates the **draft** release (never a pre-release): title
   `Kivori X.Y.Z · Pip firmware A.B.C` for today's one-product catalog, adding each further
   product and independent firmware version in registry wire order. Notes cover every product; files are `Kivori_X.Y.Z_x64-setup.exe`, its
   `.sig`, `latest.json` and `SHA256SUMS`.
6. Open Releases on GitHub, check the draft (edit the notes if a line reads badly), and publish it as a
   normal release (never tick pre-release; the updater ignores pre-releases). Installed apps see it
   within a day. Do the [validation.md](./validation.md) Phase 10 rows on a real machine first for a
   beta build.

**Release notes** come from `git log` between the previous `v*` tag and the new one
(`scripts/release-info.mjs`; no changelog files): `feat` commits under Features, `fix` under Fixes,
`perf` under Performance, and `!` or a `BREAKING CHANGE:` footer under Breaking changes; `chore`,
`docs`, `test`, `refactor`, `ci` and commits that are not conventional are left out, and so are
merge commits. The app list skips commits that only touch `firmware/`, `hardware/`, `apps/web/`,
`docs/` or `.github/`, and `web`-scoped commits. A Firmware section
follows for each product: the bundled version and "unchanged", or, when the version changed, commits
touching that product's catalog crate directories, grouped the same way. Pip's earlier homes
`firmware/desk-buddy/` and `firmware/esp32-c3/` still count. Custom firmware crate paths also stay out
of the app-only list. A product absent at the previous app tag is listed without an invented baseline. The release body ends with a compare
link; `latest.json` (shown in the app's update dialog) carries the same notes without it. The
first release has no previous `v*` tag, so instead of the whole history its body is a short "First
release" note: the app version, every product firmware version, and a line pointing to the README for
the features.

**First time:** nobody has Kivori installed yet, so the first release (0.1.0) is installed by hand
from its installer; every release after it reaches it by itself.

**Versions reset (2026-10-12):** before the first release, the app and the firmware restarted at
0.1.0 and the protocol at 0.1. Boards flashed before the reset speak protocol 1.x; reflash them once
with `just fw-flash` (or Flash firmware). The app tells such a board to "update the Kivori app" (its
protocol major is newer than the app's) and never offers the flash by itself; the Device page's
Flash firmware button, after its confirmation, restores it as an incompatible device.

A `v*` tag pushed by hand still works (a hotfix from a branch, for example): bump `version` in
`Cargo.toml` under `[workspace.package]`, update root and all catalog firmware lockfiles
(which record shared-crate versions), commit, then tag `vX.Y.Z` and push the tag. `release.yml`
fails at the first step if the tag and the workspace version differ, creates the draft itself and fills
it the same way. A re-run for the same tag updates the draft; a published release is never changed.

To try the pipeline without a tag, run the `release` workflow by hand (Actions, Run workflow); it
uploads the installer as a workflow artifact and creates no release. Pull requests that touch
packaging files run it too, and never read the signing secrets. The release scripts' tests:
`node --test scripts/release-tools.test.mjs scripts/firmware-release-guard.test.mjs scripts/release-finalize.test.mjs`
(host CI and the Release button run both suites). TOML-specific cases invoke Bun, the release CLI's
runtime. Version reads use the same TOML parser as the catalog; edits preserve quotes, comments and
line endings. The editor requires a plain `version` assignment in the named table and refuses any
edit that changes another parsed value, before writing manifests or workflow outputs.

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

The macOS override disables updater artifacts: automatic app updates currently support Windows
only, so a local Mac bundle does not need the updater signing private key. Without an Apple
signing identity the bundle is not Developer ID signed; notarization is not implemented.

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

## Catalog firmware builds and packaging

`bun scripts/build-firmware.mjs` builds one selected catalog pair. `just fw-build` and `provision`
use it. Set both `KIVORI_PRODUCT` and `KIVORI_BOARD`, or pass `--product <id> --board <id>`; no
selection is needed for a sole active pair. Empty, partial, unknown and ambiguous selections fail.
Every active pair must have a valid build recipe; registry reservations without metadata are not builds.

Desktop `app`/`desktop`, both bundle recipes and `scripts/build-desktop-firmware.ps1 [-Release]`
instead build **every** active catalog pair through `scripts/build-firmware-set.mjs`. They ignore
single-pair selector environment variables. PowerShell no longer accepts `-Product`/`-Board`.
Each build uses its catalog target/features, locks dependencies, disables default features, names the
binary explicitly, and pins the child target directory. Global `RUSTFLAGS` and
`CARGO_ENCODED_RUSTFLAGS` must be unset so the firmware retains its linker configuration.

The set builder requires a fresh absolute output directory and an absolute manifest path outside it:

```bash
staging="$(mktemp -d)"
bun scripts/build-firmware-set.mjs --out-dir "$staging/images" --manifest-out "$staging/manifest.json"
```

It validates all plans before building, sorts by wire product/board, and copies each successful image
to `firmware-<product-number>-<board-number>.elf` immediately, before another build can overwrite a
shared Cargo artifact. Only a complete successful set gets a manifest; existing outputs/manifests
are refused. Child logs go to stderr and stdout contains only the manifest path. Desktop wrappers
clear the legacy image input, use temporary staging and clean it afterward. PowerShell restores both
firmware input variables and the desktop target-directory environment on success or failure.

Local builds need each catalog Rust target/toolchain installed. `--targets` lists deduplicated targets
for CI installation. Release CI uses stable; catalog crate toolchains must match it. The Ubuntu job
builds all images and uploads **only the ELF files**, not its host-specific manifest. Windows checks
the downloaded directory and creates its own absolute manifest:

```bash
bun scripts/build-firmware-set.mjs --from-dir /absolute/download --manifest-out /absolute/manifest.json
```

Missing/extra entries, subdirectories, symlinks and empty images are refused. Native embedding then
validates ELF identities, versions and protocols and requires manifest mode to equal the entire
active catalog identity set, independent of filenames or input order. An exclusive
`KIVORI_FIRMWARE_PATH` still accepts one targeted image for development/provisioning;
`KIVORI_FIRMWARE_MANIFEST` is an absolute JSON file containing the complete set of absolute paths.
Unset inputs embed no firmware. Single-pair selection never narrows a desktop package.

These commands build software. Provisioning/flashing remains a separate explicit operation.
Release planning and titles/notes consume the same complete catalog. The Release button validates
every lineage before writes, refreshes every unique lockfile, stages only planned paths, and atomically
pushes the app tag and all new product firmware tags only after the version PR merges and its exact
commit passes main CI. Version commits travel through PRs, never a direct push to main. Preview notes receive
the complete planned version map, so dry-run and written-version notes agree. Release CLIs require
Bun for the shared TOML catalog reader; the Node test suites invoke that real reader through Bun.

For example (test product ids shown):

```bash
bun scripts/release-bump.mjs --app patch --firmware-overrides '{"product-a":"patch","product-b":"0.3.0"}' --dry-run
```

No actual release is triggered by local planning or tests. Hardware acceptance remains separate M2.8 work. Ambiguous recovery targets require an explicit
product-and-board choice and confirmation before flashing.
