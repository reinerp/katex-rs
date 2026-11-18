#!/bin/bash
# Builds the WebAssembly bundle of this crate into dist/: katex.wasm and the glue katex-wasm.js.
#
#   crates/katex-wasm/build.sh            the shipped bundle: hybrid output only, size-optimised
#   FULL=1 crates/katex-wasm/build.sh     every output mode (stock KaTeX markup with MathML, HTML only, flat) as well, for comparison
#   STABLE=1 crates/katex-wasm/build.sh   with the stable toolchain (no build-std: larger, see README.md)
#
# Needs the nightly toolchain with the rust-src component and the wasm32-unknown-unknown target:
#   rustup toolchain install nightly --profile minimal --component rust-src --target wasm32-unknown-unknown
# (built with rustc 1.102.0-nightly 59cbdee2e, 2026-11-17). With the optional `wasm-opt` of Binaryen (WASM_OPT=/path/to/wasm-opt, default: the one on PATH)
# the module's locals are reordered, which makes it compress about 1.3% better. Build-std needs the sources of the standard library's dependencies:
# with network access cargo fetches them, offline point a source replacement at a directory with the contents of
# $(rustc +nightly --print sysroot)/lib/rustlib/src/rust/library/vendor.
#
# OPT_LEVEL (default z) and INLINE (llvm inline threshold, default 25) trade size for speed: see README.md.
set -euo pipefail
cd "$(dirname "$0")/../.."
OPT_LEVEL=${OPT_LEVEL:-z}
INLINE=${INLINE:-25}
OUT=crates/katex-wasm/dist
mkdir -p "$OUT"
features=()
[ "${FULL:-0}" = 1 ] && features=(--no-default-features --features bump,full)
export CARGO_PROFILE_WASM_OPT_LEVEL=$OPT_LEVEL
if [ "${STABLE:-0}" = 1 ]; then
  export RUSTFLAGS="-C llvm-args=-inline-threshold=$INLINE ${RUSTFLAGS:-}"
  cargo build --profile wasm --target wasm32-unknown-unknown -p katex-wasm "${features[@]}"
else
  # no panic machinery: a panic is a trap
  export CARGO_PROFILE_WASM_PANIC=immediate-abort
  export RUSTFLAGS="-Zunstable-options -C llvm-args=-inline-threshold=$INLINE ${RUSTFLAGS:-}"
  cargo +nightly build --profile wasm --target wasm32-unknown-unknown -p katex-wasm "${features[@]}" \
    -Zbuild-std=std,panic_abort -Zbuild-std-features=optimize_for_size -Zpanic-immediate-abort
fi
cp "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/wasm/katex_wasm.wasm" "$OUT/katex.wasm"
WASM_OPT=${WASM_OPT:-$(command -v wasm-opt || true)}
if [ -n "$WASM_OPT" ]; then
  "$WASM_OPT" --reorder-locals --all-features "$OUT/katex.wasm" -o "$OUT/katex.wasm"
fi
cp crates/katex-wasm/js/katex-wasm.js "$OUT/"
ls -l "$OUT"
