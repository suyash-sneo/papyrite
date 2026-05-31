#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_OUT="$ROOT_DIR/packages/papyrite_flutter/android/src/main/jniLibs"
IOS_FRAMEWORKS_OUT="$ROOT_DIR/packages/papyrite_flutter/ios/Frameworks"
IOS_BUILD_OUT="$ROOT_DIR/target/papyrite-ios"
HEADER_DIR="$ROOT_DIR/ffi/include"

build_android() {
    if ! command -v cargo-ndk >/dev/null 2>&1; then
        echo "cargo-ndk is required for Android builds."
        echo "Install it with: cargo install cargo-ndk"
        exit 1
    fi

    cargo ndk \
        -t armeabi-v7a \
        -t arm64-v8a \
        -t x86 \
        -t x86_64 \
        -o "$ANDROID_OUT" \
        build -p papyrite_ffi --release
}

build_ios() {
    if ! command -v xcodebuild >/dev/null 2>&1; then
        echo "xcodebuild is required for iOS xcframework packaging."
        exit 1
    fi

    rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

    cargo build -p papyrite_ffi --release --target aarch64-apple-ios
    cargo build -p papyrite_ffi --release --target aarch64-apple-ios-sim
    cargo build -p papyrite_ffi --release --target x86_64-apple-ios

    mkdir -p "$IOS_BUILD_OUT"
    lipo -create \
        "$ROOT_DIR/target/aarch64-apple-ios-sim/release/libpapyrite_ffi.a" \
        "$ROOT_DIR/target/x86_64-apple-ios/release/libpapyrite_ffi.a" \
        -output "$IOS_BUILD_OUT/libpapyrite_ffi_sim.a"

    rm -rf "$IOS_FRAMEWORKS_OUT/Papyrite.xcframework"
    xcodebuild -create-xcframework \
        -library "$ROOT_DIR/target/aarch64-apple-ios/release/libpapyrite_ffi.a" \
        -headers "$HEADER_DIR" \
        -library "$IOS_BUILD_OUT/libpapyrite_ffi_sim.a" \
        -headers "$HEADER_DIR" \
        -output "$IOS_FRAMEWORKS_OUT/Papyrite.xcframework"
}

case "${1:-all}" in
    android)
        build_android
        ;;
    ios)
        build_ios
        ;;
    all)
        build_android
        build_ios
        ;;
    *)
        echo "usage: $0 [android|ios|all]"
        exit 1
        ;;
esac
