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
scripts/licenses/generate.sh
```

The generation script requires `cargo-about` version `0.9.1`. 
Before submitting a pull request, verify the result with `cargo-deny` version `0.20.2` using:

```bash
scripts/licenses/check.sh
```

## Development Setup

### Prerequisites

- Rust
- Git

### Installation

1. Clone the repository:
    ```bash
    git clone https://github.com/mycelium-build/datalith-app.git
    cd datalith
    ```

2. Build the project:
    ```bash
    cargo build
    ```

3. Run the project:
    ```bash
    cargo run
    ```

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
