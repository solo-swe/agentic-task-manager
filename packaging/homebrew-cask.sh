#!/bin/sh
# The Homebrew cask for the atm app (T-460), printed for a release: the universal DMG from the release's assets, with
# its checksum from the release's SHA256SUMS. The app brings its own atm CLI and puts it on PATH (the same build as the
# app, so they never drift apart), so it and the atm formula are one or the other (Hakan, T-460). A cask can declare
# a conflict only with another cask (the Cask Cookbook's `conflicts_with`), so its caveats say to uninstall the formula
# first. It says how to connect agents too; it installs nothing into them. The Homebrew workflow
# (.github/workflows/homebrew.yml) writes it to Casks/atm-app.rb in solo-swe/homebrew-tap.
#
# Usage: packaging/homebrew-cask.sh <version> <SHA256SUMS file>
set -eu

version=${1:?usage: homebrew-cask.sh <version> <SHA256SUMS file>}
sums=${2:?usage: homebrew-cask.sh <version> <SHA256SUMS file>}

# Both go into Ruby as they are (ATM-678, audit A12): a version is three numbers, a checksum 64 hex digits, nothing else
fail() {
  echo "homebrew-cask.sh: $1" >&2
  exit 1
}
dots=${version#*.}
case $version in
  *[!0123456789.]* | *..* | .* | *.) fail "the version $version isn't MAJOR.MINOR.PATCH" ;;
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) fail "the version $version isn't MAJOR.MINOR.PATCH" ;;
esac
case ${dots#*.} in *.*) fail "the version $version isn't MAJOR.MINOR.PATCH" ;; esac
file="atm-app-$version-universal.dmg"

sha=""
while read -r sum name; do
  if [ "$name" = "$file" ]; then sha=$sum; fi
done < "$sums"
if [ -z "$sha" ]; then
  fail "$file isn't in $sums"
fi
# The characters listed: a range like a-f follows the locale's collation, which can take in A-F
case $sha in *[!0123456789abcdef]*) fail "$file's checksum in $sums isn't a SHA-256" ;; esac
if [ ${#sha} -ne 64 ]; then fail "$file's checksum in $sums isn't a SHA-256"; fi

cat <<EOF
# Written by agentic-task-manager's release (packaging/homebrew-cask.sh): don't edit it by hand.
cask "atm-app" do
  version "$version"
  sha256 "$sha"

  url "https://github.com/solo-swe/agentic-task-manager/releases/download/v#{version}/atm-app-#{version}-universal.dmg"
  name "Agentic Task Manager"
  desc "Agentic Task Manager: file-based tasks for agentic development, and their dashboard"
  homepage "https://github.com/solo-swe/agentic-task-manager"

  # Big Sur (the app's minimum) is older than any macOS Homebrew runs on
  depends_on :macos

  # The app updates itself (T-483), its atm CLI with it: brew leaves an installed one alone
  auto_updates true

  app "Agentic Task Manager.app"
  # The app's own atm CLI goes on PATH: the formula's would be a second one
  binary "#{appdir}/Agentic Task Manager.app/Contents/MacOS/atm"

  caveats <<~CAVEATS
    The app brings its own atm CLI. If the atm formula is installed, uninstall it
    (\`brew uninstall solo-swe/tap/atm\`) so the atm on your PATH is the app's.

    Run \`atm agents install\` to let your AI agents use atm.
  CAVEATS
end
EOF
