#!/bin/sh
# Import the Developer ID certificate into a keychain of the release job's own, so codesign (packaging/sign-macos.sh,
# and Tauri's bundler for the app) finds the identity. The keychain lives in the job's temporary folder and goes with
# the runner.
#
# Usage: packaging/keychain-macos.sh
# Environment: CERT_P12 (the certificate and its key, a .p12 in base64), CERT_PASSWORD, RUNNER_TEMP.
set -eu

: "${CERT_P12:?CERT_P12: the Developer ID certificate, base64}"
: "${CERT_PASSWORD:?CERT_PASSWORD}"
: "${RUNNER_TEMP:?RUNNER_TEMP}"

keychain="$RUNNER_TEMP/signing.keychain-db"
password="$(uuidgen)"
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
printf '%s' "$CERT_P12" | base64 --decode > "$RUNNER_TEMP/cert.p12"
security import "$RUNNER_TEMP/cert.p12" -k "$keychain" -P "$CERT_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain"
security list-keychains -d user -s "$keychain" login.keychain-db
rm "$RUNNER_TEMP/cert.p12"
