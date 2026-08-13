# License Compliance Plan

Status: approved implementation plan  
Last reviewed: 2026-08-12  
Scope: Datalith source repository, Linux AppImage, macOS DMG, Windows NSIS installer, and GitHub releases

> This document is an engineering compliance plan, not legal advice. The first
> public release produced by this process should receive a focused legal review,
> especially for the GPL distribution boundary and the upstream theme licenses.

## 1. Executive summary

Datalith's original source code remains licensed under the MIT License. Datalith
also builds against third-party components under other licenses. The project has
chosen the conservative distribution posture that release binaries combining
GPL-3.0-or-later components are conveyed under GPL-3.0-or-later, while each
source component remains available under its own license.

Compliance must be reproducible and enforced rather than documented only once.
The repository will therefore:

1. keep the MIT license of original Datalith source explicit;
2. add the complete GPL-3.0 text and a plain-language licensing map;
3. generate and commit third-party notices from `Cargo.lock` with
   `cargo-about`;
4. enforce the accepted license and source policy with `cargo-deny`;
5. maintain non-Cargo asset provenance in `assets/licences.toml`;
6. remove any bundled theme or asset whose redistribution license cannot be
   verified from a primary source;
7. expose all applicable licenses offline from the application's About page;
8. install the license material in every desktop package;
9. publish a complete, vendored corresponding-source archive beside every
   binary release;
10. publish an SPDX JSON SBOM and `SHA256SUMS` for the release; and
11. fail CI and release publication with a specific error whenever any required
    compliance artifact is missing, stale, inconsistent, or unverifiable.

The implementation reuses established open-source tools and Zed's overall
license-generation pattern. It does not implement a custom Rust dependency
license scanner.

## 2. Confirmed licensing policy

### 2.1 Licensing layers

| Scope | Policy |
|---|---|
| Original Datalith source files and original assets | MIT |
| Datalith Cargo package metadata | `license = "MIT"` |
| GPUI and Zed crates marked Apache-2.0 | Apache-2.0 |
| `zlog`, `ztracing`, and `ztracing_macro` | GPL-3.0-or-later |
| Distributed combined Datalith executable | Conveyed under GPL-3.0-or-later conditions |
| Pixeloid font files | SIL Open Font License 1.1 |
| Other Rust dependencies, themes, and icons | Their respective licenses |

The license of an upstream component must be read from the component's own
manifest or license material. The fact that multiple components live in the
same upstream repository does not give the repository a single license. For the
Zed revision currently locked by Datalith, the `gpui` crate is Apache-2.0 while
`ztracing` is GPL-3.0-or-later.

### 2.2 What remains MIT

The following must remain true:

- `Cargo.toml` declares `license = "MIT"` for Datalith;
- the root `LICENSE` contains the unmodified MIT text for Datalith's original
  source;
- no `MIT OR GPL-3.0-or-later` expression is added unless the copyright holder
  later makes an explicit dual-licensing decision; and
- downstream users can identify which source is original Datalith code and
  continue to exercise the MIT grant over that code.

### 2.3 What GPL distribution means here

The release process will not rely on a `THIRD-PARTY-NOTICES.md` file alone to
satisfy GPL obligations. Each released binary will be accompanied by:

- the complete GPL-3.0 license;
- an unambiguous notice describing the combined binary's distribution terms;
- the machine-readable Corresponding Source for the exact released version;
- the scripts and configuration needed to build and package that version;
- clear access to the source from the same GitHub Release as the binaries; and
- appropriate legal notices in the interactive application.

No installer EULA or download condition may prohibit copying, modification,
reverse engineering, or redistribution in a way that conflicts with GPL-3.0.

## 3. Current repository findings

The implementation starts from these observed facts:

- Datalith is currently declared MIT in `Cargo.toml` and the root `LICENSE`.
- Releases produce an AppImage, an Apple Silicon DMG, and an x86-64 NSIS
  installer.
- `zlog`, `ztracing`, and `ztracing_macro` appear in the locked Cargo graph as
  GPL-3.0-or-later packages from the Zed Git revision.
