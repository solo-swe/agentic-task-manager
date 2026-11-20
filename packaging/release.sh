#!/bin/sh
# Start a release of the atm CLI: tag the merged main as it is on GitHub (origin/main), once its Cargo.toml has the
# version. Pushing the tag (`--push`, or by hand) runs .github/workflows/release.yml, which builds, signs and drafts the
# GitHub release; publishing that draft updates the Homebrew formula (.github/workflows/homebrew.yml).
# packaging/README.md has it all.
#
# It tags origin/main's commit, never HEAD (ATM-705): the branch checked out and the working tree don't change what is
# tagged, so it runs from any of them (a release branch, with work in progress).
#
# Usage: packaging/release.sh <version> [--push]     run in the repository, after the release PR is merged
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

git fetch --quiet origin main
commit=$(git rev-parse origin/main)

# atm's version on main: the workspace's, in the commit to tag
main_version=$(git show "$commit:Cargo.toml" | sed -n 's/^version = "\(.*\)"$/\1/p' | head -n 1)
[ "$main_version" = "$version" ] || fail "origin/main's Cargo.toml says $main_version, not $version: merge the release PR (or push main) first"

if git rev-parse --quiet --verify "refs/tags/$tag" > /dev/null; then fail "$tag exists already"; fi
[ -z "$(git ls-remote --tags origin "refs/tags/$tag")" ] || fail "$tag exists already on origin"

git tag -a "$tag" -m "atm $version" "$commit"
echo "Tagged $tag: origin/main, $(git log -1 --format='%h %s' "$commit")."
# On a local main with commits not pushed yet: they aren't in the release, and are left as they are
if [ "$(git rev-parse --abbrev-ref HEAD)" = "main" ]; then
  ahead=$(git rev-list --count "$commit..HEAD")
  [ "$ahead" = "0" ] || echo "Not in it: main's $ahead commit(s) not pushed to origin yet."
fi
if [ "$push" = "--push" ]; then
  git push origin "$tag"
  echo "Pushed $tag: the Release workflow builds it and drafts the release."
else
  echo "Push it to build the release: git push origin $tag"
fi
