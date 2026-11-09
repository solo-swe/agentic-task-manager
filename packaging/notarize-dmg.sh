#!/bin/sh
# Sign the app's DMG with the Developer ID, notarize it and staple the ticket to it, so it opens without a warning
# offline too (T-460). Tauri's bundler has signed and notarized the app inside it already.
#
# Usage: packaging/notarize-dmg.sh <dmg>
# Environment: IDENTITY (the signing identity, "Developer ID Application"), NOTARY_KEY (the App Store Connect API key's
# .p8 file), NOTARY_KEY_ID, NOTARY_ISSUER.
set -eu

dmg=${1:?usage: notarize-dmg.sh <dmg>}
: "${IDENTITY:?IDENTITY: the signing identity}"
: "${NOTARY_KEY:?NOTARY_KEY: the notary API key file}"
: "${NOTARY_KEY_ID:?NOTARY_KEY_ID}"
: "${NOTARY_ISSUER:?NOTARY_ISSUER}"

codesign --force --timestamp --sign "$IDENTITY" "$dmg"
codesign --verify --strict --verbose=2 "$dmg"
result=$(xcrun notarytool submit "$dmg" --key "$NOTARY_KEY" --key-id "$NOTARY_KEY_ID" --issuer "$NOTARY_ISSUER" --wait)
printf '%s\n' "$result"
# Not every notarytool exits with an error when Apple rejects a submission: its status says
case "$result" in
  *"status: Accepted"*) ;;
  *) echo "notarize-dmg.sh: notarization wasn't accepted" >&2; exit 1 ;;
esac
xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"
