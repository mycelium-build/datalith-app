# Release automation setup

Configure the following under **Settings → Secrets and variables → Actions**.

Repository secrets:

- `AUTOMATION_APP_CLIENT_ID`: the GitHub App Client ID.
- `AUTOMATION_APP_PRIVATE_KEY`: the GitHub App private key in PEM format.

The GitHub App requires **Contents: read and write** and **Pull requests: read and write** repository permissions. It also requires **Actions: write** permission on the website repository (`mycelium-build/datalith`) so release CI can trigger its deployment workflow.

After CI succeeds for a push to `main`, **Prepare release** runs Release Please to create or update a release PR when there are releasable changes. The PR updates `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, the release manifest, and the application version in `THIRD-PARTY-NOTICES.md`.

To publish a release candidate, manually run **Create release candidate** from the Actions tab. It requires exactly one open Release Please PR whose branch contains the current `main` commit. It tags that PR's head as `vX.Y.Z-rc.N`, using the version from its `Cargo.toml` and the next available RC number. Pushes to `main` do not create RC tags automatically.

To publish a stable release, merge the Release Please PR into `main`, preserving its generated release title and metadata. [Release Please supports both squash merges and merge commits](https://github.com/googleapis/release-please#whats-a-release-pr). After CI succeeds, **Prepare release** creates a draft release and pushes its `vX.Y.Z` tag at the draft's target commit once successful CI covers that commit. The stable tag is not a promotion of the previously tagged RC commit, so it can point to a different commit after merging.

Both tag kinds trigger **Release**, which builds Linux, macOS, and Windows artifacts, runs license-compliance checks, and uploads the vendored source archive, legal documents, and SBOM. The release is published after these jobs succeed, and RC releases are marked as prereleases. Publication also triggers deployment of the website.

## Build channels

`CHANNEL` defaults to `dev`. Release CI stamps `stable` for `vX.Y.Z` tags and
`preview` for `vX.Y.Z-rc.N`, then builds the same Rust target. Before packaging,
`scripts/prepare-release.py` checks `--print-build-identity` against the selected
`packager.stable.toml` or `packager.preview.toml` and prepares the named binary
and versioned packaging configuration.

| Channel | Product | Installed executable | Identifier / app data suffix |
| --- | --- | --- | --- |
| Stable | Datalith | `datalith` | `datalith` |
| Preview | Datalith Preview | `datalith-preview` | `datalith-preview` |
| Dev | Datalith Dev | Cargo's local `datalith` target | `datalith-dev` |

Identifiers use the `build.mycelium.` prefix. App data lives under the OS data
directory. Stable keeps its existing data; Dev starts fresh without migration.
User-selected Vaults can be opened by either product. Stable keeps caches in
`.datalith`; Preview and Dev use `.datalith/preview` and `.datalith/dev`.

Preview uses yellow launcher icons and a yellow in-app monolith; the UI theme
is unchanged. Dev uses a green logo. Run `python3 scripts/app_icon.py` to
regenerate all three icon sets. About shows the Git commit for Dev, the full RC
version for Preview, and the release version for Stable.
NSIS uses separate product names for install directories/uninstall entries and
separate executable names for process detection. macOS uses separate app names
and bundle identifiers. Linux packages use separate names, executables, desktop
entries, icons, and license paths.

Preview points at `updates/preview.json`; publication of that stream is still
pending. RC releases cannot change `stable.json`.

## Automatic updates

Stable releases newer than `v0.1.0` publish a signed update for each of the six
OS/architecture targets. `v0.1.0` predates the updater and is excluded from
discovery. RC releases are signed and validated but do not change the stable
manifest.

Configure this application's repository (`mycelium-build/datalith-app`) under
**Settings → Pages → Source → GitHub Actions**. Allow release tags and the
default branch to deploy to the `github-pages` environment. No `gh-pages` branch
or cross-repository token is needed. The website repository remains separate.

The final release step deploys
`https://mycelium-build.github.io/datalith-app/updates/stable.json` using the
workflow's `GITHUB_TOKEN`. Missing bundles or signatures, duplicate assets,
wrong filenames, and invalid signatures fail validation before the draft is
published. Manifest generation verifies the signatures again, selects the
highest published stable SemVer, and waits for Pages deployment to succeed.
Release finalization and regeneration share a concurrency group.

### Signing key

- `DATALITH_UPDATER_PRIVATE_KEY` is the repository Actions secret containing
  cargo-packager's base64 private key, with an empty password. Only the signing
  step receives it.
- `scripts/updates/public-key.txt` contains the matching public key. `build.rs`
  embeds it in release builds; the fake server can override it in debug builds.
- The initial owner-only backup is at
  `~/.local/share/datalith-release/updater.key` on the maintainer's PC. Keep a
  second copy in a secure offline backup or password manager accessible only
  to release maintainers. GitHub does not let you retrieve a stored secret.

To restore the Actions secret from the backup:

```sh
gh secret set DATALITH_UPDATER_PRIVATE_KEY --repo mycelium-build/datalith-app < ~/.local/share/datalith-release/updater.key
```

For planned rotation, generate a new keypair with
`cargo packager signer generate --ci --path <secure-backup-path>`. Rotation requires a dedicated transition change: build the app with the new
public key while CI still signs and validates the transition release with the
old key. The current build and validation share one key file, so changing
that file alone is insufficient. After distributing the transition release,
switch CI signing and validation to the new key and restore the shared key
file. Older installations that miss the transition need
a manual install; the current single-key updater does not provide a permanent
rotation bridge. Test and coordinate rotation before changing either key.

If the private key is lost, recover the backup before releasing. Without a
backup, existing installations cannot trust a replacement key and require a
manual install. If compromised, stop update publication, remove affected
releases, replace the key and secret, and distribute a clean installer through
a trusted channel. A compromised old key cannot authenticate its own recovery.

### Removal and recovery

Deleting or editing a release runs **Regenerate update manifest** from the
default branch. It selects the highest remaining published stable release,
verifies its complete signed bundle set, and deploys it. With none remaining,
it publishes version `0.0.0`, which installed releases treat as no update.
Removing a release does not revoke bytes already staged in a running app.

If builds or signature validation fail, leave the release as a draft, repair
or rebuild its assets, and manually run **Release** with the existing tag to
retry finalization. If only Pages publication fails after the release became
public, run **Regenerate update manifest**. The same workflow is the recovery
path for a missed release-edit/deletion event, including edits made using
`GITHUB_TOKEN`, which do not trigger another workflow. Confirm the endpoint
shows the expected version after deployment.

Bundle signatures authenticate update bytes; they are not Apple notarization
or Windows Authenticode. The HTTPS manifest itself is unsigned: tampering can
disrupt discovery, but cannot authorize an unsigned payload.
