# Windows distribution readiness

Audit: 2026-11-09. Distribution is not yet approved for submission to Microsoft.

## Published packaging

Stable v1.1.1 publishes x64 and ARM64 NSIS installers. cargo-packager 0.11.8's
bundled NSIS template uses current-user installation under `%LOCALAPPDATA%\Datalith`,
HKCU Add/Remove Programs key `Datalith` (64-bit registry view), publisher
`Mycelium Build`, and upstream `DisplayVersion` without the tag's `v` prefix.
The template supports `/S` (silent), `/P` (passive), and uninstallation with `/S`.
It does not write QuietUninstallString; a WinGet manifest should specify nullsoft
and an explicit silent uninstall switch. No custom template is justified yet.
The updater's detached `.sig` authenticates updater payloads, not Windows
Authenticode identity. It does not establish SmartScreen publisher reputation.

The manual `Validate published Windows installers` workflow downloads the exact
release assets, checks GitHub's SHA-256 digest, and tests install, same-version
reinstall, installed PE architecture/build identity, registry version/publisher,
and uninstall on native x64 and ARM64 runners. This workflow has not been run.
Linux Wine/QEMU availability is not evidence of Windows compatibility. No linked
Windows machine could be enumerated with the tools available in this session.

Before submission, also test an older stable release upgraded to the candidate,
WinGet local-manifest installation/list/upgrade/uninstall, app-running behavior,
data preservation, interactive installation, and startup on supported Windows.
Same-version reinstall is not proof of cross-version upgrade. Capture runner logs
and actual registry metadata before finalizing manifests.

## Requirements and signing

Official sources checked on the audit date:

- [WinGet policies](https://github.com/microsoft/winget-pkgs/blob/master/doc/Policies.md)
  require installation without interaction, official download URLs and security
  validation. Acceptance remains subject to Microsoft moderation and scans.
- [Manifest documentation](https://learn.microsoft.com/windows/package-manager/package/manifest)
  supports `nullsoft` and its standard silent switches. The policies reviewed do
  not impose a universal Authenticode requirement on EXE/NSIS or MSI installers.
- [MSIX signing](https://learn.microsoft.com/windows/msix/package/signing-package-overview)
  is a separate format requirement: installable MSIX needs a trusted signature.
- [Signing options](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options)
  distinguish Store MSIX signing from publisher-signed Store EXE/MSI submissions.
  Store EXE/MSI requirements must not be extrapolated to winget-pkgs.
- [SmartScreen reputation](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation)
  is separate from manifest acceptance. Signing does not guarantee immediate
  reputation; even EV no longer grants automatic reputation.

No paid certificate is established as a prerequisite for this NSIS WinGet plan.
For signing, first establish the publisher's legal identity and country. Microsoft
lists Azure Artifact Signing at approximately USD 9.99/month, available to
organizations in the EU/UK/US/Canada and individuals in US/Canada. A French
individual should not be assumed eligible. If eligible, this is a practical CI
option; otherwise obtain an appropriate CA quote including token/cloud signing
costs. Microsoft estimates OV at USD 150–300/year; this is not a vendor quote.
Do not buy or create identities/certificates without approval. Future signing
must happen upstream before hashing and updater signing, covering app,
uninstaller and installer; never replace existing published release assets.

## Separate distribution repository

Proposed layout: `mycelium-build/windows-packages` for multi-application catalog,
manifest generation and automation; optional `mycelium-build/winget-pkgs` fork
for reviewed upstream contribution branches. Neither has been created yet.
The application repository owns packaging and Windows validation only.

The distribution pipeline should consume stable releases, download existing
assets, verify and calculate SHA-256, preserve upstream versions and produce the
three manifest types under `manifests/m/MyceliumBuild/Datalith/<version>/`.
Proposed ID: `MyceliumBuild.Datalith`, installer type `nullsoft`, scope `user`,
x64/arm64 URLs, `ProductCode: Datalith`, publisher and version matching the registry.
Confirm this metadata on Windows before submission. Never rebuild or re-sign in
the distribution repository. Never include release candidates.

A scheduled reconciliation can recover missed release notifications. Updates
should open internal review PRs using a repository-scoped GitHub App token from
AUTOMATION_APP_CLIENT_ID/PRIVATE_KEY and author
`datalith-bot[bot] <311197260+datalith-bot[bot]@users.noreply.github.com>`.
Existing secrets are not automatically available in a new repository; App
installation access and secret provisioning must be arranged without displaying
private keys. Keep upstream submission disabled until user review; repository
protections must remain unchanged. No automation or manifests are implemented
yet in the proposed distribution repository.
