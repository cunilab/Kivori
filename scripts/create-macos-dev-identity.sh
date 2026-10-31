#!/usr/bin/env bash
# Creates the self-signed code-signing identity "Kivori Dev" in your login keychain, once, so local
# builds (`just bundle-macos`) are signed the same way every time and keep their Accessibility
# approval. Safe to rerun: it does nothing when the identity already exists.
#
#   scripts/create-macos-dev-identity.sh
#
# The certificate is local to your Mac and trusted by nobody else; it is not a release identity.
# Env: KIVORI_DEV_IDENTITY_NAME (default "Kivori Dev"), KIVORI_DEV_IDENTITY_KEYCHAIN (default: the
# login keychain; used by tests). Nothing secret is printed, and the temporary files are removed.
set -euo pipefail

[ "$(uname -s)" = Darwin ] || { echo "Run this on a Mac." >&2; exit 1; }
name="${KIVORI_DEV_IDENTITY_NAME:-Kivori Dev}"
keychain="${KIVORI_DEV_IDENTITY_KEYCHAIN:-$HOME/Library/Keychains/login.keychain-db}"

has_identity() {
  local found
  found="$(security find-identity -p codesigning "$keychain" 2>/dev/null)" || return 1
  grep -Fq "\"$name\"" <<<"$found"
}

if has_identity; then
  echo "'$name' already exists; nothing to do."
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
chmod 700 "$work"

cat > "$work/openssl.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $name
[ext]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
CNF

openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -config "$work/openssl.cnf" \
  -keyout "$work/key.pem" -out "$work/cert.pem" 2>/dev/null
# A random one-off password; the legacy cipher settings are what macOS `security import` reads.
export P12_PASSWORD
P12_PASSWORD="$(openssl rand -hex 16)"
openssl pkcs12 -export -inkey "$work/key.pem" -in "$work/cert.pem" -name "$name" \
  -certpbe PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1 \
  -passout env:P12_PASSWORD -out "$work/identity.p12"
security import "$work/identity.p12" -k "$keychain" -P "$P12_PASSWORD" -T /usr/bin/codesign >/dev/null

if has_identity; then
  echo "Created '$name'. macOS may list it as untrusted; that is fine for signing."
else
  echo "error: '$name' was not found after the import" >&2
  exit 1
fi