- `gpui`, `gpui_platform`, `gpui_linux`, and `sum_tree` are marked Apache-2.0.
- two Pixeloid TTF files are embedded directly into the executable; their OFL
  text exists in `assets/fonts/Pixeloid/LICENSE.txt` but is not currently
  exposed by the application.
- twenty-one third-party themes are embedded into the binary. Their JSON files
  retain author and URL fields, but the repository does not yet retain all
  verified upstream license texts.
- `gpui-component-assets` contains Lucide icons; Lucide is ISC licensed and
  includes specified Feather-derived icons under MIT.
- the About page currently shows version and product information but no license
  or source information.
- no generated dependency-notice file, corresponding-source bundle, or SBOM is
  currently published.

These findings define the remediation baseline. The new blocking CI checks must
be enabled in the same change set that makes the baseline pass; there will be no
warning-only transition on the default branch.

## 4. Target repository layout

The implementation should produce this structure:

```text
LICENSE                         # MIT text for original Datalith source
LICENSE-GPL-3.0                 # complete, unmodified GPLv3 text
LICENSING.md                    # scope map and binary distribution explanation
THIRD-PARTY-NOTICES.md          # generated, committed, human-readable notices

assets/
├── licences.toml               # canonical non-Cargo asset inventory
├── licenses/
│   ├── generated.md            # generated application-facing notice content
│   └── texts/                  # exact non-Cargo license texts where needed
├── fonts/
├── icons/
└── ...

scripts/
└── licenses/
    ├── about.toml              # cargo-about policy
    ├── about.hbs               # notice template
    ├── deny.toml               # cargo-deny policy, unless root deny.toml is preferred
    ├── generate.sh             # deterministic generation entry point
    ├── check.sh                # local/CI verification entry point
    ├── check_assets.py         # small repository-specific manifest validator
    ├── package_source.sh       # corresponding-source archive orchestration
    └── verify_package.sh       # extracted package/license verifier

docs/
└── license-compliance-plan.md  # this document
```

Only the asset-manifest validation and release orchestration are project-specific
code. Rust license discovery, license-expression resolution, vendoring, and SBOM
generation must be delegated to existing tools.

## 5. Root licensing documents

### 5.1 `LICENSE`

Keep the existing, standard MIT license text and copyright statement. This is
the license Cargo and source-hosting tools should report for original Datalith
source.

### 5.2 `LICENSE-GPL-3.0`

Add an unmodified copy of GPL version 3. The file name must make the version
explicit and must be referenced from `LICENSING.md`, the application, packages,
and release notes.

### 5.3 `LICENSING.md`

This hand-maintained document is the authoritative scope explanation. It must:

- state that original Datalith source is MIT licensed;
- state that release binaries combine components under GPL-3.0-or-later and are
  conveyed under GPL-3.0-or-later conditions;
- explain that third-party components retain their respective licenses;
- link to `LICENSE`, `LICENSE-GPL-3.0`, and `THIRD-PARTY-NOTICES.md`;
- identify the corresponding-source artifact naming convention; and
- explain how a recipient can find the exact source for an installed version.

### 5.4 `README.md`

Add a short `Licensing` section that summarizes, but does not duplicate, the
scope map and links to `LICENSING.md`.

## 6. Rust dependency compliance

### 6.1 Notice generation: `cargo-about`

Use `cargo-about`, following Zed's pattern, to resolve SPDX expressions and
produce a single grouped Markdown document. Pin the tool version in CI and in a
small tool-version file or constants section in `generate.sh`. The initial
implementation should evaluate and pin the then-current tested 0.9.x release;
version changes require an explicit dependency-update PR and regenerated diff.

`scripts/licenses/about.toml` must:

- list all accepted licenses in preference order;
- include the three production targets;
- ignore dependencies used exclusively for tests/benchmarks;
- include build dependencies because they may contribute generated or linked
  material and because the project chose the conservative audit posture;
- include transitive dependencies;
- avoid silently dropping unpublished Git dependencies;
- enable maintained workarounds such as those for `ring`, `rustls`, and relevant
  Apple framework crates; and
- contain checksum-pinned clarifications for packages whose manifests do not
  provide sufficient machine-readable metadata.

An initial policy skeleton is:

