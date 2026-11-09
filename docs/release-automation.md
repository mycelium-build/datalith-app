# Release automation setup

Configure the following under **Settings → Secrets and variables → Actions**.

Repository secrets:

- `AUTOMATION_APP_CLIENT_ID`: the GitHub App Client ID.
- `AUTOMATION_APP_PRIVATE_KEY`: the GitHub App private key in PEM format.

The GitHub App requires **Contents: read and write** and **Pull requests: read and write** repository permissions. It also requires **Actions: write** permission on the website repository (`mycelium-build/datalith`) so release CI can trigger its deployment workflow.

After CI succeeds for a push to `main`, **Prepare release** runs Release Please to create or update a release PR when there are releasable changes. The PR updates `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, the release manifest, and the application version in `THIRD-PARTY-NOTICES.md`.

Versioning follows conventional commits: `fix:` bumps the patch, `feat:` bumps the minor, and `!` or a `BREAKING CHANGE:` footer bumps the major. `bump-minor-pre-major` is explicitly disabled, so breaking changes also bump the major while the current version is `0.x`. The breaking changes accumulated since v0.1.0 therefore make the next stable release v1.0.0, with normal automatic versioning continuing afterward.

To publish a release candidate, manually run **Create release candidate** from the Actions tab. It requires exactly one open Release Please PR whose branch contains the current `main` commit. It tags that PR's head as `vX.Y.Z-rc.N`, using the version from its `Cargo.toml` and the next available RC number. Pushes to `main` do not create RC tags automatically. The same workflow then starts **Release** for the new tag with `workflow_dispatch` from `main` (mode `build`) instead of relying on the tag push, which RC tags no longer trigger.

To publish a stable release, merge the Release Please PR into `main`, preserving its generated release title and metadata. [Release Please supports both squash merges and merge commits](https://github.com/googleapis/release-please#whats-a-release-pr). The release PR already passed CI, so on the merge push **Prepare release** immediately creates a draft release and its `vX.Y.Z` tag (`force-tag-creation`), without waiting for CI on `main`. The tag must exist even while the release is a draft: Release Please finds the previous release through it, and would otherwise recount every change since the bootstrap commit and bump the version a second time. The stable tag is not a promotion of the previously tagged RC commit, so it can point to a different commit after merging.

Stable tags trigger **Release** on push, and RC tags reach it through **Create release candidate**. **Release** builds Linux, macOS, and Windows artifacts, runs license-compliance checks, and uploads the vendored source archive, legal documents, and SBOM. The release is published after these jobs succeed, and RC releases are marked as prereleases. Publication also triggers deployment of the website.

Each target is compiled by a **Build** job and packaged by a separate job named after the target. **Build** uploads the binary as a run artifact. The packaging job downloads it, then packages, signs, notarizes and uploads it to the draft release. If packaging, signing or notarization fails, use **Re-run failed jobs** on the run: only the failed packaging jobs (and finalization) run again, from the binary that was already built. Builds do not wait for license compliance or the draft release, and they never see the signing secrets.

**Build** keeps a Rust dependency cache per target. Its key follows the toolchain, `Cargo.toml` and `Cargo.lock`, so all RCs of a version and their stable release share it. GitHub isolates caches by tag, so `vX.Y.Z-rc.2` could never read the cache of `vX.Y.Z-rc.1`; every run can, however, read the caches saved on `main`. RC builds therefore run from `main` and save the cache there, even when a later step fails, and later RCs and the stable release restore it. Stable tag runs only restore it. GitHub evicts cache entries unused for 7 days.

## Linux distribution notification

After a stable release is finalized, **Release** triggers `distribute.yml` in
[`mycelium-build/linux-packages`](https://github.com/mycelium-build/linux-packages).
That repository records the release assets and checksums, then publishes signed pacman, APT and RPM repositories using the existing packages. Release candidates are excluded.

To retry, run **Publish Linux packages** in that repository on `main`, with
`app=datalith`, the published tag (`vX.Y.Z`), and `publish=true`. Check its Actions for
the result. Publication runs independently in that repository; it does not recompile or test Datalith.

## Build channels

`CHANNEL` defaults to `dev`. Release CI stamps `stable` for `vX.Y.Z` tags and `preview` for `vX.Y.Z-rc.N`, then builds the same Rust target. Before packaging, `scripts/prepare-release.py` checks `--print-build-identity` against the selected `packager.stable.toml` or `packager.preview.toml` and prepares the named binary and versioned packaging configuration.

| Channel | Product | Installed executable | Identifier / app data suffix |
| --- | --- | --- | --- |
| Stable | Datalith | `datalith` | `datalith` |
| Preview | Datalith Preview | `datalith-preview` | `datalith-preview` |
| Dev | Datalith Dev | Cargo's local `datalith` target | `datalith-dev` |

Identifiers use the `build.mycelium.` prefix. App data lives under the OS data directory. Stable keeps its existing data; Dev starts fresh without migration. User-selected Vaults can be opened by either product. Stable keeps caches in `.datalith`; Preview and Dev use `.datalith/preview` and `.datalith/dev`.

Preview uses yellow launcher icons and a yellow in-app monolith; the UI theme is unchanged. Dev uses a green logo. Run `python3 scripts/app_icon.py` to regenerate the flat icon sets (SVG, PNG, hicolor, ICO) of all three channels. The macOS icons are layered Icon Composer documents (`assets/logo/<channel>/datalith.icon`) so macOS applies Liquid Glass to each part of the monolith; run `uv run scripts/macos_icon.py` on a Mac with Xcode 26 or later to regenerate them and compile each into `Assets.car` (named by `CFBundleIconName` in `packaging/macos/Info.plist`), the fallback `datalith.icns` and the `datalith-macos.png` rendition. The script stops with an explanation when Icon Composer is unavailable. `scripts/prepare-release.py` adds `Assets.car` and the Info.plist overlay to the packager configuration of macOS targets. About shows the Git commit for Dev, the full RC version for Preview, and the release version for Stable. NSIS uses separate product names for install directories/uninstall entries and separate executable names for process detection. macOS uses separate app names and bundle identifiers. Linux packages use separate names, executables, desktop entries, icons, and license paths. The macOS DMG window has a blue or yellow background from `assets/dmg/`; run `uv run scripts/dmg_background.py` to regenerate both, and keep its layout constants in sync with `[dmg]` in the packager configurations.

Preview points at `https://updates.datalith.work/preview` and receives only published `vX.Y.Z-rc.N` releases. Stable points at `https://updates.datalith.work/stable` and receives only stable releases.

