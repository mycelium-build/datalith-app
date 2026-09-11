---
status: accepted
---

# Auto-update architecture: crate-driven self-updates over a Pages manifest

Datalith needs self-updating desktop packages (Windows NSIS, macOS app, Linux
AppImage) with a Zed-like experience: automatic discovery and download,
explicit user-triggered apply, and package-manager installs left to their
owners. The design had to choose a discovery mechanism, an updater-eligibility
mechanism, an installation-kind mechanism, a staging model, and an apply
model — while staying small enough to maintain. Datalith Preview (a second
release channel) is wanted as a product, but the updater must ship first; the
design therefore has to let channels arrive later without reworking it.

We use `cargo-packager-updater` as the update engine, driven by a single
manifest endpoint (`updates/stable.json`) published to this repository's
GitHub Pages site by release CI as the final publication step. Updater
eligibility is gated on the existing `DATALITH_RELEASE_TAG` build stamp:
stamped release builds construct the updater, unstamped development builds
never do. Installation kind is detected at runtime — Windows and macOS release
builds are self-managed; a Linux build is self-managed if and only if the
AppImage runtime's contractual `APPIMAGE` variable is set, otherwise it shares
discovery and links to the website download page. Downloaded update bytes are
verified by the crate's minisign signature check and held in memory for the
session only; applying an update rides the normal quit flow and hands off to
the crate's `install` only after that flow has fully completed, with success
or failure observed on next launch. Relaunch is per-platform: on Windows the
NSIS installer relaunches the app (and the crate exits the process from
inside `install`); on macOS and Linux the crate only replaces the files, so
the app spawns a detached reloader before exiting.
Release channels arrive as the final deliverables: a committed `CHANNEL` file
stamped by CI with one `Channel` module deriving product name, application
identifier, data directory, and updater endpoint — at which point the stamp
gate becomes the stable arm of that module and `updates/preview.json` (written
only by RC releases) slots beside the stable manifest.

## Considered options

- **Custom GitHub REST API discovery:** release deletion removes versions
  naturally and needs no extra publication step, but the crate is designed
  around polling fixed manifest endpoints; constructing `Update` values from
  hand-rolled API filtering fights the library, adds unauthenticated
  rate-limit exposure, and reimplements version comparison. A static manifest
  keeps discovery on the crate's documented path; deletion is handled by a
  regeneration hook instead.
- **Hand-rolled async updater modeled on Zed's `auto_update` crate:** one HTTP
  stack and native GPUI async with per-chunk progress, but means owning
  per-OS install logic and minisign verification — more code to maintain than
  wrapping the crate's blocking calls on a dedicated thread. Rejected in favor
  of fitting the existing cargo-packager distribution pipeline.
- **Hosting the manifest on the website repository's Pages site:** keeps
  update URLs on the product domain, but makes a second repository
  trust-critical for update availability and requires cross-repo dispatch
  credentials in release CI. Same-repository Pages publishes with the
  workflow's own `GITHUB_TOKEN`, keeps one trust boundary, and reduces release
  removal to regenerating one deployed file. Users never see the endpoint
  URL, so the product domain buys nothing.
- **Build-stamped installation kind:** precise, but the Linux release job
  packages a single binary into AppImage, `.deb`, `.rpm`, and `.pkg.tar.zst`;
  stamping per format would mean rebuilding the binary per format and
  maintaining CI assertions against drift. The `APPIMAGE` environment variable
  is a contractual runtime signal — the updater crate itself relies on it — so
  one environment read replaces the entire stamping mechanism.
- **Per-release link for externally managed packages:** pointing native
  package users at the exact GitHub release requires a `release_url` manifest
  field and per-version link plumbing. A single constant website download-page
  URL carries the same user value with no manifest extensions.
