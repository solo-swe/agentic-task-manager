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
file="atm-app-$version-universal.dmg"

sha=""
while read -r sum name; do
  if [ "$name" = "$file" ]; then sha=$sum; fi
done < "$sums"
if [ -z "$sha" ]; then
  echo "homebrew-cask.sh: $file isn't in $sums" >&2
  exit 1
fi

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
