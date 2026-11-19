#!/usr/bin/env bash
# Makes the Ed25519 key pair that licence keys are signed with. Run once, by
# the owner. The private key is written to a file you name and is never
# printed; only the public key is shown, to paste into the app.
#
# Usage:
#   LICENCE_PRIVATE_KEY_FILE=/somewhere/safe/licence-private.pem scripts/licence-keygen.sh
#
# Then paste the printed line into PUBLIC_KEY in
# apps/desktop/src-tauri/src/licence.rs, as Some("...").
#
# Keep the private key file out of the repository and backed up: whoever holds
# it can issue licences, and if it is lost no new licence can be issued for
# builds that carry its public half.
#
# Needs OpenSSL 3 (macOS's own `openssl` is LibreSSL and cannot do this):
#   brew install openssl@3
set -euo pipefail

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

OPENSSL="${OPENSSL:-$(brew --prefix openssl@3 2>/dev/null)/bin/openssl}"
[[ -x "${OPENSSL}" ]] || die "OpenSSL 3 was not found. Install it, or set OPENSSL to its path."
"${OPENSSL}" version | grep -q '^OpenSSL 3' || die "${OPENSSL} is not OpenSSL 3"

[[ -n "${LICENCE_PRIVATE_KEY_FILE:-}" ]] || die "set LICENCE_PRIVATE_KEY_FILE to where the private key should be written"
[[ ! -e "${LICENCE_PRIVATE_KEY_FILE}" ]] || die "${LICENCE_PRIVATE_KEY_FILE} already exists; refusing to replace a key"

umask 077
"${OPENSSL}" genpkey -algorithm ed25519 -out "${LICENCE_PRIVATE_KEY_FILE}"

echo "Private key written to ${LICENCE_PRIVATE_KEY_FILE} (readable only by you)."
echo "Public key, for PUBLIC_KEY in apps/desktop/src-tauri/src/licence.rs:"
# The last 32 bytes of the DER form are the raw public key.
"${OPENSSL}" pkey -in "${LICENCE_PRIVATE_KEY_FILE}" -pubout -outform DER | tail -c 32 | "${OPENSSL}" base64 -A
echo
