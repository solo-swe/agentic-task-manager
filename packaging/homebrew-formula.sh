#!/bin/sh
# The Homebrew formula for the atm CLI (docs/better-way-to-distribute-atm.md §1), printed for a release: one binary
# per platform, from the release's assets, each with its checksum from the release's SHA256SUMS. The Homebrew workflow
# (.github/workflows/homebrew.yml) writes it to Formula/atm.rb in solo-swe/homebrew-tap.
#
# Usage: packaging/homebrew-formula.sh <version> <SHA256SUMS file>
set -eu

version=${1:?usage: homebrew-formula.sh <version> <SHA256SUMS file>}
sums=${2:?usage: homebrew-formula.sh <version> <SHA256SUMS file>}

# Both go into Ruby as they are (ATM-678, audit A12): a version is three numbers, a checksum 64 hex digits, nothing else
fail() {
  echo "homebrew-formula.sh: $1" >&2
  exit 1
}
dots=${version#*.}
case $version in
  *[!0123456789.]* | *..* | .* | *.) fail "the version $version isn't MAJOR.MINOR.PATCH" ;;
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) fail "the version $version isn't MAJOR.MINOR.PATCH" ;;
esac
case ${dots#*.} in *.*) fail "the version $version isn't MAJOR.MINOR.PATCH" ;; esac
base="https://github.com/solo-swe/agentic-task-manager/releases/download/v$version"

# The checksum of a target's archive, from SHA256SUMS ("<sha256>  <file>" lines); fails when it isn't listed, or isn't
# a SHA-256
sha() {
  file="atm-$version-$1.tar.gz"
  found=""
  while read -r sum name; do
    if [ "$name" = "$file" ]; then found=$sum; fi
  done < "$sums"
  if [ -z "$found" ]; then
    fail "$file isn't in $sums"
  fi
  # The characters listed: a range like a-f follows the locale's collation, which can take in A-F
  case $found in *[!0123456789abcdef]*) fail "$file's checksum in $sums isn't a SHA-256" ;; esac
  if [ ${#found} -ne 64 ]; then fail "$file's checksum in $sums isn't a SHA-256"; fi
  printf '%s' "$found"
}

mac_arm=$(sha aarch64-apple-darwin)
mac_intel=$(sha x86_64-apple-darwin)
linux_arm=$(sha aarch64-unknown-linux-gnu)
linux_intel=$(sha x86_64-unknown-linux-gnu)

cat <<EOF
# Written by agentic-task-manager's release (packaging/homebrew-formula.sh): don't edit it by hand.
class Atm < Formula
  desc "Agentic Task Manager: file-based tasks for agentic development, and their dashboard"
  homepage "https://github.com/solo-swe/agentic-task-manager"
  version "$version"
  license "MIT"

  on_macos do
    on_arm do
      url "$base/atm-$version-aarch64-apple-darwin.tar.gz"
      sha256 "$mac_arm"
    end
    on_intel do
      url "$base/atm-$version-x86_64-apple-darwin.tar.gz"
      sha256 "$mac_intel"
    end
  end

  on_linux do
    on_arm do
      url "$base/atm-$version-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "$linux_arm"
    end
    on_intel do
      url "$base/atm-$version-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "$linux_intel"
    end
  end

  def install
    bin.install "atm"
  end

  test do
    assert_equal version.to_s, shell_output("#{bin}/atm version").strip
  end
end
EOF