- **Building channels now (two product identities up front):** Datalith
  Preview does not exist yet — RC releases currently ship with the stable
  identity — so launching it inside the updater integration would add
  identifiers, icon variants, data-directory isolation, a second manifest, and
  coexistence testing to the critical path of the updater itself. The stamped
  `CHANNEL`-file pattern (Zed's, proven across four channels) remains the
  chosen design, delivered as the final deliverables once the updater is live;
  it is additive because the stamp gate already isolates "no updater" builds
  and the manifest layout lives under `updates/`.
- **Two binary crates (lib + `datalith` + `datalith-preview`):** gives each
  channel its own package metadata home, but forces a lib+bins restructuring
  of a single-crate app. The stamped-channel file achieves one in-binary
  source of truth with far less churn when channels land.
- **Persist staged updates across quit:** lets a staged update survive restart
  and remain applicable after release deletion, but requires an on-disk
  staging area, restore-time re-verification, and orphan sweeping.
  Session-scoped staging loses at most one unnoticed background re-download,
  and quitting frees the in-memory bytes for free.
- **In-app apply with failure feedback:** staying alive to report install
  failure gives slightly better errors on Unix-likes but holds the old process
  open during its own replacement and cannot work uniformly on Windows, where
  the NSIS installer is a separate process. Hand-off-and-quit is uniform and
  simple; failure is detected on next launch.
- **Signed manifests:** would add defense-in-depth, but the bundle's minisign
  signature already gates installation, and SemVer monotonicity blocks
  rollback; manifest compromise degrades to "no or bogus update offered,"
  never arbitrary code execution.
- **System-wake restart of in-flight operations:** a dedicated
  `cx.on_system_wake` handler would recover faster after suspend, but a
  suspend-killed connection already fails via the operation timeout and the
  next scheduled check retries. Deferred as a latency optimization, not a
  correctness mechanism.

## Consequences

- This repository's GitHub Pages deployment now participates in update
  availability. Manifest publication must be the last step of release
  finalization, with publication failure failing the finalize job.
- Release deletion does not remove versions from discovery by itself; a
  `release: deleted`/edited workflow regenerates and republishes the manifest
  from the remaining published releases. This hook must exist before the
  first updater-bearing release.
- The first updater-bearing release is a one-way cutover: earlier installs are
  primitive and never self-update. Updates always jump straight to the latest
  version; there is no minimum-version floor and no differential updates.
- The dependency graph carries two reqwest copies (the app's HTTP client and
  the crate's), and the updater's blocking I/O runs on per-operation threads
  (no persistent worker) with progress crossing a channel into the GPUI
  entity — mirroring the existing Vault Catalog threading pattern.
- Externally managed (native Linux package) users are directed to the website
  download page; the updater never writes to package-manager-owned files.
- Windows apply semantics are installer-driven: "Restart to update" quits the
  app and the NSIS installer (passive mode) performs replacement and
  relaunch. The crate ends the process from inside `Update::install`
  (`std::process::exit(0)`), so all cleanup must have completed before the
  call. On macOS and Linux the crate's `install` returns without relaunching;
  the application relaunches itself via a detached reloader spawned before
  exit. The failure model therefore never promises in-app install error
  reporting.
- Development builds are inert by construction: only release CI stamps
  `DATALITH_RELEASE_TAG`, and only stamped builds construct the updater. This
  lets the update module and its UI merge incrementally behind the gate with
  no user-visible effect until release automation makes updates discoverable.
- When channels land, Preview users are not carried onto stable releases; a
  Preview user who wants stable installs Datalith separately, and
  Preview-visible copy must set this expectation.

---

Revised 2026-09-11: simplified from the original version — manifest moved from
the website repository's Pages site to this repository's, build-stamped
installation kind replaced by runtime `APPIMAGE` detection, the externally
managed link reduced to a constant download-page URL, system-wake handling
deferred, and channel construction resequenced from a prerequisite to the
final deliverables. The updater engine, staging model, apply model, and trust
model are unchanged.

Revised 2026-09-11 (second pass, after crate-source verification): pinned the
six manifest platform keys against the crate's code (`macos-*`, not the
`darwin-*` of its documentation example), recorded that the crate's `timeout`
is a total reqwest timeout, made the prerelease skip in the finalize job
explicit, and split the apply model's relaunch per platform — NSIS `/R` on
Windows, an application-spawned detached reloader on macOS and Linux, with
`install` called only after the quit flow completes because the crate exits
the Windows process from inside the call.


Revised 2026-09-11 (D4): publish directly through GitHub Pages Actions rather
than committing to `gh-pages`. `GITHUB_TOKEN` pushes do not trigger branch-based
Pages builds; direct deployment uses the same repository token and allows
release finalization to wait for deployment success. Approved during D4.

Revised during D5: preserve the existing quit and installer behavior on all
platforms, including the macOS administrator prompt. Pre-quit write probes and
additional recovery notifications were rejected; D5 verifies platform behavior
without adding them.

Revised during D6: channel identity now comes from the compiled `CHANNEL` file.
CI selects per-channel packager metadata and verifies it against the built binary.
Preview has separate product, executable, package, and application-data names;
Dev uses fresh `datalith-dev` data without migration. Preview's launcher and in-app
monolith use yellow, with the existing UI theme preserved. The full RC version is
used in package and updater bundle names. Preview publication remains D7.

D6 clarification: Dev uses a green logo and displays the build's Git commit in
About. Preview displays the full RC version; Stable displays the release version.
Vault documents remain shared, while Preview/Dev catalog and search caches use
`.datalith/preview` and `.datalith/dev`. Stable keeps the existing `.datalith`
cache paths. No cache migration is needed.

Revised during D7: every release publication and release edit/deletion regenerates
both channel manifests from GitHub releases and deploys them together. This
user-approved simplification avoids preserving a live-site file across Pages'
whole-site deployments. Each selector is independent: stable tags for Stable,
`vX.Y.Z-rc.N` prereleases for Preview, ordered numerically. An empty channel
publishes the no-update version `0.0.0`. Preview never switches to Stable.