```toml
accepted = [
    "GPL-3.0-or-later",
    "Apache-2.0",
    "MIT",
    "MIT-0",
    "Apache-2.0 WITH LLVM-exception",
    "MPL-2.0",
    "BSD-3-Clause",
    "BSD-2-Clause",
    "ISC",
    "CC0-1.0",
    "0BSD",
    "NCSA",
    "Unicode-3.0",
    "Zlib",
    "BSL-1.0",
    "bzip2-1.0.6",
]

targets = [
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
]

ignore-dev-dependencies = true
ignore-build-dependencies = false
ignore-transitive-dependencies = false
private = { ignore = false }

workarounds = ["ring", "rustls", "cocoa"]
```

The accepted list is not a blanket approval mechanism. Adding a new identifier
requires a review of its obligations and compatibility with the distribution
policy. The PR adding it must explain why it is acceptable.

Known clarification work includes:

- `gpui_shared_string`: establish Apache-2.0 from the exact Zed revision and
  checksum the relevant upstream license file;
- `gpui_util`: the same treatment;
- `cfg_block`: confirm its packaged Apache-2.0 license file; and
- any Git dependency for which `cargo-about` cannot follow a license symlink.

Do not invent a license expression to make generation pass. Every clarification
must cite an upstream file from the exact locked revision and pin its checksum.

### 6.2 Policy enforcement: `cargo-deny`

Use `cargo-deny` as an independent policy check, initially pinned to the tested
0.18.x release. It should check:

- license expressions;
- advisories where appropriate;
- yanked packages;
- approved registries and Git sources;
- exact Git pinning policy; and
- unexpected duplicate or wildcard dependencies according to an explicit
  warning/error policy.

All Git dependency origins must be allowlisted narrowly. Direct Git dependencies
in `Cargo.toml` should use `rev` rather than relying only on `Cargo.lock` when the
upstream API permits it. At minimum, Zed and `gpui-component` must be pinned in
the manifest so a future `cargo update` cannot silently move the compliance
boundary.

`cargo-deny` and `cargo-about` have separate purposes:

- `cargo-deny` answers whether the dependency graph conforms to project policy;
- `cargo-about` generates the license material distributed to recipients.

Both must pass.

### 6.3 Production scope

Distributed notices cover the union of the dependency graphs for:

- `x86_64-unknown-linux-gnu`;
- `aarch64-apple-darwin`; and
- `x86_64-pc-windows-msvc`.

Dependencies used only by tests and benchmarks are excluded from the user-facing
notice file, but remain visible to development/security audits. Build
dependencies are included in compliance checking. If the final notice generator
cannot distinguish a build-only tool that leaves no material in the output, it
is acceptable to over-disclose it; under-disclosure is not acceptable.

## 7. Non-Cargo asset compliance

### 7.1 Canonical manifest

`assets/licences.toml` is the single source of truth for bundled fonts, icons,
themes, and other third-party assets. Markdown notices are generated from it and
must not become an independently edited inventory.

Each entry should contain at least:

```toml
[[asset]]
id = "pixeloid"
kind = "font"
paths = [
    "assets/fonts/Pixeloid/PixeloidSans.ttf",
    "assets/fonts/Pixeloid/PixeloidSans-Bold.ttf",
]
name = "Pixeloid Sans"
author = "GGBotNet"
copyright = "Copyright 2020-2025 GGBotNet"
source = "https://ggbot.net/fonts/"
revision = "upstream release or immutable archive identifier"
license = "OFL-1.1"
license_file = "assets/fonts/Pixeloid/LICENSE.txt"
reserved_names = ["Pixeloid"]
distribution = "embedded"
```

For an asset copied through an intermediary project, record both sources:

```toml
upstream_source = "..."
adaptation_source = "https://github.com/longbridge/gpui-component"
adaptation_revision = "6ef264a40a4646c635010afaabfd0723b758f71e"
```

`revision` must be immutable where the upstream provides tags, commits, or
release archives. A moving homepage alone is insufficient evidence.

### 7.2 Coverage validator

`check_assets.py` may be a small repository-specific script because no generic
Cargo tool can know which Datalith asset directories are embedded. It must:

