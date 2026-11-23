#!/usr/bin/env bash
# Builds the universal app, signs every executable in it with the Developer ID
# and the hardened runtime, notarises it, and produces a stapled DMG. Given the
# updater key, it also produces the signed update and its manifest.
#
# The owner runs this. It never sees a secret: the certificate's private key
# stays in the login keychain, and the notarisation credentials stay in a
# keychain profile that `notarytool` reads by name.
#
# One-time setup, done by hand:
#   1. Install the "Developer ID Application" certificate in the login keychain.
#   2. Store notarisation credentials under a profile name of your choice:
#        xcrun notarytool store-credentials "<profile>"
#      (it asks for the credentials itself; nothing is passed to this script).
#
# Usage:
#   SIGNING_IDENTITY="Developer ID Application: <name> (<TEAMID>)" \
#   NOTARY_PROFILE="<profile>" \
#   scripts/release.sh
#
# To make the update as well, add (see "Updates" below):
#   TAURI_SIGNING_PRIVATE_KEY_PATH=/somewhere/safe/updater.key \
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD="<the key's password, or empty>" \
#   UPDATE_BASE_URL="https://<where the update files will be served from>"
#
# Rehearsal without an Apple account (ad-hoc signature, no notarisation; the
# result only runs on this Mac):
#   SIGNING_IDENTITY=- SKIP_NOTARIZE=1 scripts/release.sh
#
# Updates. One-time setup, done by hand, outside the repository:
#   pnpm --filter @mbiro/desktop tauri signer generate -w /somewhere/safe/updater.key
# Paste the contents of the .pub file it writes into PUBLIC_KEY in
# apps/desktop/src-tauri/src/update.rs, and the manifest's address into
# MANIFEST_URL there. Keep the key file safe and backed up: if it is lost,
# installed copies can never be updated again.
#
# With the key given, this script leaves two more files in the output folder:
# the update archive and latest.json. Upload the archive first, then
# latest.json, to UPDATE_BASE_URL. The app starts offering the update as soon
# as latest.json is there. This script uploads nothing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="universal-apple-darwin"
BUNDLE_DIR="${ROOT}/target/${TARGET}/release/bundle/macos"
OUT_DIR="${ROOT}/target/release-artifacts"
SKIP_NOTARIZE="${SKIP_NOTARIZE:-0}"
# Must match TARGET in apps/desktop/src-tauri/src/update.rs.
UPDATE_TARGET="darwin-universal"
MAKE_UPDATE=0
[[ -n "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]] && MAKE_UPDATE=1

say() { printf '\n==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------- preflight
[[ "$(uname -s)" == "Darwin" ]] || die "this script runs on macOS only"
[[ -n "${SIGNING_IDENTITY:-}" ]] || die "set SIGNING_IDENTITY (see the header of this file)"
if [[ "${SKIP_NOTARIZE}" != "1" ]]; then
  [[ -n "${NOTARY_PROFILE:-}" ]] || die "set NOTARY_PROFILE, or SKIP_NOTARIZE=1 for a rehearsal"
  [[ "${SIGNING_IDENTITY}" != "-" ]] || die "an ad-hoc signature cannot be notarised"
fi
if [[ "${MAKE_UPDATE}" == "1" ]]; then
  [[ -r "${TAURI_SIGNING_PRIVATE_KEY_PATH}" ]] || die "cannot read TAURI_SIGNING_PRIVATE_KEY_PATH"
  [[ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+set}" ]] \
    || die "set TAURI_SIGNING_PRIVATE_KEY_PASSWORD (empty if the key has none)"
  [[ "${UPDATE_BASE_URL:-}" == https://* ]] || die "set UPDATE_BASE_URL to an https:// address"
fi
for tool in codesign hdiutil ditto lipo spctl plutil pnpm node tar; do
  command -v "${tool}" >/dev/null || die "${tool} was not found"
done
xcrun --find notarytool >/dev/null 2>&1 || die "notarytool was not found (install Xcode Command Line Tools)"
xcrun --find stapler >/dev/null 2>&1 || die "stapler was not found (install Xcode Command Line Tools)"

SIDECAR="${ROOT}/apps/desktop/src-tauri/binaries/aria2c-${TARGET}"
[[ -x "${SIDECAR}" ]] || die "aria2c is missing: run scripts/build-aria2.sh first"
"${ROOT}/scripts/build-nm-host.sh"

if [[ "${SIGNING_IDENTITY}" != "-" ]]; then
  security find-identity -v -p codesigning | grep -qF "${SIGNING_IDENTITY}" \
    || die "no valid code-signing identity matching SIGNING_IDENTITY is in the keychain"
fi
if [[ "${SKIP_NOTARIZE}" != "1" ]]; then
  say "Checking the notarisation profile"
  xcrun notarytool history --keychain-profile "${NOTARY_PROFILE}" >/dev/null \
    || die "notarytool could not use the profile \"${NOTARY_PROFILE}\""
fi

# A real signature carries a secure timestamp; an ad-hoc one cannot.
if [[ "${SIGNING_IDENTITY}" == "-" ]]; then
  TIMESTAMP=(--timestamp=none)
else
  TIMESTAMP=(--timestamp)
fi

# -------------------------------------------------------------------- build
say "Building the universal app"
# Tauri signs and notarises by itself when it finds these. This script does
# both explicitly instead, so every step is visible and checked.
(
  cd "${ROOT}"
  env -u APPLE_SIGNING_IDENTITY -u APPLE_CERTIFICATE -u APPLE_CERTIFICATE_PASSWORD \
    -u APPLE_API_KEY -u APPLE_API_ISSUER -u APPLE_API_KEY_PATH \
    -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID \
    pnpm --filter @mbiro/desktop tauri build --target "${TARGET}" --bundles app
)

shopt -s nullglob
APPS=("${BUNDLE_DIR}"/*.app)
shopt -u nullglob
[[ ${#APPS[@]} -eq 1 ]] || die "expected exactly one .app in ${BUNDLE_DIR}"
APP="${APPS[0]}"
PLIST="${APP}/Contents/Info.plist"
NAME="$(plutil -extract CFBundleName raw -o - "${PLIST}")"
VERSION="$(plutil -extract CFBundleShortVersionString raw -o - "${PLIST}")"
BUNDLE_ID="$(plutil -extract CFBundleIdentifier raw -o - "${PLIST}")"
MAIN_EXE="$(plutil -extract CFBundleExecutable raw -o - "${PLIST}")"
DMG="${OUT_DIR}/${NAME}-${VERSION}-universal.dmg"

# --------------------------------------------------------------------- sign
say "Signing ${NAME} ${VERSION} (${BUNDLE_ID})"
# Inside out: helper executables first, then the bundle, which seals them.
# No entitlements: neither the app nor aria2c needs a hardened-runtime exception.
for exe in "${APP}/Contents/MacOS"/*; do
  [[ "$(basename "${exe}")" == "${MAIN_EXE}" ]] && continue
  codesign --force --options runtime "${TIMESTAMP[@]}" \
    --identifier "${BUNDLE_ID}.$(basename "${exe}")" \
    --sign "${SIGNING_IDENTITY}" "${exe}"
done
codesign --force --options runtime "${TIMESTAMP[@]}" --sign "${SIGNING_IDENTITY}" "${APP}"

say "Checking signatures"
codesign --verify --deep --strict --verbose=2 "${APP}"
for exe in "${APP}/Contents/MacOS"/*; do
  info="$(codesign --display --verbose=4 "${exe}" 2>&1)"
  archs="$(lipo -archs "${exe}")"
  [[ "${archs}" == *arm64* && "${archs}" == *x86_64* ]] \
    || die "$(basename "${exe}") is not universal (${archs})"
  grep -q "flags=.*runtime" <<<"${info}" \
    || die "$(basename "${exe}") is not signed with the hardened runtime"
  if [[ "${SIGNING_IDENTITY}" != "-" ]]; then
    grep -q "^Authority=Developer ID Application" <<<"${info}" \
      || die "$(basename "${exe}") is not signed with a Developer ID Application certificate"
    grep -q "^Timestamp=" <<<"${info}" \
      || die "$(basename "${exe}") has no secure timestamp"
  fi
  echo "ok: $(basename "${exe}") (${archs})"
done

# ----------------------------------------------------------------- notarise
WORK="$(mktemp -d "${TMPDIR:-/tmp}/release.XXXXXX")"
trap 'rm -rf "${WORK}"' EXIT

# Submits one file and waits. Prints Apple's log and stops if it is not accepted.
notarise() {
  local file="$1" result="${WORK}/notary.json" status id
  xcrun notarytool submit "${file}" --keychain-profile "${NOTARY_PROFILE}" \
    --wait --output-format json >"${result}" || true
  status="$(plutil -extract status raw -o - "${result}" 2>/dev/null || echo unknown)"
  id="$(plutil -extract id raw -o - "${result}" 2>/dev/null || echo "")"
  echo "notarisation ${status} (submission ${id:-unknown})"
  if [[ "${status}" != "Accepted" ]]; then
    [[ -n "${id}" ]] && xcrun notarytool log "${id}" --keychain-profile "${NOTARY_PROFILE}" || true
    die "Apple did not accept $(basename "${file}")"
  fi
}

if [[ "${SKIP_NOTARIZE}" != "1" ]]; then
  say "Notarising the app"
  ditto -c -k --keepParent "${APP}" "${WORK}/app.zip"
  notarise "${WORK}/app.zip"
  # The ticket goes on the app too, so a copy dragged out of the DMG opens offline.
  xcrun stapler staple "${APP}"
fi

# ---------------------------------------------------------------------- dmg
say "Building the DMG"
mkdir -p "${OUT_DIR}" "${WORK}/dmg"
ditto "${APP}" "${WORK}/dmg/$(basename "${APP}")"
ln -s /Applications "${WORK}/dmg/Applications"
hdiutil create -volname "${NAME}" -srcfolder "${WORK}/dmg" -fs HFS+ -format UDZO -ov "${DMG}" >/dev/null
codesign --force "${TIMESTAMP[@]}" --sign "${SIGNING_IDENTITY}" "${DMG}"

if [[ "${SKIP_NOTARIZE}" != "1" ]]; then
  say "Notarising the DMG"
  notarise "${DMG}"
  xcrun stapler staple "${DMG}"
  xcrun stapler validate "${DMG}"

  say "Asking Gatekeeper"
  spctl --assess --type execute --verbose=2 "${APP}"
  spctl --assess --type open --context context:primary-signature --verbose=2 "${DMG}"
fi

# ------------------------------------------------------------------- update
# Made from the app as it is now: signed, and stapled if it was notarised, so
# an updated copy is exactly what a fresh install from the DMG would be.
if [[ "${MAKE_UPDATE}" == "1" ]]; then
  say "Making the update"
  ARCHIVE_NAME="${NAME}-${VERSION}-universal.app.tar.gz"
  ARCHIVE="${OUT_DIR}/${ARCHIVE_NAME}"
  rm -f "${ARCHIVE}" "${ARCHIVE}.sig"
  # COPYFILE_DISABLE keeps macOS's ._ companion files out of the archive.
  COPYFILE_DISABLE=1 tar -czf "${ARCHIVE}" -C "$(dirname "${APP}")" "$(basename "${APP}")"
  # The signer reads the key's path and password from the environment. The
  # version goes inside the signature, so the manifest cannot pass an old
  # release off as a new one.
  (cd "${ROOT}" && pnpm --filter @mbiro/desktop tauri signer sign --app-version "${VERSION}" "${ARCHIVE}" >/dev/null)
  [[ -s "${ARCHIVE}.sig" ]] || die "the update was not signed"
  node -e '
    const [signatureFile, out, version, target, url] = process.argv.slice(1);
    const fs = require("node:fs");
    const manifest = {
      version,
      notes: process.env.UPDATE_NOTES ?? "",
      pub_date: new Date().toISOString(),
      platforms: { [target]: { signature: fs.readFileSync(signatureFile, "utf8").trim(), url } },
    };
    fs.writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
  ' "${ARCHIVE}.sig" "${OUT_DIR}/latest.json" "${VERSION}" "${UPDATE_TARGET}" \
    "${UPDATE_BASE_URL%/}/${ARCHIVE_NAME// /%20}"
  rm -f "${ARCHIVE}.sig"
fi

say "Done"
echo "${DMG}"
shasum -a 256 "${DMG}"
if [[ "${MAKE_UPDATE}" == "1" ]]; then
  echo "${ARCHIVE}"
  echo "${OUT_DIR}/latest.json"
  echo "Upload the archive, then latest.json, to ${UPDATE_BASE_URL%/}/"
else
  echo "No update was made: TAURI_SIGNING_PRIVATE_KEY_PATH is not set."
fi
if [[ "${SKIP_NOTARIZE}" == "1" ]]; then
  echo "Rehearsal only: this build is not notarised and will be blocked on any other Mac."
fi
