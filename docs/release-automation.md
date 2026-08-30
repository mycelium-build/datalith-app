# Release automation setup

Configure the following under **Settings → Secrets and variables → Actions**.

Repository secrets:

- `AUTOMATION_APP_CLIENT_ID`: the GitHub App Client ID.
- `AUTOMATION_APP_PRIVATE_KEY`: the GitHub App private key in PEM format.

The GitHub App requires **Contents: read and write** and **Pull requests: read and write** repository permissions. It also requires **Actions: write** permission on the website repository (`mycelium-build/datalith`) so release CI can trigger its deployment workflow.

Successful pushes to `main` create or update a release PR and tag its commit as an RC. Merge the release PR with a merge commit (not squash) to tag that release branch commit as the stable version. Both tag kinds publish Linux, macOS, and Windows artifacts.
