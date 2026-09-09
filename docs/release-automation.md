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
