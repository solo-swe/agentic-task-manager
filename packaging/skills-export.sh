#!/bin/sh
# packaging/skills-export.sh <version> <checkout>: put this version's atm skills into a checkout of solo-swe/skills,
# the public skills repo our apps share (docs/adr/ADR-002-atm-public-skills-repo.md). Only atm's part changes: the
# skills/atm-* folders become this repository's skills/atm-* (the portable core, as written: no Claude Code
# allowed-tools, no /atm: links), and the README's atm section, between its markers, says which version they are.
# Another app's folders and README sections are left as they are. The skills workflow commits, tags and pushes.
set -eu

if [ $# -ne 2 ]; then
  echo "usage: skills-export.sh <version> <checkout of solo-swe/skills>" >&2
  exit 2
fi
version=$1
target=$2
here=$(cd "$(dirname "$0")/.." && pwd)

case "$version" in
  *[!0-9.]* | "" | .* | *.) echo "skills-export.sh: $version isn't a version like 0.33.0" >&2; exit 1 ;;
esac
if [ ! -d "$target/.git" ]; then
  echo "skills-export.sh: $target isn't a git checkout" >&2
  exit 1
fi

mkdir -p "$target/skills"
# atm's skills only: the last version's go, this version's come in
for old in "$target"/skills/atm-*; do
  if [ -e "$old" ]; then rm -rf "$old"; fi
done
for skill in "$here"/skills/atm-*; do
  if [ -d "$skill" ]; then cp -R "$skill" "$target/skills/"; fi
done

# The README's atm section, between the markers; the rest of the README as it is
begin='<!-- atm:begin -->'
end='<!-- atm:end -->'
section="$begin
## atm

Skills for [atm](https://github.com/solo-swe/agentic-task-manager), the Agentic Task Manager: version $version
(\`skills/atm-*\`). They need atm installed (\`brew install solo-swe/tap/atm\`). With atm installed,
\`atm agents install\` puts the skills that match your atm where your agent harness reads them, the better way;
without it, \`npx skills add solo-swe/skills --skill atm-loop-tasks\` (or pick them from the list).
$end"
readme="$target/README.md"
if [ ! -f "$readme" ]; then
  printf '# Skills\n\nAgent skills for solo-swe'"'"'s apps, one section each.\n\n%s\n' "$section" > "$readme"
elif grep -qF "$begin" "$readme"; then
  SECTION="$section" awk -v begin="$begin" -v end="$end" '
    $0 == begin { print ENVIRON["SECTION"]; skipping = 1; next }
    $0 == end { skipping = 0; next }
    !skipping { print }
  ' "$readme" > "$readme.new"
  mv "$readme.new" "$readme"
else
  printf '\n%s\n' "$section" >> "$readme"
fi
