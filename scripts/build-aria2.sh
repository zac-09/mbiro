#!/usr/bin/env bash
# Builds a universal (arm64 + x86_64) aria2c that links only against libraries
# shipped with macOS, and places it where Tauri expects a sidecar.
#
# aria2 is GPL-2.0-or-later. It is shipped as a separate executable and never
# linked into our binaries. The exact source is the pinned tarball below, built
# unmodified with the configure flags in this file.
#
# Usage: scripts/build-aria2.sh
# Needs: Xcode Command Line Tools (clang, make, lipo). No Homebrew packages.
set -euo pipefail

ARIA2_VERSION="1.37.0"
ARIA2_SHA256="60a420ad7085eb616cb6e2bdf0a7206d68ff3d37fb5a956dc44242eb2f79b66b"
ARIA2_URL="https://github.com/aria2/aria2/releases/download/release-${ARIA2_VERSION}/aria2-${ARIA2_VERSION}.tar.xz"

# Oldest macOS the binary will run on. Keep in step with
# bundle.macOS.minimumSystemVersion in apps/desktop/src-tauri/tauri.conf.json.
export MACOSX_DEPLOYMENT_TARGET="11.0"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="${ROOT}/.cache/aria2-${ARIA2_VERSION}"
OUT="${ROOT}/apps/desktop/src-tauri/binaries"
TARBALL="${WORK}/aria2-${ARIA2_VERSION}.tar.xz"
JOBS="$(sysctl -n hw.ncpu)"
SDK="$(xcrun --show-sdk-path)"

mkdir -p "${WORK}" "${OUT}"

if [[ ! -f "${TARBALL}" ]]; then
  echo "==> Fetching aria2 ${ARIA2_VERSION}"
  curl --proto '=https' --tlsv1.2 -fsSL -o "${TARBALL}.part" "${ARIA2_URL}"
  mv "${TARBALL}.part" "${TARBALL}"
fi

echo "==> Verifying checksum"
echo "${ARIA2_SHA256}  ${TARBALL}" | shasum -a 256 -c -

build_arch() {
  local arch="$1" host="$2"
  local src="${WORK}/src-${arch}"

  echo "==> Building aria2c for ${arch}"
  rm -rf "${src}"
  mkdir -p "${src}"
  tar -xJf "${TARBALL}" -C "${src}" --strip-components=1

  (
    cd "${src}"
    # TLS and hashing come from the system (AppleTLS, CommonCrypto), zlib from
    # the SDK. Everything that would pull in a non-system library is off.
    # BitTorrent and Metalink are compiled out: they are not in v1.
    # --with-ca-bundle is irrelevant: AppleTLS uses the system trust store.
    ./configure \
      --build="$(uname -m | sed 's/arm64/aarch64/')-apple-darwin" \
      --host="${host}" \
      --disable-dependency-tracking \
      --disable-nls \
      --disable-bittorrent \
      --disable-metalink \
      --enable-websocket \
      --with-appletls \
      --without-gnutls \
      --without-openssl \
      --without-libnettle \
      --without-libgcrypt \
      --without-libgmp \
      --without-libxml2 \
      --without-libexpat \
      --without-sqlite3 \
      --without-libssh2 \
      --without-libcares \
      --without-libuv \
      --with-libz \
      CC="clang" CXX="clang++" \
      CFLAGS="-arch ${arch} -O2 -isysroot ${SDK}" \
      CXXFLAGS="-arch ${arch} -O2 -isysroot ${SDK} -std=c++14" \
      LDFLAGS="-arch ${arch} -isysroot ${SDK}" \
      >"${WORK}/configure-${arch}.log" 2>&1 \
      || { tail -40 "${WORK}/configure-${arch}.log"; exit 1; }

    make -j"${JOBS}" >"${WORK}/make-${arch}.log" 2>&1 \
      || { tail -60 "${WORK}/make-${arch}.log"; exit 1; }
  )

  strip -x -o "${WORK}/aria2c-${arch}" "${src}/src/aria2c"
}

build_arch arm64 aarch64-apple-darwin
build_arch x86_64 x86_64-apple-darwin

echo "==> Joining slices"
UNIVERSAL="${OUT}/aria2c-universal-apple-darwin"
lipo -create -output "${UNIVERSAL}" "${WORK}/aria2c-arm64" "${WORK}/aria2c-x86_64"

# Tauri resolves a sidecar by target triple. The same universal file serves
# single-architecture dev builds too.
cp -f "${UNIVERSAL}" "${OUT}/aria2c-aarch64-apple-darwin"
cp -f "${UNIVERSAL}" "${OUT}/aria2c-x86_64-apple-darwin"

echo "==> Checking the result"
lipo -info "${UNIVERSAL}"

# Fail if anything outside the OS is linked.
if otool -L "${UNIVERSAL}" | grep -E '^\s+/' | grep -vE '^\s+(/usr/lib/|/System/Library/)'; then
  echo "error: aria2c links a library that does not ship with macOS" >&2
  exit 1
fi
otool -L "${UNIVERSAL}"

# Run whichever slice this machine can execute.
"${UNIVERSAL}" --version | head -12

echo "==> Done: ${UNIVERSAL}"
