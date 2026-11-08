#!/bin/sh
# The Homebrew cask for the atm app (T-460), printed for a release: the universal DMG from the release's assets, with
# its checksum from the release's SHA256SUMS. The app brings its own atm CLI and puts it on PATH (the same build as the
# app, so they never drift apart), so it conflicts with the atm formula: one or the other (Hakan, T-460). After
# installing it only says how to connect agents; it installs nothing into them. The Homebrew workflow
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
  name "atm"
  desc "Agentic Task Manager: file-based tasks for agentic development, and their dashboard"
  homepage "https://github.com/solo-swe/agentic-task-manager"

  depends_on macos: ">= :big_sur"
  # The app's own atm CLI goes on PATH: the formula's would be a second one
  conflicts_with formula: "solo-swe/tap/atm"

  app "atm.app"
  binary "#{appdir}/atm.app/Contents/MacOS/atm"

  caveats <<~CAVEATS
    Run \`atm agents install\` to let your AI agents use atm.
  CAVEATS
end
EOF
