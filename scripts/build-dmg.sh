#!/usr/bin/env bash
#
# build-dmg.sh — builds VoiceInk and packages a styled DMG.
#
# Tauri 2's CLI invokes bundle_dmg.sh with missing arguments (upstream bug),
# so its DMG step always fails with "Not enough arguments". This script runs
# `npm run tauri build` to produce the .app, ignores the expected DMG failure,
# cleans orphan `rw.*.dmg` temp files that pile up after each failed attempt,
# then invokes bundle_dmg.sh directly with the arguments Tauri would have
# passed, producing a proper styled drag-to-Applications installer.

set -eu

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APP_NAME="VoiceInk"
# Read version from Cargo.toml so we never drift.
VERSION="$(grep '^version' src-tauri/Cargo.toml | head -1 | sed -E 's/version *= *"([^"]+)".*/\1/')"
ARCH="$(uname -m)"  # arm64 on Apple Silicon, x86_64 on Intel
# Tauri names DMGs with aarch64 instead of arm64; keep parity.
[ "$ARCH" = "arm64" ] && ARCH="aarch64"

DMG_NAME="${APP_NAME}_${VERSION}_${ARCH}.dmg"
BUNDLE_DIR="src-tauri/target/release/bundle"
MACOS_DIR="${BUNDLE_DIR}/macos"
DMG_DIR="${BUNDLE_DIR}/dmg"

echo "=== VoiceInk DMG builder ==="
echo "Version: ${VERSION}  Arch: ${ARCH}"

# 0. Bundled llama-server sidecar (Built-in text processor). No-op when the
# pinned build already exists; Tauri's externalBin step fails without it.
echo ""
echo "--- Step 0/3: llama-server sidecar ---"
bash scripts/build-llama-server.sh

# 1. Build the .app via Tauri. The DMG step will fail — we ignore that exit code.
echo ""
echo "--- Step 1/3: Building .app via Tauri (DMG step expected to fail) ---"
set +e
npm run tauri build
set -e

# 2. Sanity-check the .app actually exists. If it doesn't, something other
# than the DMG step failed and we should bail loud.
if [ ! -d "${MACOS_DIR}/${APP_NAME}.app" ]; then
  echo ""
  echo "ERROR: ${MACOS_DIR}/${APP_NAME}.app was not produced."
  echo "The Tauri build failed before reaching the DMG step — check output above."
  exit 1
fi

# 3. Clean orphan temp files that Tauri's failed DMG attempts leave behind.
# Without this the source folder bloats and --sandbox-safe would package them.
orphan_count="$(find "${MACOS_DIR}" -maxdepth 1 -name 'rw.*.dmg' | wc -l | tr -d ' ')"
if [ "${orphan_count}" -gt 0 ]; then
  echo ""
  echo "--- Cleaning ${orphan_count} orphan rw.*.dmg files from prior failed builds ---"
  rm -f "${MACOS_DIR}"/rw.*.dmg
fi

# Also detach any leftover /Volumes/VoiceInk mount from an aborted prior attempt.
if mount | grep -q "/Volumes/${APP_NAME}"; then
  echo "Detaching stale /Volumes/${APP_NAME} mount..."
  hdiutil detach "/Volumes/${APP_NAME}" -force >/dev/null 2>&1 || true
fi

# 4. Invoke bundle_dmg.sh directly with Tauri's intended arguments.
# --sandbox-safe avoids the RW-mount path that races on low-disk / Sonoma+.
# --skip-jenkins skips the AppleScript icon-positioning dance (brittle, and
# the result is functionally identical — a drop-target Applications link).
echo ""
echo "--- Step 2/3: Packaging DMG ---"
cd "${DMG_DIR}"
rm -f "${DMG_NAME}"

bash ./bundle_dmg.sh \
  --volname "${APP_NAME}" \
  --window-pos 200 120 \
  --window-size 600 400 \
  --icon-size 100 \
  --icon "${APP_NAME}.app" 150 190 \
  --app-drop-link 450 190 \
  --sandbox-safe \
  --skip-jenkins \
  "${DMG_NAME}" \
  "../macos/"

# 5. Summary.
echo ""
echo "--- Step 3/3: Done ---"
DMG_PATH="$(cd "$ROOT"; pwd)/${BUNDLE_DIR}/dmg/${DMG_NAME}"
echo "DMG:   ${DMG_PATH}"
echo "Size:  $(ls -lh "${DMG_PATH}" | awk '{print $5}')"
echo "SHA-256:"
shasum -a 256 "${DMG_PATH}"
