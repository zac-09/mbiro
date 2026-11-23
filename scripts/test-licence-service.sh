#!/usr/bin/env bash
# Checks the app and the licence service against each other, on this machine.
#
# It starts services/licence with a key pair made for this run and thrown away
# afterwards, buys two licences through the stand-in payment adapter, refunds
# one, and runs the app's activation test against the result. No real key is
# read or needed.
#
# Usage: scripts/test-licence-service.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/licence-test.XXXXXX")"
PORT="${PORT:-8791}"
SERVER="http://127.0.0.1:${PORT}"
SECRET="only-for-this-test-run"
SERVICE_PID=""

cleanup() {
  [[ -n "${SERVICE_PID}" ]] && kill "${SERVICE_PID}" 2>/dev/null || true
  rm -rf "${WORK}"
}
trap cleanup EXIT
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

node -e '
  const { generateKeyPairSync } = require("node:crypto");
  const { privateKey } = generateKeyPairSync("ed25519");
  require("node:fs").writeFileSync(process.argv[1], privateKey.export({ format: "pem", type: "pkcs8" }), { mode: 0o600 });
' "${WORK}/throwaway.pem"

(
  cd "${ROOT}/services/licence"
  LICENCE_PRIVATE_KEY_FILE="${WORK}/throwaway.pem" STUB_PAYMENT_SECRET="${SECRET}" PORT="${PORT}" \
    exec node_modules/.bin/tsx src/local.ts
) >"${WORK}/service.log" 2>&1 &
SERVICE_PID=$!

for _ in $(seq 1 50); do
  grep -q "^Public key for the app:" "${WORK}/service.log" 2>/dev/null && break
  kill -0 "${SERVICE_PID}" 2>/dev/null || { cat "${WORK}/service.log" >&2; die "the service did not start"; }
  sleep 0.2
done
PUBLIC_KEY="$(sed -n 's/^Public key for the app: //p' "${WORK}/service.log")"
[[ -n "${PUBLIC_KEY}" ]] || die "the service did not print its public key"

# Sends one event to the stand-in payment adapter, signed as it expects.
webhook() {
  local body="$1" signature
  signature="$(printf '%s' "${body}" | openssl dgst -sha256 -hmac "${SECRET}" | awk '{print $NF}')"
  curl -fsS -X POST "${SERVER}/v1/payments/stub" \
    -H "content-type: application/json" -H "x-stub-signature: ${signature}" \
    --data-binary "${body}" >/dev/null
}
# The key in the most recent licence email the service printed.
last_key() { grep -E '^[A-Za-z0-9_-]{40,}\.[A-Za-z0-9_-]{80,}$' "${WORK}/service.log" | tail -1; }

webhook '{"kind":"purchase","order":"1","email":"buyer@example.com","plan":"standard"}'
LICENCE="$(last_key)"
webhook '{"kind":"purchase","order":"2","email":"refunded@example.com","plan":"standard"}'
REFUNDED="$(last_key)"
webhook '{"kind":"refund","order":"2"}'
[[ -n "${LICENCE}" && -n "${REFUNDED}" && "${LICENCE}" != "${REFUNDED}" ]] || die "the service did not issue two keys"

export PATH="${HOME}/.cargo/bin:${PATH}"
cd "${ROOT}"
MBIRO_TEST_LICENCE_SERVER="${SERVER}" MBIRO_TEST_PUBLIC_KEY="${PUBLIC_KEY}" \
MBIRO_TEST_LICENCE="${LICENCE}" MBIRO_TEST_REFUNDED_LICENCE="${REFUNDED}" \
  cargo test -p mbiro-desktop --test activation -- --ignored
