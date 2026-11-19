#!/bin/sh
set -eu
# Requires JDK 17, Android SDK 34, Build Tools 36 and NDK r27d.
# Install cargo-ndk and the aarch64-linux-android Rust target before building.
cd "$(dirname "$0")"
root=$(git rev-parse --show-toplevel)
backend="$root/target/android/gpui-mobile"
sh "$PWD/prepare-backend.sh"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${ANDROID_HOME:?Set ANDROID_HOME}/ndk/27.3.13750724}"
gradle="$backend/example/android/gradle"
mkdir -p app/build/gpui-java
for helper in GpuiInputActivity GpuiPlatformView GpuiCamera GpuiVideoPlayer; do
    cp "$gradle/app/src/main/java/dev/gpui/mobile/$helper.java" app/build/gpui-java/
done
cargo ndk -t arm64-v8a --platform 31 -o app/build/native build --lib --locked
sh "$gradle/gradlew" -p "$PWD" :app:assembleDebug
