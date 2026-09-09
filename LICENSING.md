# Datalith licensing

This document is the authoritative explanation of how Datalith and its
components are licensed. It is a plain-language scope map, not the license text
itself. The authoritative license texts are the files named below.

## What is licensed how

| Scope | License |
|---|---|
| Original Datalith source code | MIT |
| Original Datalith artwork (pixel-art icons, application icons) | MIT |
| Datalith Cargo package metadata | `license = "MIT"` |
| GPUI Kit and its `gpui-pre-*` dependencies | Apache-2.0 |
| Pixeloid font files | SIL Open Font License 1.1 |
| Bundled third-party themes | Their respective licenses (see `THIRD-PARTY-NOTICES.md`) |
| Lucide icons (via `gpui-kit-assets`) | ISC, plus MIT for the Feather-derived subset |
| **Distributed Datalith executable** | **MIT, with third-party notices** |

## Original Datalith source is MIT

Datalith's original source code and original artwork remain licensed under the
MIT License. The full text is in [LICENSE](LICENSE). Downstream users may
continue to exercise the MIT grant over original Datalith code and assets.

Datalith's pixel-art icons are original 7x7 pixel grids generated from ASCII
source files (`assets/icons/*.txt` via `scripts/txt2svg.py`). They are
stylistically inspired by line-icon sets such as Nucleo Arcade, but they are
original works, not copies of those proprietary icons. They are MIT-covered
first-party assets.

## Distributed binaries are conveyed under MIT

Datalith's release binaries are conveyed under the MIT License. The bundled
third-party components retain their own licenses; their complete notices and
license texts are included in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Third-party components

Third-party components retain their own licenses. The complete, generated
inventory — with license expressions, copyright notices, and full license texts
for Rust dependencies and bundled assets — is in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md). The canonical non-Cargo asset
inventory is [assets/licenses.toml](assets/licenses.toml).

The retained Twilight theme uses a custom permissive grant and is explicitly
documented for maintainer review; see "Bundled theme review" in
`THIRD-PARTY-NOTICES.md`.

## Release source archive

Each release publishes a complete, vendored source archive named:

```text
datalith-X.Y.Z-corresponding-source.tar.zst
```

where `X.Y.Z` is the released version. The archive contains the exact source
tree for the release commit, the vendored `Cargo.lock` and `vendor/` directory,
and the scripts needed to rebuild each supported platform binary offline.

To find the exact source for an installed version, use the version shown in
`Settings -> About`. The corresponding-source archive for that version is
attached to the GitHub Release named `vX.Y.Z` at:

```text
https://github.com/mycelium-build/datalith-app/releases/tag/vX.Y.Z
```

Pre-release versions (`vX.Y.Z-rc.N`) follow the same convention and use their
full version string.

## No warranty

Each component is distributed without warranty under the terms of its own
license; see the individual license texts, in particular the disclaimer in
[LICENSE](LICENSE) and the licenses reproduced in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
