# Contributing

We welcome contributions! Here's how you can get involved:

## Getting Started

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Project Guidelines

- Your branch name should follow [Conventional Branch](https://conventionalbranch.org/) format
- Use Rust with strict compiler checks (`#![deny(warnings)]`)
- Follow the existing code style
- Add doc comments and update the Datalith doc for public items
- Write clear, descriptive commit messages following [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) style

## Testing

Before submitting a pull request, please ensure that:

1. The project builds without errors (`cargo build`)
2. You run fmt (`cargo fmt`)
3. You run clippy and fix all issues (`cargo clippy`)
4. All tests pass (`cargo test`)
5. The code follows the project guidelines
6. New functionality includes appropriate tests

## Third-Party Notices

When adding a Rust dependency, update `Cargo.toml` and `Cargo.lock`. 
When adding a bundled asset, add it to `assets/licenses.toml` and include its license file.
Do not edit `THIRD-PARTY-NOTICES.md` by hand; regenerate it with:

```bash
uv run --no-project --python '>=3.11' -- scripts/licenses/generate.sh
```

The generation script requires the `cargo-about` version pinned in `scripts/tool-versions.env`.
Before submitting a pull request, verify the result with the pinned `cargo-deny` using:

```bash
uv run --no-project --python '>=3.11' -- scripts/licenses/check.sh
```

## Dependency Updates

- Dependabot opens grouped PRs every Monday (`patch`, `minor`, `major`, titled `fix(deps): …`), plus one PR per security advisory. They are reviewed and merged by hand.
- On Dependabot Cargo PRs, the **Dependency notices** workflow regenerates `THIRD-PARTY-NOTICES.md` and pushes it, so `license-compliance` passes on the rerun.
- The **Cargo.lock refresh** workflow opens one `chore(deps): refresh Cargo.lock` PR each Monday for transitive crates, which Dependabot only updates for security fixes.
- The **Advisories** workflow checks RustSec advisories on `main` daily.
- Dependabot also bumps the Rust toolchain in `rust-toolchain.toml`; the workflows install the toolchain from that file.
- Tools Dependabot cannot see (cargo-deny, cargo-about, cargo-packager, nFPM) are pinned once in `scripts/tool-versions.env`. The **Tool versions** workflow checks for new releases every Monday and opens one `chore(ci): bump tool versions` PR. After it is merged, run `scripts/install-dev-tools.sh` again.

## Development Setup

### Prerequisites

- Rust
- Git
- [uv](https://docs.astral.sh/uv/), for Python tools and scripts
- [prek](https://prek.j178.dev), to run the Git hooks
- For changes to dependencies or license material: `cargo-deny` and `cargo-about` at the versions
  pinned in `scripts/tool-versions.env` (install them with `scripts/install-dev-tools.sh`).
  The license scripts run with uv (see [Third-Party Notices](#third-party-notices)).

### Installation

1. Clone the repository:
    ```bash
    git clone https://github.com/mycelium-build/datalith-app.git
    cd datalith-app
    ```

2. Install prek and enable the Git hooks:
    ```bash
    uv tool install prek
    prek install
    ```
    `prek install` reads `.pre-commit-config.yaml` and installs both hooks once for the clone:

    | Hook | Runs when | Checks |
    |---|---|---|
    | pre-commit | Rust files are staged | `cargo fmt --all`; the formatting of the committed files is staged, so the commit goes through formatted |
    | pre-commit | dependency, license or `assets/` files are staged | `scripts/licenses/check.sh` without advisories (CI still checks them); stale notices are regenerated and staged, so the commit goes through with them. A real license or policy failure still stops the commit |
    | pre-push | the pushed commits touch Rust code or its build files | `cargo clippy --locked --all-targets` |

    The license hook needs the license tools at their pinned versions:
    ```bash
    scripts/install-dev-tools.sh
    ```
    The fixes only touch what you staged: prek sets unstaged changes aside while the hooks run. If formatting conflicts with unstaged changes in the same file, prek drops the fix and stops the commit; run `cargo fmt --all`, stage, and commit again.
    Run every pre-commit hook on the whole tree with `prek run --all-files`.
    Bypass a hook once with `--no-verify`.

3. Build the project:
    ```bash
    cargo build
    ```

4. Run the project:
    ```bash
    cargo run
    ```
    The Dev build shows its green icon: on macOS it sets its Dock icon at startup; on Linux, Cargo runs it through `scripts/dev-runner-linux.sh`, which installs a hidden `datalith-dev.desktop` entry and the Dev icon under `~/.local/share` (remove them with `scripts/dev-runner-linux.sh --uninstall`, or skip them with `DATALITH_DEV_DESKTOP_ENTRY=0`); on Windows the icon is embedded in the executable.

### Development Workflow

1. Make changes to the source code in the `src/` directory.

2. Run tests:
    ```bash
    cargo test
    ```

3. Check for lint issues:
    ```bash
    cargo clippy
    ```

## Our stance on AI

We allow PR made with AI, but you are responsible for your code. This implies that you have reread all your code and reaches a good quality. You should always understand what you have done. If you clearly did not read the code or it is fully vibe coded we allow ourselves to close the PR without reason.

## License

By contributing to this project, you agree that your contributions will be licensed under the MIT License.
