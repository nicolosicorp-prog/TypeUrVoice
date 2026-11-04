#!/usr/bin/env bash
# Build the bundled `llama-server` sidecar used by VoiceInk's Built-in
# (on-device) text processor.
#
# - Pinned llama.cpp release, built from source — no downloaded executables.
# - Static (no dylibs) with Metal + the Metal shader library embedded, so the
#   result is a single self-contained file Tauri can ship via `externalBin`.
# - Output: src-tauri/binaries/llama-server-<target-triple>
#
# Usage: bash scripts/build-llama-server.sh [--force]
set -euo pipefail

LLAMA_TAG="v0.6.0"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
OUT_DIR="$ROOT/src-tauri/binaries"
OUT="$OUT_DIR/llama-server-$TRIPLE"
WORK="${LLAMA_BUILD_DIR:-$ROOT/src-tauri/target/llama.cpp-$LLAMA_TAG}"

if [[ -x "$OUT" && "${1:-}" != "--force" ]]; then
  echo "llama-server already built: $OUT ($("$OUT" --version 2>&1 | head -1))"
  exit 0
fi

command -v cmake >/dev/null || { echo "cmake not found (brew install cmake)"; exit 1; }

if [[ ! -d "$WORK/.git" ]]; then
  rm -rf "$WORK"
  git clone --depth 1 --branch "$LLAMA_TAG" https://github.com/ggml-org/llama.cpp.git "$WORK"
fi

GPU_FLAGS=()
if [[ "$(uname)" == "Darwin" ]]; then
  # GGML_NATIVE=OFF + an explicit baseline: a -mcpu=native build made on an
  # M3/M4 can use instructions M1 Macs lack (illegal-instruction crash).
  # armv8.5-a+fp16 is the M1 feature set.
  GPU_FLAGS=(-DGGML_METAL=ON -DGGML_METAL_EMBED_LIBRARY=ON -DGGML_NATIVE=OFF -DGGML_CPU_ARM_ARCH=armv8.5-a+fp16)
fi

# Keep the builder's home/checkout paths out of the binary (__FILE__ strings).
PREFIX_MAP="-ffile-prefix-map=$WORK=llama.cpp -ffile-prefix-map=$ROOT=. -ffile-prefix-map=$HOME=~"

cmake -S "$WORK" -B "$WORK/build" \
  -DCMAKE_BUILD_TYPE=Release \
  "-DCMAKE_C_FLAGS=$PREFIX_MAP" \
  "-DCMAKE_CXX_FLAGS=$PREFIX_MAP" \
  "-DCMAKE_OBJC_FLAGS=$PREFIX_MAP" \
  -DBUILD_SHARED_LIBS=OFF \
  -DLLAMA_CURL=OFF \
  -DLLAMA_OPENSSL=OFF \
  -DLLAMA_BUILD_TESTS=OFF \
  -DLLAMA_BUILD_EXAMPLES=OFF \
  -DLLAMA_BUILD_SERVER=ON \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 \
  "${GPU_FLAGS[@]}"
cmake --build "$WORK/build" --config Release --target llama-server -j "$(sysctl -n hw.ncpu 2>/dev/null || nproc)"

BUILT="$WORK/build/bin/llama-server"

# Fail loudly if the binary links against non-system dylibs (e.g. Homebrew
# OpenSSL) — those don't exist on users' Macs.
if [[ "$(uname)" == "Darwin" ]] && otool -L "$BUILT" | tail -n +2 | grep -vE '/usr/lib/|/System/Library/' ; then
  echo "error: $BUILT links non-system libraries (see above)"; exit 1
fi

mkdir -p "$OUT_DIR"
cp "$BUILT" "$OUT"
chmod +x "$OUT"
echo "Built $OUT ($("$OUT" --version 2>&1 | head -1))"
