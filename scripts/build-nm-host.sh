#!/usr/bin/env bash
# Builds the native messaging host for both architectures, joins them, and
# places the result where Tauri expects a sidecar. Takes a few seconds.
#
# Usage: scripts/build-nm-host.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${ROOT}/apps/desktop/src-tauri/binaries"
# Keep in step with scripts/build-aria2.sh and tauri.conf.json.
export MACOSX_DEPLOYMENT_TARGET="11.0"

cd "${ROOT}"
for triple in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --quiet --release --package mbiro-nm-host --target "${triple}"
done

mkdir -p "${OUT}"
UNIVERSAL="${OUT}/nm-host-universal-apple-darwin"
lipo -create -output "${UNIVERSAL}" \
  "target/aarch64-apple-darwin/release/mbiro-nm-host" \
  "target/x86_64-apple-darwin/release/mbiro-nm-host"
# Tauri resolves a sidecar by target triple; the universal file serves all three.
cp -f "${UNIVERSAL}" "${OUT}/nm-host-aarch64-apple-darwin"
cp -f "${UNIVERSAL}" "${OUT}/nm-host-x86_64-apple-darwin"
echo "nm-host: $(lipo -archs "${UNIVERSAL}")"
