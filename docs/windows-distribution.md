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

## Windows host observations (2026-11-09)

Read-only checks on a Windows 11 25H2 x64 workstation (build 26200, WinGet
1.29.380) with stable 1.1.1 and Preview 1.1.0-rc.2 already installed. The
validation script was **not** run: the host is not disposable and Windows
Sandbox is not enabled. Install, reinstall, upgrade, uninstall, ARM64, WinGet
local manifests and SmartScreen remain untested.

- Release v1.1.1 is published, not draft/prerelease. Both
  `datalith_1.1.1_x64-setup.exe` (`44cd8ee2…8d556b`) and
  `datalith_1.1.1_arm64-setup.exe` (`244b6485…415df9`) match GitHub's SHA-256
  digest. Both installers are 32-bit (x86, `0x14c`) NSIS stubs and `NotSigned`.
- Installed `datalith.exe` is x64 (`0x8664`), GUI subsystem, `NotSigned`.
  `--print-build-identity` piped to `ConvertFrom-Json` returns
  `{"channel":"stable","version":"1.1.1",…}` with exit code 0 (stable over
  repeated runs). Without a pipe the GUI binary yields no captured output, so the
  script must keep the pipeline form.
- HKCU key `Datalith`: DisplayName `Datalith`, Publisher `Mycelium Build`,
  DisplayVersion `1.1.1`, InstallLocation `"%LOCALAPPDATA%\Datalith"` (stored
  **with literal quotes**), UninstallString `"…\uninstall.exe"`, no
  QuietUninstallString, NoModify/NoRepair `1`. This confirms `ProductCode:
  Datalith`, `Scope: user` and the explicit nullsoft silent uninstall switch.
- Preview registers a separate key `Datalith Preview` under
  `%LOCALAPPDATA%\Datalith Preview` (`datalith-preview.exe`), so it does not
  collide with the stable ProductCode. No machine-scope (HKLM) entry exists.

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

Signing budget: **zero** (user decision, 2026-11-09). Paid certificates and
Azure Artifact Signing are excluded. The objective is fewer SmartScreen warnings,
not merely a valid signature; neither signing nor WinGet acceptance guarantees
that an unknown download will stop triggering warnings.

The free candidate for existing NSIS releases is
[SignPath Foundation](https://signpath.org/terms.html), subject to acceptance.
Its conditions include OSI-licensed components without commercial dual licensing,
verifiable builds, MFA, documented signing roles/policy and manual release
approval. The certificate publisher is SignPath Foundation. Datalith's MIT
license alone does not establish eligibility: audit bundled dependencies/assets,
especially the custom Twilight license recorded in LICENSING.md, and project
reputation requirements. No application or enrollment has been submitted.

Self-signing does not establish public SmartScreen trust. Microsoft Store MSIX
signing is free and Store delivery avoids the usual SmartScreen download warning,
but requires a different packaging/distribution path; it does not sign the NSIS
assets downloaded from GitHub. Keep this as a separate option requiring review.
If no free service accepts the project, retain unsigned NSIS distribution and
report that the SmartScreen objective remains unresolved; do not buy a fallback.

Future signing must happen upstream before hashing and updater signing, covering
app, uninstaller and installer; never replace existing published release assets.
No identity/certificate creation or service enrollment without user approval.
See the [Windows handoff](windows-handoff.md) for reproducible validation steps.

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