1. enumerate all tracked files under configured embedded-asset paths;
2. expand every manifest path/glob;
3. fail if an embedded file matches no manifest entry;
4. fail if a manifest entry matches no file;
5. fail on overlapping entries unless explicitly allowed;
6. verify that every referenced license file exists and is non-empty;
7. validate SPDX expressions;
8. require author, source, revision, and copyright fields for third-party
   assets;
9. reject a theme marked `third-party` without a verified license; and
10. emit deterministic Markdown in a stable order.

Use a maintained TOML parser and SPDX-expression parser rather than implementing
either grammar. The script must print the exact asset ID and missing or invalid
field on failure.

### 7.3 Themes remediation

Audit all twenty-one third-party themes individually against primary sources.
For each theme:

1. identify the original project and immutable revision;
2. retrieve the original license and copyright statement;
3. identify the `gpui-component` adaptation revision;
4. record both layers in `assets/licences.toml`;
5. retain the required license text; and
6. verify that the license permits redistribution and adaptation.

If an explicit redistribution license cannot be demonstrated, remove the theme
JSON from the repository, remove its `include_str!` registration, and adjust any
tests or defaults referring to it. Author and URL metadata alone are not a
license.

### 7.4 Fonts and icons

The first implementation must at least include:

- Pixeloid copyright, OFL-1.1 text, and reserved font name;
- `gpui-component-assets` under Apache-2.0;
- Lucide under ISC;
- Feather's MIT notice for the applicable derived icons; and
- a statement identifying original Datalith pixel-art icons and application
  artwork as MIT-covered first-party assets.

## 8. Generated notices

`scripts/licenses/generate.sh` is the only supported regeneration entry point.
It should:

1. verify the expected pinned tool versions;
2. validate `assets/licences.toml`;
3. render the non-Cargo asset sections;
4. run `cargo about generate --locked --fail` for the production target union;
5. combine the licensing overview, fonts, themes, icons, and Rust dependency
   sections;
6. write `THIRD-PARTY-NOTICES.md` for repository and package use;
7. write `assets/licenses/generated.md` for embedding in the application; and
8. produce byte-for-byte deterministic output.

Generated files are committed. CI regenerates them in a clean checkout and
fails if `git diff --exit-code` detects a difference.

The generated document must include component names, versions or revisions,
source URLs, selected license expression, copyright notices, and full required
license texts. Grouping many crates under one identical license text is
preferred over repeating the same text hundreds of times.

## 9. Application changes

### 9.1 About page

Extend `Settings -> About` to show:

- product name and exact version;
- `Copyright (c) 2026 mycelium-build`;
- `Original Datalith source code: MIT License`;
- a clear statement that the distributed application includes
  GPL-3.0-or-later components and is conveyed under GPL-3.0-or-later;
- the GPL no-warranty notice in concise form;
- `View GNU GPL`;
- `View third-party licenses`; and
- `View corresponding source`.

The source URL must be version-specific, for example the GitHub Release URL for
`v${CARGO_PKG_VERSION}`, and must not point only to `main`. Pre-release version
mapping must be defined and tested.

### 9.2 Offline license viewer

Compile the following content into the application with `include_str!` or the
existing asset system:

- `LICENSE`;
- `LICENSE-GPL-3.0`;
- `LICENSING.md`; and
- `assets/licenses/generated.md`.

Expose it through a scrollable Markdown view. Add a Help-menu action analogous
to Zed's `View Dependency Licenses`, as well as the About-page entry. The user
must be able to inspect the licenses without network access.

### 9.3 Tests

Add tests proving that:

- About contains the MIT/GPL scope statement;
- each legal-document action resolves an embedded non-empty asset;
- the displayed application version is the version used to construct the
  release-source URL; and
- the Help menu contains the dependency-license action.

## 10. Desktop packaging

Update package configuration so every installed artifact contains:

- `LICENSE`;
- `LICENSE-GPL-3.0`;
- `LICENSING.md`; and
- `THIRD-PARTY-NOTICES.md`.

Preferred installed locations are:

| Format | Location |
|---|---|
| AppImage | `usr/share/doc/datalith/` |
| macOS app bundle | `Datalith.app/Contents/Resources/Licenses/` |
| NSIS installation | `Licenses/` below the Datalith installation directory |

