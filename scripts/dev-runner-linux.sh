#!/bin/sh
# Cargo runner for Linux (see .cargo/config.toml): runs every binary Cargo starts, unchanged.
#
# Linux desktops find a window's icon from its app id (`datalith-dev` for the Dev channel) by
# looking for a matching .desktop file; a `cargo run` build has none, so it shows a generic icon.
# When a Dev build of Datalith starts, this installs (or refreshes) a hidden
# ~/.local/share/applications/datalith-dev.desktop pointing at it and the green Dev icon in
# ~/.local/share/icons. Tests, other binaries, Stable/Preview checkouts and CI are passed straight
# through.
#
# The build runs as a `datalith-dev` hard link next to the binary: KDE Plasma matches a window's
# executable name against StartupWMClass before its app id, so a process named `datalith` would be
# grouped under the installed Stable entry and get its blue icon.
#
#   scripts/dev-runner-linux.sh --uninstall   removes the entry and the icons
#   DATALITH_DEV_DESKTOP_ENTRY=0              leaves them alone

set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
STEM=datalith-dev
DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}
DESKTOP_FILE=$DATA_HOME/applications/$STEM.desktop
ICONS=$DATA_HOME/icons/hicolor
# Marks entries this script wrote, so it only ever refreshes or removes its own.
MARKER=X-Datalith-Dev-Entry=true

refresh_caches() {
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache --force --ignore-theme-index --quiet "$ICONS" 2>/dev/null || true
    fi
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database --quiet "$DATA_HOME/applications" 2>/dev/null || true
    fi
}

install_entry() {
    binary=$1
    for png in "$ROOT"/assets/logo/dev/hicolor/*/apps/datalith.png; do
        size=$(basename "$(dirname "$(dirname "$png")")")
        mkdir -p "$ICONS/$size/apps"
        cp "$png" "$ICONS/$size/apps/$STEM.png"
    done
    mkdir -p "$ICONS/scalable/apps" "$(dirname "$DESKTOP_FILE")"
    cp "$ROOT/assets/logo/dev/datalith.svg" "$ICONS/scalable/apps/$STEM.svg"
    # NoDisplay keeps the Dev build out of the application menu; no MimeType, so datalith://
    # links keep opening the installed Stable app.
    cat >"$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Datalith Dev
Comment=Development build of Datalith ($ROOT)
Exec="$binary"
Icon=$STEM
StartupWMClass=$STEM
Terminal=false
NoDisplay=true
$MARKER
EOF
    refresh_caches
    echo "Installed the Datalith Dev icon ($DESKTOP_FILE); remove it with scripts/dev-runner-linux.sh --uninstall." >&2
}

uninstall_entry() {
    if [ -f "$DESKTOP_FILE" ] && grep -qx "$MARKER" "$DESKTOP_FILE"; then
        rm -f "$DESKTOP_FILE"
    fi
    rm -f "$ICONS"/*/apps/$STEM.png "$ICONS/scalable/apps/$STEM.svg"
    refresh_caches
    echo "Removed the Datalith Dev desktop entry and icons."
}

is_dev_app() {
    # Only the app itself (target/<profile>/datalith), not test binaries (target/<profile>/deps/datalith-<hash>).
    [ "$(basename -- "$1")" = datalith ] || return 1
    [ "$(basename -- "$(dirname -- "$1")")" != deps ] || return 1
    # Stable and Preview checkouts would clash with the packaged apps' entries.
    [ "$(tr -d '[:space:]' <"$ROOT/CHANNEL")" = dev ] || return 1
    [ "${DATALITH_DEV_DESKTOP_ENTRY:-1}" != 0 ] && [ -z "${CI:-}" ]
}

ensure_entry() {
    binary=$(CDPATH='' cd -- "$(dirname -- "$1")" && pwd)/$STEM

    if [ -f "$DESKTOP_FILE" ]; then
        # Never touch an entry someone else wrote.
        grep -qx "$MARKER" "$DESKTOP_FILE" || return 0
        # Up to date: same binary and icons no older than the checkout's.
        if grep -qxF "Exec=\"$binary\"" "$DESKTOP_FILE" &&
            [ ! "$ROOT/assets/logo/dev/datalith.svg" -nt "$ICONS/scalable/apps/$STEM.svg" ]; then
            return 0
        fi
    fi
    install_entry "$binary"
}

if [ "${1:-}" = "--uninstall" ]; then
    uninstall_entry
    exit 0
fi
if [ "$#" -gt 0 ] && is_dev_app "$1"; then
    # A failure here must never keep the binary from running.
    link=$(dirname -- "$1")/$STEM
    if ln -f -- "$1" "$link" 2>/dev/null; then
        shift
        set -- "$link" "$@"
        (ensure_entry "$1") || echo "Could not install the Datalith Dev icon; continuing." >&2
    fi
fi
exec "$@"
