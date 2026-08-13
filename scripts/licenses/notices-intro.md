# Datalith Third-Party Notices

Datalith includes third-party software and assets under a variety of open-source
licenses. This file records the components, their copyright notices, and the
license text under which each is distributed.

The Rust-dependency section is generated from `Cargo.lock` with `cargo-about`.
The bundled-assets section is generated from `assets/licences.toml`. This file
is produced by `scripts/licenses/generate.sh`; do not edit it by hand.

The complete scope map, including the GPL distribution boundary and
Corresponding Source, is in `LICENSING.md`.

## Review notes

The following bundled themes have licensing that is permissive but unusual or
not fully expressible as a standard SPDX identifier. They are retained for now
and flagged for focused legal review before the first public release:

- **Adventure, Fahrenheit, Harper, Kibble** (`iTerm2-Color-Schemes`): the
  collection repository is MIT, but upstream notes that each individual scheme
  belongs to its original author, whose separate license is not recorded in the
  collection.
- **Gruvbox**: no `LICENSE` file in the upstream repository; the README declares
  the license as "MIT/X11".
- **Molokai**: the `molokai.vim` source is MIT, but the palette is derived from
  the Monokai theme (copyright Wimer Hazenberg), whose own license is
  ambiguous.
- **Twilight**: distributed under a custom permissive grant from the TextMate
  themes bundle (no standard SPDX identifier).

If a review finds any of these unsuitable for redistribution, remove the theme
from `src/ui/themes/`, its `include_str!` registration, and its
`assets/licences.toml` entry in the same change.
