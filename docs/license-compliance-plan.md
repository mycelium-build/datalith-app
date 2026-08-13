# License compliance release checklist

Status: best-effort controls implemented; first public release still requires
the maintainer checks below

Last reviewed: 2026-08-13

Scope: source, AppImage, DEB, RPM, Arch package, DMG, NSIS installer, and GitHub
release assets

This is a best-effort engineering checklist, not legal advice. The project does
not require paid legal review to release. License texts control where they
differ from this document, and uncertainty must be disclosed or resolved by
removing the affected component.

## Policy

- Original Datalith source and artwork remain MIT licensed.
- Each third-party component retains its own license.
- Because the executable links `zlog`, `ztracing`, and `ztracing_macro`, which
  declare `GPL-3.0-or-later` at the locked Zed revision, release binaries use
  the project's conservative GPL-3.0-or-later distribution posture.
- Every binary release must provide the exact Corresponding Source from which
  it was built. Source must stay available for as long as the binary is
  distributed.
- An asset or dependency with unclear redistribution terms is removed or
  blocks release; it is never silently allowlisted.

The policy and applicable texts are in `LICENSING.md`, `LICENSE`,
`LICENSE-GPL-3.0`, and `THIRD-PARTY-NOTICES.md`.

## Controls already implemented

| Surface | Enforcement |
|---|---|
| Rust dependencies | Pinned `cargo-deny` policy and `cargo-about` notice generation |
| Git dependencies | Exact commits locked in `Cargo.lock`; builds use `--locked`; Git sources allowlisted |
| Bundled assets | `assets/licences.toml`, complete-coverage validation, and immutable theme-license evidence |
| Notices | One generated `THIRD-PARTY-NOTICES.md`, embedded by the app and shipped in packages |
| Application | Offline license viewer and version-specific source-release link |
| Packages | Extract each format and compare its legal files byte-for-byte with the release source |
| Corresponding Source | Exact tagged tree plus `cargo vendor --locked` and offline Cargo configuration |
| SBOM | Pinned Syft SPDX JSON scan with required core dependency checks |
| Release integrity | Exact artifact-set validation followed by `SHA256SUMS` |

## Final cleanup before the first public release

These are maintainer release checks, in order:

1. **Focused maintainer review.** Recheck the documented GPL distribution
   posture, MIT/GPL wording, TextMate custom grant, Pixeloid OFL handling, and
   retained theme provenance against available primary sources. Theme-license
   evidence is pinned to immutable upstream commits. Pixeloid's publisher does
   not expose a stable versioned archive, so the manifest records version 1.0
   from the embedded metadata and SHA-256 identifiers for the exact retained
   files. Document any further limitation rather than claiming certainty that
   the evidence does not support.
2. **Corresponding Source reconstruction.** Build the source archive for a
   release-candidate tag, unpack it on Linux, macOS, and Windows, disable
   network access, and run `cargo build --release --locked --offline` for each
   supported target. Record the commands and successful workflow run.
3. **End-to-end release-candidate rehearsal.** Let the tag workflow create its
   draft release. Install and launch every package, open the license viewer,
   follow the source link, validate the SBOM, and run `sha256sum -c
   SHA256SUMS`. Confirm there are no unchecksummed or duplicate assets.
4. **Require the gates in branch protection.** Configure `main` to require both
   the normal CI job and `license-compliance`; confirm the rule cannot be
   bypassed by the release automation account.
5. **Resolve review findings, then publish manually.** The workflow deliberately
   leaves the release as a draft. Do not publish it until steps 1-3 pass.

Security advisories ignored in `deny.toml` are outside the license inventory,
but they still require an explicit release-risk decision before publication.

## How future licensing is verified

Every pull request and push to `main` runs `scripts/licenses/check.sh`. It:

1. rejects unapproved dependency licenses and registries/Git sources;
2. checks bans, yanked packages, advisories, and documented advisory ignores;
3. requires every tracked embedded asset to have exactly one manifest entry;
4. rejects unapproved asset-license expressions and incomplete third-party
   provenance; and
5. regenerates notices and fails if the committed notice is stale.

Every release tag repeats that gate before any build can complete. The release
workflow then:

1. builds packages with the exact release tag embedded in the source link;
2. extracts each package and verifies the four legal documents byte-for-byte;
3. publishes vendored Corresponding Source for the exact tag;
4. creates a versioned SPDX SBOM and checks the GPL and UI dependency records;
5. rejects a missing, duplicate, or unexpected release artifact; and
6. checksums the complete accepted artifact set.

Branch protection must require both the normal CI job and `license-compliance`.
GitHub Actions remain pinned by commit SHA; licensing tools remain pinned by
version. A dependency, tool, target, package format, or asset change must update
its policy/inventory and generated notice in the same pull request.

## Definition of releasable

- [x] Original source scope and binary distribution posture are documented.
- [x] Dependency and asset drift fail CI.
- [x] Notices are generated once and used by the app and every package.
- [x] Locked Git revisions and compliance tool versions are reproducible.
- [x] Package legal files and release artifact membership are verified.
- [x] Versioned Corresponding Source, SBOM, and checksums are automated.
- [ ] Focused maintainer review is complete and limitations are documented.
- [x] Retained-theme evidence is immutable where practical.
- [ ] Offline source reconstruction passes on all supported targets.
- [ ] A complete release-candidate rehearsal passes.
- [ ] Branch protection requires both CI gates.

Where metadata conflicts with upstream license material, stop the release and
resolve the conflict from primary evidence.