The packager's `license-file` for the distributed application must not present
the MIT file as if it were the sole license governing the combined binary. Its
exact value and installer presentation must be verified against the pinned
`cargo-packager` behavior. Prefer the explanatory `LICENSING.md` if the field is
display-only and explicitly add all full texts as package resources.

`verify_package.sh` must inspect an extracted package and emit a separate,
actionable error for each missing file. Extraction is platform/format specific:

- extract the AppImage in Linux CI;
- inspect the mounted or unpacked DMG and `.app` resources in macOS CI; and
- unpack or install the NSIS artifact in an isolated Windows CI directory.

Do not mark a platform artifact successful until its own inspection passes.

## 11. Corresponding Source archive

### 11.1 Existing tools, not a custom vendor system

Use Cargo's built-in `cargo vendor --locked` to collect crates.io and Git source
dependencies. The project-specific script only assembles and verifies the
archive.

### 11.2 Required contents

For version `X.Y.Z`, publish:

```text
datalith-X.Y.Z-corresponding-source.tar.zst
```

The archive must contain:

- the exact Datalith source tree for the release commit;
- `Cargo.toml`, `Cargo.lock`, and `rust-toolchain.toml`;
- source assets and legal documents;
- build, packaging, and license-generation scripts;
- the complete Cargo `vendor/` directory;
- generated `.cargo/config.toml` pointing Cargo at `vendor/`;
- a manifest recording the release commit, toolchain, target triples, and
  packaging-tool versions;
- instructions for offline builds on each supported platform; and
- any non-system source or interface definitions needed to generate, install,
  run, and modify the executable.

Do not include `.git`, `target`, local configuration, credentials, developer
caches, or unrelated files.

### 11.3 Deterministic creation

`package_source.sh` must:

1. require a clean checkout at the release tag;
2. verify that Cargo package version and tag agree;
3. create a temporary staging directory;
4. copy tracked source using a Git archive or explicit tracked-file list;
5. run `cargo vendor --locked` into the staging directory;
6. write the offline Cargo source replacement configuration;
7. generate a source manifest;
8. normalize archive paths, ordering, ownership, and timestamps where tooling
   supports it; and
9. create the `.tar.zst` archive.

### 11.4 Reconstruction test

Each platform job must download or receive the exact staged source bundle,
extract it, disable network access to Cargo where practical, and run:

```text
cargo build --release --locked --offline --target <matrix target>
```

The same source archive must rebuild all three targets in their native CI
environments. A Linux-only successful build is insufficient.

The source archive must remain attached to the GitHub Release for as long as any
corresponding binary remains downloadable. Release automation must never replace
a released source archive with different bytes under the same name.

## 12. SBOM and release integrity

### 12.1 SPDX SBOM

Use Anchore Syft, installed through its maintained GitHub Action pinned by full
commit SHA, to produce SPDX JSON. Syft is selected because it is an established
open-source generator supporting filesystems, archives, and SPDX JSON. It does
not replace `cargo-about` or the asset manifest.

Publish:

```text
datalith-X.Y.Z.spdx.json
```

Generate the release SBOM from the staged corresponding-source/package context
and validate that it is non-empty, valid JSON, identifies Datalith's version,
and lists the expected GPL, Apache, MPL, font, and icon components. If Syft does
not discover manually embedded assets, enrich or merge those records from
`assets/licences.toml` using an SPDX-aware tool or a narrowly tested script; do
not silently omit them.

### 12.2 Checksums

After every release artifact is finalized, generate `SHA256SUMS` containing:

- AppImage;
- DMG;
- NSIS installer;
- corresponding-source archive;
- SPDX JSON SBOM;
- `THIRD-PARTY-NOTICES.md`;
- `LICENSE`; and
- `LICENSE-GPL-3.0`.

Generate checksums once, after fan-in of the matrix artifacts, and upload the
checksum file last. Fail on duplicate names, missing expected artifacts, or an
artifact changing after checksum generation.

Cryptographic signing is a recommended future enhancement but is not a blocker
for the first implementation of this plan.

## 13. CI design

### 13.1 Pull-request compliance job

