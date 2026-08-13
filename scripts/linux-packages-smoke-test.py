"""Build the native Linux packages in CI parity and smoke-test them in distrobox.

CI builds on ubuntu-22.04 (glibc 2.35), so the release binary must be compiled
inside an ubuntu:22.04 container — building on a newer host (e.g. Fedora 44,
glibc 2.43) produces a binary no released Debian can run. This script mirrors
the release workflow exactly:

  1. create a distrobox builder (ubuntu:22.04)
  2. install build deps + Rust (pinned in rust-toolchain.toml) + nFPM + cargo-packager
  3. cargo build --release --locked --target x86_64-unknown-linux-gnu
  4. nfpm package for deb, rpm, archlinux; cargo packager for the AppImage
  5. create one distrobox per format and smoke-test it:
     - deb on debian trixie
     - rpm on fedora 44
     - archlinux on arch
     - AppImage on debian trixie (self-contained, extracted and run)
     Each native container installs the package with its manager (resolving the
     declared runtime deps), checks `ldd` resolves every library, then launches
     the app under Xvfb and verifies it stays up.

Usage:
  uv run scripts/smoke-native-packages.py [--build/--no-build] [--cleanup]

  --build     (default) run the CI-parity build before smoke-testing.
  --no-build  reuse packages already in target/x86_64-unknown-linux-gnu/release.
  --cleanup   remove distrobox containers after the run.

Requires: distrobox (with podman or docker) on a Linux host.

PATCH : this script should be checked against the latest version of the
.github/workflows/release.yml action, to ensure it's aligned with the release process.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import shlex
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PKG_DIR = ROOT / "target" / "x86_64-unknown-linux-gnu" / "release"
CONTAINER = "datalith-builder"
BUILDER_IMAGE = "docker.io/library/ubuntu:22.04"
NFPM_VERSION = "2.47.0"

# package extension -> (smoke-test distro name, image, install command)
FORMATS = {
    ".deb": {
        "distro": "debian",
        "image": "docker.io/library/debian:trixie",
        "install": "sudo apt-get update -qq && sudo apt-get install -y --no-install-recommends /tmp/datalith.deb",
        "xvfb": "sudo apt-get install -y --no-install-recommends xvfb",
    },
    ".rpm": {
        "distro": "fedora",
        "image": "docker.io/library/fedora:44",
        "install": "sudo dnf install -y /tmp/datalith.rpm",
        "xvfb": "sudo dnf install -y xorg-x11-server-Xvfb",
    },
    ".pkg.tar.zst": {
        "distro": "arch",
        "image": "docker.io/library/archlinux:latest",
        "install": "sudo pacman -U --noconfirm /tmp/datalith.pkg.tar.zst",
        "xvfb": "sudo pacman -S --noconfirm xorg-server-xvfb",
    },
}

# The AppImage bundles its own shared libraries via linuxdeploy, so it is
# self-contained: no runtime deps to install, just extract and run. We still
# test it on a foreign distro to prove portability.
APPIMAGE = {
    "distro": "debian",
    "image": "docker.io/library/debian:trixie",
    "xvfb": "sudo apt-get install -y --no-install-recommends xvfb",
}



def run(cmd: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    """Run a command on the host, capturing output for inspection."""
    print("+ " + " ".join(shlex.quote(str(c)) for c in cmd), file=sys.stderr)
    result = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if result.stdout:
        sys.stderr.write(result.stdout)
    if result.stderr:
        sys.stderr.write(result.stderr)
    if check and result.returncode != 0:
        raise subprocess.CalledProcessError(
            result.returncode, cmd, output=result.stdout, stderr=result.stderr
        )
    return result


def distrobox_cmd(container: str, cmd: list[str]) -> list[str]:
    return ["distrobox", "enter", "--name", container, "--", *cmd]


def container_exists(name: str) -> bool:
    listing = subprocess.run(
        ["distrobox", "list"], capture_output=True, text=True, check=False
    ).stdout
    return bool(re.search(rf"\b{re.escape(name)}\b", listing))


def package_version() -> str:
    match = re.search(
        r'^version\s*=\s*"([^"]+)"',
        (ROOT / "Cargo.toml").read_text(),
        flags=re.MULTILINE,
    )
    if not match:
        raise SystemExit("cannot read version from Cargo.toml")
    return match.group(1)


def ensure_container(name: str, image: str) -> None:
    if container_exists(name):
        return
    print(f"[{name}] creating from {image}")
    run(["distrobox", "create", "--yes", "--name", name, "--image", image])
    # Freshly created containers sometimes start with a not-yet-settled
    # filesystem (first boot race): touch a file so /tmp is really writable
    # before the smoke test copies the package in.
    run(distrobox_cmd(name, ["bash", "-c", "echo ready > /tmp/.distrobox-ready"]))


def build_packages() -> None:
    """Mirror the CI build steps inside the ubuntu:22.04 builder container."""
    ensure_container(CONTAINER, BUILDER_IMAGE)
    c = distrobox_cmd

    print("[builder] installing Linux build dependencies")
    run(c(CONTAINER, ["bash", "-c", f"cd {ROOT} && ./scripts/install-linux-build-dependencies.sh"]))

    print("[builder] installing AppImage build tools (file, zstd)")
    run(c(CONTAINER, ["bash", "-c",
        "sudo apt-get install -y --no-install-recommends file zstd"]))

    rust = "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.96.0"
    print("[builder] installing Rust (as pinned in rust-toolchain.toml)")
    run(c(CONTAINER, ["bash", "-c", rust + " && echo 'export PATH=$HOME/.cargo/bin:$PATH' >> ~/.bashrc"]))

    print(f"[builder] installing nFPM {NFPM_VERSION}")
    nfpm_url = (
        f"https://github.com/goreleaser/nfpm/releases/download/v{NFPM_VERSION}/"
        f"nfpm_{NFPM_VERSION}_Linux_x86_64.tar.gz"
    )
    run(c(CONTAINER, ["bash", "-c",
        f"curl -fsSL {nfpm_url} -o /tmp/nfpm.tar.gz && "
        "sudo tar xzf /tmp/nfpm.tar.gz -C /usr/local/bin nfpm && nfpm --version"]))

    print("[builder] building release binary (CI parity)")
    run(c(CONTAINER, ["bash", "-c",
        f"cd {ROOT} && export PATH=$HOME/.cargo/bin:$PATH && "
        "cargo clean && "
        "cargo build --release --locked --target x86_64-unknown-linux-gnu"]))

    print("[builder] packaging deb / rpm / archlinux")
    env = (
        f"export PACKAGE_VERSION={package_version()} "
        "PACKAGE_ARCH=x86_64 PACKAGE_TARGET=x86_64-unknown-linux-gnu"
    )
    run(c(CONTAINER, ["bash", "-c",
        f"cd {ROOT} && export PATH=$HOME/.cargo/bin:$PATH && {env} && "
        "for packager in deb rpm archlinux; do "
        "nfpm package --config nfpm.yaml --packager $packager "
        "--target target/x86_64-unknown-linux-gnu/release/; done"]))

    print("[builder] building AppImage (CI parity)")
    run(c(CONTAINER, ["bash", "-c",
        "export PATH=$HOME/.cargo/bin:$PATH && "
        "cargo install cargo-packager --version 0.11.8 --locked && cargo-packager --version"]))
    run(c(CONTAINER, ["bash", "-c",
        f"cd {ROOT} && export PATH=$HOME/.cargo/bin:$PATH && "
        "APPIMAGE_EXTRACT_AND_RUN=1 cargo packager --release "
        "--target x86_64-unknown-linux-gnu --formats appimage"]))


def find_packages() -> list[pathlib.Path]:
    packages = []
    for suffix in FORMATS:
        packages.extend(PKG_DIR.glob(f"*{suffix}"))
    packages.extend(PKG_DIR.glob("*.AppImage"))
    if not packages:
        raise SystemExit(f"no packages found in {PKG_DIR}")
    return packages


def smoke_test_appimage(pkg: pathlib.Path, cleanup: bool) -> bool:
    distro = APPIMAGE["distro"]
    container = f"datalith-smoke-{distro}"
    print(f"\n=== [{distro}] {pkg.name} (AppImage) ===")

    ensure_container(container, APPIMAGE["image"])
    c = distrobox_cmd
    remote = "/tmp/datalith.AppImage"

    run(c(container, ["sudo", "cp", str(pkg), remote]))
    run(c(container, ["sudo", "chmod", "+x", remote]))

    print(f"  installing Xvfb on {distro}...")
    run(c(container, ["bash", "-c", APPIMAGE["xvfb"]]), check=False)

    print(f"  launching AppImage under Xvfb on {distro} (15s)...")
    result = run(
        c(container, [
            "bash", "-c",
            "timeout 15 xvfb-run -a env APPIMAGE_EXTRACT_AND_RUN=1 /tmp/datalith.AppImage",
        ]),
        check=False,
    )
    ok = result.returncode in (0, 124)
    if ok:
        print(f"  OK: AppImage stayed up on {distro}")
    else:
        print(f"  FAIL: AppImage crashed on {distro} (exit {result.returncode})", file=sys.stderr)

    if cleanup:
        run(["distrobox", "rm", "--force", "--yes", container], check=False)
    return ok


def format_for(pkg: pathlib.Path) -> str:
    """Return the FORMATS key for a package path (.deb, .rpm, .pkg.tar.zst)."""
    name = pkg.name
    for suffix in FORMATS:
        if name.endswith(suffix):
            return suffix
    return pkg.suffix


def smoke_test(pkg: pathlib.Path, cleanup: bool) -> bool:
    if pkg.suffix == ".AppImage":
        return smoke_test_appimage(pkg, cleanup)
    spec = FORMATS[format_for(pkg)]
    distro = spec["distro"]
    container = f"datalith-smoke-{distro}"
    print(f"\n=== [{distro}] {pkg.name} ===")

    ensure_container(container, spec["image"])
    c = distrobox_cmd
    remote = f"/tmp/datalith{pkg.suffix}"

    run(c(container, ["sudo", "cp", str(pkg), remote]))

    print(f"  installing package on {distro}...")
    result = run(c(container, ["bash", "-c", spec["install"]]), check=False)
    if result.returncode != 0:
        print(f"  FAIL: package installation failed on {distro}", file=sys.stderr)
        if cleanup:
            run(["distrobox", "rm", "--force", "--yes", container], check=False)
        return False

    print(f"  checking linked libraries on {distro}...")
    result = run(c(container, ["bash", "-c", "ldd /usr/bin/datalith"]), check=False)
    if result.returncode != 0:
        print(f"  FAIL: could not run ldd on {distro}", file=sys.stderr)
        if cleanup:
            run(["distrobox", "rm", "--force", "--yes", container], check=False)
        return False
    if "not found" in result.stdout:
        print(f"  FAIL: unresolved shared libraries on {distro}:", file=sys.stderr)
        for line in result.stdout.splitlines():
            if "not found" in line:
                print("    " + line, file=sys.stderr)
        if cleanup:
            run(["distrobox", "rm", "--force", "--yes", container], check=False)
        return False

    print(f"  installing Xvfb on {distro}...")
    run(c(container, ["bash", "-c", spec["xvfb"]]), check=False)

    print(f"  launching app under Xvfb on {distro} (15s)...")
    result = run(
        c(container, ["bash", "-c", "timeout 15 xvfb-run -a /usr/bin/datalith"]),
        check=False,
    )
    ok = result.returncode in (0, 124)
    if ok:
        print(f"  OK: app stayed up on {distro}")
    else:
        print(f"  FAIL: app crashed on {distro} (exit {result.returncode})", file=sys.stderr)

    if cleanup:
        run(["distrobox", "rm", "--force", "--yes", container], check=False)
    return ok


def main() -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    parser.add_argument(
        "--build",
        dest="build",
        action="store_true",
        default=True,
        help="run the CI-parity build before smoke-testing (default)",
    )
    parser.add_argument(
        "--no-build",
        dest="build",
        action="store_false",
        help="reuse packages already in target/ instead of building",
    )
    parser.add_argument(
        "--cleanup",
        action="store_true",
        help="remove distrobox containers after the run",
    )
    args = parser.parse_args()

    if shutil.which("distrobox") is None:
        raise SystemExit("distrobox not found in PATH")

    if args.build:
        build_packages()

    failed = 0
    for pkg in find_packages():
        if not smoke_test(pkg, args.cleanup):
            failed = 1

    return failed


if __name__ == "__main__":
    raise SystemExit(main())
