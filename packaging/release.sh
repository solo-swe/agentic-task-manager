#!/bin/sh
# Start a release of the atm CLI: check that the version is the one the repository says, then tag it. Pushing the tag
# (`--push`, or by hand) runs .github/workflows/release.yml, which builds, signs and drafts the GitHub release;
# publishing that draft updates the Homebrew formula (.github/workflows/homebrew.yml). packaging/README.md has it all.
#
# Usage: packaging/release.sh <version> [--push]     run from the repository's root, on main, after the release PR
set -eu

version=${1:?usage: release.sh <version> [--push]}
push=${2:-}
tag="v$version"

fail() {
  echo "release.sh: $1" >&2
  exit 1
}

case "$version" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) fail "$version isn't a version (MAJOR.MINOR.PATCH)" ;;
esac
[ -z "$push" ] || [ "$push" = "--push" ] || fail "unknown option $push (only --push)"

# atm's version: the workspace's
cargo_version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ "$cargo_version" = "$version" ] || fail "Cargo.toml says $cargo_version, not $version"

branch=$(git rev-parse --abbrev-ref HEAD)
[ "$branch" = "main" ] || fail "releases are tagged on main (this is $branch)"
[ -z "$(git status --porcelain)" ] || fail "the working tree has changes: commit or stash them first"
git fetch --quiet origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || fail "main isn't origin/main: pull (or push) first"
if git rev-parse --quiet --verify "refs/tags/$tag" > /dev/null; then fail "$tag exists already"; fi

git tag -a "$tag" -m "atm $version"
echo "Tagged $tag."
if [ "$push" = "--push" ]; then
  git push origin "$tag"
  echo "Pushed $tag: the Release workflow builds it and drafts the release."
else
  echo "Push it to build the release: git push origin $tag"
fi