Add a blocking `license-compliance` job that runs when relevant files change and
also runs unconditionally before release. Relevant paths include:

```text
Cargo.toml
Cargo.lock
assets/**
src/ui/themes/**
src/app/assets.rs
src/app/fonts.rs
scripts/licenses/**
LICENSE*
LICENSING.md
THIRD-PARTY-NOTICES.md
.github/workflows/**
```

The job runs:

1. install pinned `cargo-about` and `cargo-deny` using a maintained pinned
   installer action where possible;
2. `cargo deny check`;
3. `scripts/licenses/check.sh`;
4. regenerate notices;
5. ensure generated files have no diff; and
6. run application tests covering legal-document integration.

The job is a required branch-protection check.

### 13.2 Release workflow

Restructure the release workflow into explicit gates:

```text
validate tag/version
        |
license-compliance
        |
build corresponding-source bundle
        |
offline rebuild/package matrix
   /         |          \
Linux      macOS      Windows
   \         |          /
inspect packages and collect artifacts
        |
generate/validate SPDX SBOM
        |
generate SHA256SUMS
        |
create or update draft GitHub Release
        |
upload the complete immutable artifact set
```

The current workflow creates the draft before all compliance work has passed.
It may continue to use a draft as a staging object, but the draft must never be
published unless every gate succeeds and every required artifact is present.

### 13.3 Clear failure messages

Scripts must return non-zero and use stable, searchable error prefixes. Examples:

```text
LICENSE-E001 unknown Cargo license: package=<name> version=<version> expression=<expression>
LICENSE-E002 missing license metadata: package=<name> source=<source>
LICENSE-E101 uncovered embedded asset: path=<path>
LICENSE-E102 unverified theme license: id=<id> source=<url>
LICENSE-E103 missing asset license file: id=<id> path=<path>
LICENSE-E201 generated notices are stale: run scripts/licenses/generate.sh
LICENSE-E301 package legal file missing: format=<format> file=<file>
LICENSE-E401 offline source rebuild failed: target=<target>
LICENSE-E402 release/source version mismatch: tag=<tag> cargo=<version>
LICENSE-E501 missing release artifact: name=<expected-name>
LICENSE-E502 checksum mismatch: file=<file>
LICENSE-E601 invalid SPDX SBOM: reason=<reason>
```

Each CI step should upload diagnostic logs on failure without uploading a
partially approved release artifact set.

## 14. Release artifact contract

A release is compliant only if its GitHub Release contains all of:

```text
Datalith-X.Y.Z-<platform package>
datalith-X.Y.Z-corresponding-source.tar.zst
datalith-X.Y.Z.spdx.json
THIRD-PARTY-NOTICES.md
LICENSE
LICENSE-GPL-3.0
SHA256SUMS
```

The application version, tag, package filenames, source manifest, SBOM, and
release URL must agree. Release candidates follow the same contract and use
their full pre-release version.

## 15. Implementation sequence

The work should be delivered atomically to the default branch, but it can be
developed as dependent commits or stacked pull requests.

### Phase 1: policy and inventory

- Add `LICENSE-GPL-3.0` and `LICENSING.md`.
- Add the README licensing summary.
- Create `assets/licences.toml` and its schema/validation rules.
- Inventory Pixeloid, local artwork, gpui-component assets, Lucide, and Feather.
- Audit every theme against primary sources.
- Remove themes that cannot be licensed for redistribution.

Acceptance criteria:

- every embedded non-Cargo asset is covered exactly once;
- every third-party asset has immutable provenance and a license text;
- no theme remains based only on an author/URL field.

### Phase 2: automated Rust notices and policy

- Add pinned `cargo-about` configuration and template.
- Add checksum-backed upstream clarifications.
- Add `cargo-deny` configuration and narrow Git source allowlist.
- Pin direct Git dependencies by revision where possible.
- Generate and commit the first notices.

Acceptance criteria:

- all three target graphs generate without missing-license warnings;
- GPL packages are explicitly present in the output;
- dev-only packages are absent from distributed notices;
- a lockfile change without regeneration fails locally and in CI.

### Phase 3: application and package integration

