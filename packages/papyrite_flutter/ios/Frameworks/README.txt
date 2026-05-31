This directory is populated by scripts/build-mobile-libs.sh.

Expected output:

    Papyrite.xcframework

The Dart loader uses DynamicLibrary.process() on iOS because the native Rust
symbols are linked into the app process through this vendored framework.
