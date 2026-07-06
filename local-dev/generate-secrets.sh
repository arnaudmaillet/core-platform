#!/usr/bin/env bash
# Generates the LOCAL-ONLY signing key the fleet needs, into local-dev/secrets/
# (gitignored). Run this once before `docker compose -f docker-compose.fleet.yml up`.
# The key never leaves your machine — nothing here is committed.
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)/secrets"
mkdir -p "$DIR"

priv="$DIR/auth-signing-private.pem"
pub="$DIR/auth-signing-public.pem"

if [ -f "$priv" ] && [ -f "$pub" ]; then
  echo "auth signing key already present in $DIR (delete to regenerate)"
  exit 0
fi

# ES256 (P-256). genpkey emits a PKCS#8 PEM directly — the format
# jsonwebtoken's EncodingKey::from_ec_pem requires (SEC1 is rejected).
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$priv" 2>/dev/null
openssl ec -in "$priv" -pubout -out "$pub" 2>/dev/null
chmod 600 "$priv"
echo "generated ES256 auth signing keypair in $DIR"