## macOS signing and notarization

macOS apps and DMGs are signed with a **Developer ID Application** certificate and notarized by Apple, so Gatekeeper opens them without warnings. This uses the paid Apple Developer Program membership and requires no approval from Apple: notarization is an automated scan. The macOS jobs fail if any of these repository secrets is missing:

- `APPLE_CERTIFICATE`: the Developer ID Application certificate and its private key, exported as `.p12` and base64-encoded.
- `APPLE_CERTIFICATE_PASSWORD`: the `.p12` export password.
- `APPLE_SIGNING_IDENTITY`: the certificate's full name, e.g. `Developer ID Application: Name (TEAMID)`.
- `APPLE_API_KEY`: the App Store Connect API key ID used by `notarytool`.
- `APPLE_API_ISSUER`: that key's issuer ID.
- `APPLE_API_PRIVATE_KEY`: the contents of that key's `AuthKey_<KEY_ID>.p8` file.

Only the Account Holder can create Developer ID certificates. Create one from Xcode (**Settings → Accounts → Manage Certificates → + → Developer ID Application**), then export it from Keychain Access (**My Certificates**, certificate with its private key, `.p12`). Create the API key in App Store Connect under **Users and Access → Integrations → App Store Connect API**, with the *Developer* role; its `.p8` file can only be downloaded once.

```sh
base64 -i DeveloperID.p12 | gh secret set APPLE_CERTIFICATE --repo mycelium-build/datalith-app
gh secret set APPLE_CERTIFICATE_PASSWORD --repo mycelium-build/datalith-app
gh secret set APPLE_SIGNING_IDENTITY --repo mycelium-build/datalith-app
gh secret set APPLE_API_KEY --repo mycelium-build/datalith-app
gh secret set APPLE_API_ISSUER --repo mycelium-build/datalith-app
gh secret set APPLE_API_PRIVATE_KEY --repo mycelium-build/datalith-app < AuthKey_<KEY_ID>.p8
```

cargo-packager signs the app with the hardened runtime, notarizes and staples it, then builds and signs the DMG. The workflow then notarizes and staples the DMG and checks both with `codesign`, `spctl`, and `stapler`. The updater bundle is archived from the stapled app. Check the certificate's expiry date (`security find-certificate -c "Developer ID Application" -p | openssl x509 -noout -enddate`) and renew the secrets before then. Already-notarized releases keep working after the certificate expires.

## Automatic updates

Stable releases newer than `v0.1.0` publish a signed update for each of the six OS/architecture targets. `v0.1.0` predates the updater and is excluded from discovery. RC releases publish signed Preview updates. Each deployment regenerates both manifests from GitHub releases, selecting each channel independently.

