#!/usr/bin/env bash
# Issues one licence key by hand, for testing and for support cases. The
# purchase flow will do the same thing automatically (M4). Run by the owner:
# it reads the private key from the file you name and prints only the licence.
#
# Usage:
#   LICENCE_PRIVATE_KEY_FILE=/somewhere/safe/licence-private.pem \
#   scripts/issue-licence.sh buyer@example.com standard
#
# The key is `<payload>.<signature>`, both URL-safe base64. The payload is
# {"email":...,"plan":...,"issued":"YYYY-MM-DD"}, exactly as the app reads it
# in apps/desktop/src-tauri/src/licence.rs.
set -euo pipefail

die() { printf 'error: %s\n' "$*" >&2; exit 1; }

OPENSSL="${OPENSSL:-$(brew --prefix openssl@3 2>/dev/null)/bin/openssl}"
[[ -x "${OPENSSL}" ]] || die "OpenSSL 3 was not found. Install it, or set OPENSSL to its path."
"${OPENSSL}" version | grep -q '^OpenSSL 3' || die "${OPENSSL} is not OpenSSL 3"

[[ $# -eq 2 ]] || die "usage: issue-licence.sh <email> <plan>"
EMAIL="$1"
PLAN="$2"
# Kept strict so the values can go into JSON as they are.
[[ "${EMAIL}" =~ ^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$ ]] || die "that does not look like an email address"
[[ "${PLAN}" =~ ^[A-Za-z0-9_-]{1,32}$ ]] || die "a plan is letters, digits, - and _ only"
[[ -n "${LICENCE_PRIVATE_KEY_FILE:-}" ]] || die "set LICENCE_PRIVATE_KEY_FILE to the private key's path"
[[ -r "${LICENCE_PRIVATE_KEY_FILE}" ]] || die "cannot read ${LICENCE_PRIVATE_KEY_FILE}"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/licence.XXXXXX")"
trap 'rm -rf "${WORK}"' EXIT

printf '{"email":"%s","plan":"%s","issued":"%s"}' "${EMAIL}" "${PLAN}" "$(date +%Y-%m-%d)" >"${WORK}/payload"
"${OPENSSL}" pkeyutl -sign -inkey "${LICENCE_PRIVATE_KEY_FILE}" -rawin -in "${WORK}/payload" -out "${WORK}/signature"

b64url() { "${OPENSSL}" base64 -A | tr '+/' '-_' | tr -d '='; }
printf '%s.%s\n' "$(b64url <"${WORK}/payload")" "$(b64url <"${WORK}/signature")"
