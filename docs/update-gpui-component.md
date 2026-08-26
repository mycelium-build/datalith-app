# Update GPUI Component

This guide explains how to update `gpui-component` while keeping it compatible with GPUI.

## 1. Choose the gpui-component commit

Find the commit you want to use on the [gpui-component repository](https://github.com/longbridge/gpui-component). Use the full commit hash, not a branch or a short hash.

For this project, the dependency entries look like this:

```toml
gpui-component = { git = "https://github.com/longbridge/gpui-component", rev = "GPUI_COMPONENT_COMMIT" }
gpui-component-assets = { git = "https://github.com/longbridge/gpui-component", rev = "GPUI_COMPONENT_COMMIT" }
```

Keep the same commit for `gpui-component` and `gpui-component-assets`.

## 2. Find the matching GPUI commit

Open the `Cargo.lock` file at the selected `gpui-component` commit. Search for the `gpui` package and copy the full commit hash from its source entry:

```toml
name = "gpui"
source = "git+https://github.com/zed-industries/zed#GPUI_COMMIT"
```

The GPUI commit is the hash after `zed#`. Do not choose the latest GPUI commit separately; `gpui-component` may depend on an older GPUI API.

## 3. Keep the GPUI URLs revisionless

Declare the Zed dependencies without `rev` in your `Cargo.toml`:

```toml
gpui = { git = "https://github.com/zed-industries/zed" }
gpui_platform = { git = "https://github.com/zed-industries/zed", features = ["font-kit", "wayland", "x11"] }
reqwest_client = { git = "https://github.com/zed-industries/zed" }
```

This is intentional. `gpui-component` uses the revisionless Zed URL internally. Adding `rev` to the application's direct dependency creates a different Cargo source identity, even when both URLs resolve to the same commit. Cargo then builds two copies of GPUI, and their traits and types are incompatible.

## 4. Pin GPUI in Cargo.lock

After changing `Cargo.toml`, update the revisionless GPUI source to the commit found in step 2:

```sh
cargo update 'git+https://github.com/zed-industries/zed#gpui@0.2.2' --precise GPUI_COMMIT
```

Use the GPUI version shown in your dependency tree if it is not `0.2.2`. The command updates the related Zed workspace packages to the same commit.

Commit `Cargo.lock`. The lockfile is what pins the exact GPUI commit for this revisionless git dependency.

## 5. Check for duplicate GPUI versions

Run:

```sh
cargo tree -p gpui --depth 0
cargo tree -i gpui --depth 2
```

All GPUI entries must use one source ending in the selected commit. There must not be one source containing `?rev=` and another source without it.

## 6. Update API changes

If the dependency graph is unified but the application still fails to compile, fix the API changes for the selected `gpui-component` commit. Use the compiler errors to find remaining API changes.

## 7. Verify the update

Run the formatter, locked check, and tests:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test
```

Do not use `cargo update` without `--precise` for this update. It can move the unpinned git dependency to the current Zed revision and reintroduce the duplicate-GPUI problem.