Configure this application's repository (`mycelium-build/datalith-app`) under **Settings → Pages → Source → GitHub Actions**. Allow release tags and the default branch to deploy to the `github-pages` environment. No `gh-pages` branch or cross-repository token is needed. The website repository remains separate.

The final release step deploys the `stable` and `preview` manifests at `https://updates.datalith.work/` using the workflow's `GITHUB_TOKEN`. Missing bundles or signatures, duplicate assets, wrong filenames, and invalid signatures fail validation before the draft is published. Manifest generation verifies the signatures again, selects the highest published version in each channel, and waits for Pages deployment to succeed. RC numbers compare numerically (`rc.10` follows `rc.9`). Drafts and releases whose prerelease flag disagrees with their channel are excluded. Release finalization and regeneration share a concurrency group.

### Signing key

- `DATALITH_UPDATER_PRIVATE_KEY` is the repository Actions secret containing cargo-packager's base64 private key, with an empty password. Only the signing step receives it.
- `scripts/updates/public-key.txt` contains the matching public key. `build.rs` embeds it in release builds; the fake server can override it in debug builds.
- The initial owner-only backup is at `~/.local/share/datalith-release/updater.key` on the maintainer's PC. Keep a second copy in a secure offline backup or password manager accessible only  to release maintainers. GitHub does not let you retrieve a stored secret.

To restore the Actions secret from the backup:

```sh
gh secret set DATALITH_UPDATER_PRIVATE_KEY --repo mycelium-build/datalith-app < ~/.local/share/datalith-release/updater.key
```

For planned rotation, generate a new keypair with `cargo packager signer generate --ci --path <secure-backup-path>`. Rotation requires a dedicated transition change: build the app with the new public key while CI still signs and validates the transition release with the old key. The current build and validation share one key file, so changing that file alone is insufficient. After distributing the transition release, switch CI signing and validation to the new key and restore the shared key file. Older installations that miss the transition need a manual install; the current single-key updater does not provide a permanent rotation bridge. Test and coordinate rotation before changing either key.

If the private key is lost, recover the backup before releasing. Without a backup, existing installations cannot trust a replacement key and require a manual install. If compromised, stop update publication, remove affected releases, replace the key and secret, and distribute a clean installer through a trusted channel. A compromised old key cannot authenticate its own recovery.

### Removal and recovery

Deleting or editing a release runs **Regenerate update manifests** from the default branch. It selects the highest remaining published release in each channel, verifies each complete signed bundle set, and deploys both manifests. A channel with none remaining publishes version `0.0.0`, which installed releases treat as no update. Removing a release does not revoke bytes already staged in a running app.

If packaging or signing fails for a target, run **Re-run failed jobs** on the same run: it reuses the built binaries. A re-run keeps the workflow file of the original run, so when the fix is in the workflow itself, merge it to `main` instead and run **Release** from `main` with the existing tag, the `package` mode and the failed run's ID in `build_run`. This packages, signs and uploads every target again from that run's binaries, without compiling, then finalizes; the binaries' build identity must match the tag, and they expire 7 days after the build. If builds or signature validation fail in a way a re-run cannot fix, leave the release as a draft, repair or rebuild its assets, and manually run **Release** with the existing tag to retry finalization. If only Pages publication fails after the release became public, run **Regenerate update manifests**. The same workflow is the recovery path for a missed release-edit/deletion event, including edits made using `GITHUB_TOKEN`, which do not trigger another workflow. Confirm the endpoint shows the expected version after deployment.

For a finalization retry, select `main` as the workflow branch to use corrected finalization scripts, enter the existing release tag in the `tag` input and keep the `finalize` mode. This skips all build jobs and reuses the uploaded assets. The SBOM and legal documents are taken from the release tag's source, even when the workflow runs from `main`. Explicit tag validation includes draft releases.

Bundle signatures authenticate update bytes; they are not Apple notarization or Windows Authenticode. The HTTPS manifest itself is unsigned: tampering can disrupt discovery, but cannot authorize an unsigned payload.

## Homebrew macOS tap

`mycelium-build/homebrew-tap` maintains the `datalith` and `datalith@preview` casks
from the published Apple Silicon and Intel DMGs. Both retain the integrated updater.

After deploying update manifests, **Release** dispatches the tap's `update.yml`
workflow on `main` with the published tag. Publish the tap first and install the
existing automation GitHub App on it with **Actions: write**. The existing app
secrets are reused; the tap commits updates with its own `GITHUB_TOKEN`.

The tap validates the artifacts before updating a channel and rejects version
downgrades. Dispatch does not wait for completion: inspect the tap's workflow run.
To retry, run `update.yml` there with the tag, or leave it empty to refresh both
channels. A tap failure does not unpublish the app release.
