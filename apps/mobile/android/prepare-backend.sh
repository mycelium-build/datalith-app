#!/bin/sh
set -eu
# Prepare the pinned GPUI Android backend before compiling the NativeActivity bridge.
cd "$(dirname "$0")"
root=$(git rev-parse --show-toplevel)
backend="$root/target/android/gpui-mobile"
rev=7548ebc4b24f00494af9976af8852c7163ded9ce
if ! test -d "$backend/.git"; then
    git init -q "$backend"
fi
if ! git -C "$backend" cat-file -e "$rev^{commit}" 2>/dev/null; then
    git -C "$backend" fetch --depth=1 https://github.com/longbridge/gpui-mobile "$rev"
fi
git -C "$backend" checkout -q --detach "$rev"
test "$(git -C "$backend" rev-parse HEAD)" = "$rev"
if ! git -C "$backend" apply --reverse --check "$PWD/gpui-mobile.patch" 2>/dev/null; then
    git -C "$backend" apply "$PWD/gpui-mobile.patch"
fi
if ! git -C "$backend" apply --reverse --check "$PWD/android-dispatch.patch" 2>/dev/null; then
    git -C "$backend" apply "$PWD/android-dispatch.patch"
fi
