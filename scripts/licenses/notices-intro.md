# Datalith Third-Party Notices

Datalith includes third-party software and assets under a variety of open-source
licenses. This file records the components, their copyright notices, and the
license text under which each is distributed.

The Rust-dependency section is generated from `Cargo.lock` with `cargo-about`.
The bundled-assets section is generated from `assets/licences.toml`. This file
is produced by `scripts/licenses/generate.sh`; do not edit it by hand.

The complete scope map, including the GPL distribution boundary and
Corresponding Source, is in `LICENSING.md`.

## Bundled theme review

Five themes were removed after review against their upstream licenses, and two
were retained with corrected provenance:

- **Removed — Adventure, Fahrenheit, Harper, Kibble** (`iTerm2-Color-Schemes`):
  the collection's MIT license explicitly covers only the collection itself and
  notes that each individual scheme belongs to its original author, whose
  license is not recorded in the collection. No chain of title could be
  established, so the themes were removed.
- **Removed — Molokai**: the MIT license on `molokai.vim` covers the Vim source
  only; the Monokai palette (copyright Wimer Hazenberg) that the theme's colors
  derive from has no license.
- **Retained — Gruvbox**: no standalone `LICENSE` file upstream, but the README
  declares "MIT/X11" (MIT) and Debian packages it as MIT under
  copyright Pavel Pertsev (morhetz).
- **Retained — Twilight**: distributed under the TextMate themes bundle's
  permissive grant (copy/use/modify/sell/distribute), recorded as
  `LicenseRef-TextMateThemesBundle`.
