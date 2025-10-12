#!/bin/sh

set -ex

# A couple of steps are necessary to get this build working which makes it slightly
# nonstandard compared to most other builds.
#
# * First, the Rust standard library needs to be recompiled with atomics
#   enabled. to do that we use Cargo's unstable `-Zbuild-std` feature.
#
# * Next we need to compile everything with the `atomics` and `bulk-memory`
#   features enabled, ensuring that LLVM will generate atomic instructions,
#   shared memory, passive segments, etc.

RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals --cfg getrandom_backend="wasm_js"' \
  cargo +nightly build --features "webgl potree_wasm_worker" --example potree --target wasm32-unknown-unknown -Z build-std=std,panic_abort --profile wasm-release


wasm-bindgen --target web  --out-dir ./wasm --out-name "bevy_pointcloud"  ./target/wasm32-unknown-unknown/wasm-release/examples/potree.wasm