- Extend About.
- Add offline license viewer and Help action.
- Add version-specific corresponding-source link.
- Include all four legal documents in each package.
- Add UI and package-inspection tests.

Acceptance criteria:

- licenses are readable without network access;
- each extracted platform package contains the required files;
- the source link resolves to the exact release version pattern.

### Phase 4: corresponding source, SBOM, and release gates

- Implement the `cargo vendor --locked` source-bundle wrapper.
- Verify offline native builds from the same bundle on all platforms.
- Add pinned Syft SPDX generation and validation.
- Add final artifact fan-in and `SHA256SUMS` generation.
- Make compliance checks required before release publication.

Acceptance criteria:

- the source archive rebuilds all three release binaries offline;
- the SBOM covers Rust and manually embedded assets;
- all release files appear in `SHA256SUMS` and verify;
- deletion or omission of any legal/source artifact makes the release fail with
  a named error code.

### Phase 5: first compliant release review

- Run the process for a release candidate.
- Retain complete CI logs and the generated artifact set.
- Perform a focused legal review of `LICENSING.md`, the About wording, theme
  determinations, GPL Corresponding Source contents, and installer behavior.
- Fix review findings before publishing the stable release.

## 16. Definition of done

The repository is considered conformant to this engineering plan when all of
the following are true:

- [ ] Original Datalith source remains clearly MIT licensed.
- [ ] GPL-3.0 text and binary distribution scope are explicit.
- [ ] `THIRD-PARTY-NOTICES.md` is generated, committed, and current.
- [ ] `cargo-about` includes the full production target union.
- [ ] `cargo-deny` rejects unapproved licenses and sources.
- [ ] GPL packages, including `zlog` and `ztracing`, are visible in notices.
- [ ] Every embedded asset is represented in `assets/licences.toml`.
- [ ] Every bundled theme has a verified license or has been removed.
- [ ] Pixeloid's OFL and reserved-name notice are included.
- [ ] Lucide/Feather and gpui-component asset notices are included.
- [ ] About exposes MIT/GPL scope, no warranty, licenses, and exact source link.
- [ ] Legal documents are available offline in the application.
- [ ] AppImage, DMG, and NSIS packages contain all required legal files.
- [ ] A single corresponding-source archive rebuilds all target releases
      offline.
- [ ] An SPDX JSON SBOM is published and validated.
- [ ] `SHA256SUMS` binds the complete release artifact set.
- [ ] Pull requests cannot merge when compliance generation or validation fails.
- [ ] Releases cannot publish when any compliance gate fails.
- [ ] The first compliant public release has completed focused legal review.

## 17. Maintenance rules

After implementation:

- every dependency update PR must include the generated notice diff;
- every new asset must update `assets/licences.toml` in the same PR;
- tool updates must remain version-pinned and receive their own generated diff;
- license exceptions require an upstream primary-source citation and checksum;
- GitHub Actions must remain pinned by full commit SHA;
- released source bundles must not be deleted while their binaries remain
  available;
- a change to supported platforms must update Cargo target policy, package
  verification, source reconstruction, and SBOM generation together; and
- this document must be reviewed whenever licensing policy, packaging formats,
  release hosting, or major dependency sources change.

## 18. References and precedents

Implementation should consult these primary references:

- Zed's per-crate license declarations, root GPL/Apache texts,
  `script/generate-licenses`, `script/check-licenses`, `assets/themes/LICENSES`,
  `assets/icons/LICENSES`, and in-application `OpenLicenses` action at the exact
  Zed revision locked in `Cargo.lock`;
- the GNU GPL version 3, particularly Corresponding Source and conveying
  non-source forms;
- Mozilla's MPL 2.0 FAQ for executable-form/source-availability obligations;
- the SIL Open Font License 1.1 and its official FAQ;
- cargo-about's configuration and output-template documentation;
- cargo-deny's license and source-policy documentation;
- Cargo's `vendor` command documentation; and
- Anchore Syft's SPDX JSON and directory/archive scanning documentation.

Where this plan and a license text differ, the license text controls. Where an
upstream project's metadata and copied license files conflict, stop the release
and resolve the discrepancy from primary upstream evidence rather than choosing
the more convenient interpretation.
