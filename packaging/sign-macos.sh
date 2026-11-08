#!/bin/sh
# Sign a macOS build of atm with the Developer ID and notarize it (docs/better-way-to-distribute-atm.md §1). The
# release workflow imports the certificate into a keychain of its own first. A bare binary can't have its ticket
# stapled: Gatekeeper finds the notarization online the first time it runs.
#
# Usage: packaging/sign-macos.sh <binary>
# Environment: IDENTITY (the signing identity, "Developer ID Application"), NOTARY_KEY (the App Store Connect API
# key's .p8 file), NOTARY_KEY_ID, NOTARY_ISSUER.
set -eu

binary=${1:?usage: sign-macos.sh <binary>}
: "${IDENTITY:?IDENTITY: the signing identity}"
: "${NOTARY_KEY:?NOTARY_KEY: the notary API key file}"
: "${NOTARY_KEY_ID:?NOTARY_KEY_ID}"
: "${NOTARY_ISSUER:?NOTARY_ISSUER}"

codesign --force --options runtime --timestamp --sign "$IDENTITY" "$binary"
codesign --verify --strict --verbose=2 "$binary"

# notarytool takes an archive: a zip of the signed binary
submission="$(dirname "$binary")/atm-notarize.zip"
ditto -c -k --keepParent "$binary" "$submission"
result=$(xcrun notarytool submit "$submission" --key "$NOTARY_KEY" --key-id "$NOTARY_KEY_ID" --issuer "$NOTARY_ISSUER" --wait)
rm "$submission"
printf '%s\n' "$result"
# Not every notarytool exits with an error when Apple rejects a submission: its status says
case "$result" in
  *"status: Accepted"*) ;;
  *) echo "sign-macos.sh: notarization wasn't accepted" >&2; exit 1 ;;
esac
