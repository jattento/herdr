#!/usr/bin/env bash
# Absorb a new upstream snapshot: refresh the mirror branch, tag a rollback
# point, and rebase our delta on top.
#
# Topology: `main` carries our commits, `upstream` mirrors xai-org/main exactly.
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

if [ -n "$(git status --porcelain)" ]; then
  echo "error: working tree is dirty. Commit or stash first." >&2
  exit 1
fi

branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$branch" != "master" ]; then
  echo "error: run this from main (currently on '$branch')." >&2
  exit 1
fi

git remote get-url upstream >/dev/null 2>&1 ||
  git remote add upstream https://github.com/herdrdev/herdr.git

git config rerere.enabled true
git fetch upstream --tags

old=$(git rev-parse upstream/master@{1} 2>/dev/null || echo "")
new=$(git rev-parse upstream/master)
if git merge-base --is-ancestor "$new" HEAD; then
  echo "Already on top of upstream/master ($(git rev-parse --short "$new")). Nothing to sync."
  git branch -f upstream upstream/master
  exit 0
fi

tag="pre-sync/$(date +%Y-%m-%d)"
suffix=1
while git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; do
  suffix=$((suffix + 1))
  tag="pre-sync/$(date +%Y-%m-%d).$suffix"
done
git tag "$tag" master
echo "Rollback point: $tag -> $(git rev-parse --short master)"

git branch -f upstream upstream/master
echo "Mirror branch 'upstream' now at $(git rev-parse --short upstream/master)"
[ -n "$old" ] && echo "Upstream moved: $(git rev-parse --short "$old") -> $(git rev-parse --short "$new")"

echo
echo "Rebasing our delta onto upstream/master..."
if ! git rebase upstream/master; then
  cat >&2 <<'MSG'

Rebase stopped on a conflict. Resolve it, then `git rebase --continue`.
To abort entirely: `git rebase --abort` (main is also saved at the tag above).
MSG
  exit 1
fi

cat <<MSG

Rebase clean. Now verify, from overlay/TOUCHPOINTS.md:
  cargo check -p herdr-spaces && cargo test -p herdr-spaces
  cargo check
  cargo build --release
  overlay/scripts/overlay-diff.sh

Then publish the rewritten history:
  git push --force-with-lease origin master
  git push origin upstream
MSG
