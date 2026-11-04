#!/bin/sh
# Hexium's upstream sync, patch-queue style (as Zen keeps Firefox): Hex is the
# pinned upstream (dev's base), Hexium is the queue of "hexium: …" commits on
# top, one change each, and every fork in hexium/forks is Hex's pinned
# revision of that dependency plus its own small queue on branch `hexium`.
# A sync replays each queue on the new upstream, regenerates the Cargo pins,
# checks, and pushes. .github/workflows/hexium-sync.yml runs it every week.
#
#   hexium/sync.sh                 replay, check, push dev (rerun it after
#                                  fixing a conflict; it carries on)
#   hexium/sync.sh --dry-run       replay and check; push nothing but fork tags
#   hexium/sync.sh --no-check      skip the local checks (CI runs its own)
#   hexium/sync.sh --push-to REF   push the result to REF instead of dev
#   hexium/sync.sh --pins [NAME=SHA ...]
#                                  only point Cargo.toml/Cargo.lock at the forks
#                                  (after committing to a fork's hexium branch)
#
# Exit codes: 0 synced (or already current), 2 a patch conflicts and needs a
# person or an agent (the rebase is left open: fix the files it names, `git
# add` them, and run this again; it finishes the rebase, checks and pushes),
# 1 anything else.
set -eu

root=${HEXIUM_ROOT:-$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)}
cd "$root"
# The rebase rewrites hexium/ under our feet (an older patch has older
# tools), so run from a copy taken before it starts.
if [ -z "${HEXIUM_TOOLS:-}" ]; then
  tools=$(mktemp -d)
  cp hexium/sync.sh hexium/scrub-docs.py hexium/forks "$tools/"
  HEXIUM_ROOT=$root HEXIUM_TOOLS=$tools exec sh "$tools/sync.sh" "$@"
fi
tools=$HEXIUM_TOOLS
FORKS_DIR=${FORKS_DIR:-"$root/.."}
dry_run=0 check=1 push_ref=dev pins_only=0
while [ $# -gt 0 ]; do
  case $1 in
    --dry-run) dry_run=1 ;;
    --no-check) check=0 ;;
    --push-to) push_ref=$2; shift ;;
    --pins) pins_only=1 ;;
    *=*) [ $pins_only = 1 ] || { echo "NAME=SHA needs --pins" >&2; exit 1; }
      overrides="${overrides:-} $1" ;;
    *) echo "unknown option $1" >&2; exit 1 ;;
  esac
  shift
done

say() { printf 'hexium-sync: %s\n' "$*"; }
short() { printf %.7s "$1"; }
forks() { grep -v -e '^#' -e '^[[:space:]]*$' "$tools/forks"; }
# The revision pinned for git URL $1 in the Cargo.toml on stdin.
rev_for() { grep -F "git = \"$1\"" | sed -n 's/.*rev = "\([0-9a-f]*\)".*/\1/p' | head -n 1; }
pins=$(mktemp)
trap 'rm -rf "$pins" "$pins.toml" "$tools"' EXIT

# Points Cargo.toml at each fork's revision in $pins. A dependency Hex already
# patches in [patch.crates-io] (transcribe.cpp) gets that line repointed; any
# other gets a [patch."<upstream>"] block for every crate Cargo.lock has from
# it (Rune). Then Cargo.lock is re-resolved without updating anything else.
apply_pins() {
  # Drop the block this wrote last time, and the blank lines it leaves.
  awk '/^# hexium-forks begin/{skip=1} !skip{line[++n]=$0} /^# hexium-forks end/{skip=0}
    END { while (n > 0 && line[n] == "") n--; for (i = 1; i <= n; i++) print line[i] }' Cargo.toml > Cargo.toml.new
  mv Cargo.toml.new Cargo.toml
  forks | while read -r name up fork; do
    sha=$(sed -n "s/^$name //p" "$pins")
    [ -n "$sha" ] || continue
    if awk -v up="$up" -v fork="$fork" '
        /^\[/ { in_patch = ($0 == "[patch.crates-io]") }
        in_patch && (index($0, "git = \"" up "\"") || index($0, "git = \"" fork "\"")) { found = 1 }
        END { exit !found }' Cargo.toml; then
      awk -v up="$up" -v fork="$fork" -v sha="$sha" '
        /^\[/ { in_patch = ($0 == "[patch.crates-io]") }
        in_patch && (index($0, "git = \"" up "\"") || index($0, "git = \"" fork "\"")) {
          sub(/ = .*/, " = { git = \"" fork "\", rev = \"" sha "\" }")
        }
        { print }' Cargo.toml > Cargo.toml.new
      mv Cargo.toml.new Cargo.toml
    else
      crates=$(awk -v up="git+$up?" -v fork="git+$fork?" '
        /^name = / { name = $3; gsub(/"/, "", name) }
        /^source = / && (index($0, up) || index($0, fork)) { print name }' Cargo.lock | sort -u)
      {
        printf '\n# hexium-forks begin (hexium/sync.sh writes this block from hexium/forks)\n'
        printf '[patch."%s"]\n' "$up"
        for crate in $crates; do
          printf '%s = { git = "%s", rev = "%s" }\n' "$crate" "$fork" "$sha"
        done
        printf '# hexium-forks end\n'
      } >> Cargo.toml
    fi
  done
  cargo metadata -q --format-version 1 > /dev/null
}

# The fork revisions dev uses now, read back from its Cargo.toml.
current_pins() {
  forks | while read -r name up fork; do
    sha=$(rev_for "$fork" < "${1:-Cargo.toml}")
    if [ -n "$sha" ]; then echo "$name $sha"; fi
  done > "$pins"
}

if [ $pins_only = 1 ]; then
  current_pins
  for pin in ${overrides:-}; do
    grep -v "^${pin%%=*} " "$pins" > "$pins.new" || true
    echo "${pin%%=*} ${pin#*=}" >> "$pins.new"
    mv "$pins.new" "$pins"
  done
  apply_pins
  say "Cargo.toml and Cargo.lock point at: $(tr '\n' ' ' < "$pins")"
  exit 0
fi

rebasing() { [ -d "$(git rev-parse --git-path rebase-merge)" ] || [ -d "$(git rev-parse --git-path rebase-apply)" ]; }
replay=1 resume=0
if rebasing; then
  # Rerun after fixing a conflict by hand: finish the rebase, resolving the
  # mechanical conflicts that come after it the same way.
  say "resuming the rebase in progress"
  resume=1
  git show HEAD:Cargo.toml > "$pins.toml"
  current_pins "$pins.toml"
else
  git remote get-url upstream > /dev/null 2>&1 ||
    git remote add upstream https://github.com/anomalyco/hex.git
  git fetch -q upstream dev
  git fetch -q origin dev
  if ! git diff --quiet || ! git diff --cached --quiet; then
    say "commit or stash your changes first"
    exit 1
  fi
  git checkout -q dev
  if git merge-base --is-ancestor HEAD origin/dev; then
    git reset -q --hard origin/dev
  else
    # Local work (a finished conflict fix, unpushed patches) is kept, but
    # only if it has everything origin's dev has.
    missing=$(git log --format=%s "$(git merge-base origin/dev upstream/dev)..origin/dev" |
      while read -r subject; do
        git log --format=%s "$(git merge-base HEAD upstream/dev)..HEAD" | grep -q -x -F -- "$subject" || echo "$subject"
      done)
    if [ -n "$missing" ] || ! git merge-base --is-ancestor "$(git merge-base origin/dev upstream/dev)" HEAD; then
      say "origin's dev has work this dev lacks; reset to origin/dev (or rebase onto it) first:"
      printf '%s\n' "$missing" | sed 's/^/  /'
      exit 1
    fi
  fi
  if git merge-base --is-ancestor upstream/dev HEAD; then
    if [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/dev)" ]; then
      say "already on top of upstream $(git rev-parse --short upstream/dev)"
      exit 0
    fi
    say "already on top of upstream; checking and pushing the local patches"
    replay=0
    current_pins
  fi
fi
base=$(git merge-base HEAD upstream/dev)

# 1. Forks: replay each queue when Hex moved the revision it pins.
[ $replay = 1 ] && [ $resume = 0 ] && forks | while read -r name up fork; do
  new=$(git show upstream/dev:Cargo.toml | rev_for "$up")
  old=$(git show "$base:Cargo.toml" | rev_for "$up")
  if [ -z "$new" ]; then
    say "$name: Hex no longer pins $up; update hexium/forks"
    exit 1
  fi
  dir=$FORKS_DIR/$name
  [ -d "$dir/.git" ] || git clone -q --filter=blob:none "$fork" "$dir"
  git -C "$dir" fetch -q "$fork" hexium
  tip=$(git -C "$dir" rev-parse FETCH_HEAD)
  git -C "$dir" fetch -q "$up" "$new"
  if git -C "$dir" merge-base --is-ancestor "$new" "$tip"; then
    sha=$tip
  else
    say "$name: Hex moved its pin $(short "$old") -> $(short "$new"); replaying the fork's commits"
    git -C "$dir" fetch -q "$up" "$old"
    git -C "$dir" checkout -q --detach "$tip"
    if ! git -C "$dir" rebase -q --onto "$new" "$old"; then
      say "$name: its patch conflicts in $dir; resolve it, push it as branch hexium, rerun"
      exit 2
    fi
    sha=$(git -C "$dir" rev-parse HEAD)
    # Cargo can only build a revision it can fetch, and the tag keeps every
    # pinned revision reachable for older releases.
    git -C "$dir" push -q "$fork" "HEAD:refs/tags/hexium-$(short "$sha")"
    [ $dry_run = 1 ] || git -C "$dir" push -q --force "$fork" HEAD:refs/heads/hexium
  fi
  echo "$name $sha" >> "$pins"
done

# 2. Hex: replay the queue. Conflicts in files only Hexium's tooling writes
#    are resolved mechanically: the Cargo pins are regenerated, README.md and
#    CONTRIBUTING.md stay Hexium's, and a doc takes upstream's text with
#    OpenCode scrubbed out (hexium/scrub-docs.py). Anything else stops for a
#    person or an agent.
resolve() {
  other=
  for f in $(git diff --name-only --diff-filter=U); do
    case $f in
      Cargo.toml | Cargo.lock)
        git checkout -q HEAD -- Cargo.toml Cargo.lock
        apply_pins
        git add Cargo.toml Cargo.lock ;;
      README.md | CONTRIBUTING.md)
        git checkout -q --theirs -- "$f"
        git add -- "$f" ;;
      docs/*.md)
        if ! git cat-file -e "REBASE_HEAD:$f" 2> /dev/null || ! git cat-file -e "HEAD:$f" 2> /dev/null; then
          git rm -q -- "$f" # deleted by the patch, or by upstream
        else
          git checkout -q HEAD -- "$f"
          python3 "$tools/scrub-docs.py" "$f" > /dev/null
          git add -- "$f"
        fi ;;
      *) other="$other $f" ;;
    esac
  done
  if [ -n "$other" ]; then
    say "patch $(git rev-parse --short REBASE_HEAD) ($(git log -1 --format=%s REBASE_HEAD)) conflicts with upstream in:"
    printf '  %s\n' $other
    exit 2
  fi
}
if [ $resume = 1 ] || { [ $replay = 1 ] && ! git rebase -q upstream/dev; }; then
  while :; do
    resolve
    at=$(git rev-parse -q --verify REBASE_HEAD || true)
    GIT_EDITOR=true git rebase --continue > /dev/null 2>&1 && break
    rebasing || break
    if [ -z "$(git diff --name-only --diff-filter=U)" ] && [ "$(git rev-parse -q --verify REBASE_HEAD || true)" = "$at" ]; then
      say "git rebase --continue is stuck on $(short "$at"); see git status"
      exit 1
    fi
  done
fi
# Fold regenerated pins into the first patch and newly scrubbed docs into the
# docs patch, so each patch keeps one concern.
patch_named() { git log --format='%H %s' upstream/dev..HEAD | sed -n "s/^\([0-9a-f]*\) hexium: $1\$/\1/p" | head -n 1; }
fixups=0
apply_pins
if ! git diff --quiet -- Cargo.toml Cargo.lock; then
  git commit -q --fixup="$(git log --reverse --format=%H upstream/dev..HEAD | head -n 1)" Cargo.toml Cargo.lock
  fixups=1
fi
python3 "$tools/scrub-docs.py" > /dev/null
if ! git diff --quiet -- docs; then
  git commit -q --fixup="$(patch_named 'docs without OpenCode')" -- docs
  fixups=1
fi
[ $fixups = 0 ] || GIT_SEQUENCE_EDITOR=true git rebase -q -i --autosquash upstream/dev

# 3. Checks, then publish (pushing dev runs the Hexium release workflow).
if [ $check = 1 ]; then
  # A macOS SDK older than 26.4 lacks SystemLanguageModel.contextSize; patch
  # it for the local check only and always restore it.
  swift=src/apple_intelligence.swift
  if [ "$(uname)" = Darwin ] && ! grep -rq contextSize "$(xcrun --show-sdk-path)/System/Library/Frameworks/FoundationModels.framework" 2> /dev/null; then
    trap 'git checkout -q -- "$swift"; rm -rf "$pins" "$pins.toml" "$tools"' EXIT
    sed -i '' 's/SystemLanguageModel.default.contextSize/4096/' "$swift"
  fi
  cargo check --locked
  cargo test --locked --release --bin hex -- transcription:: app_paths hexium_ i18n > /dev/null
fi
if [ $dry_run = 1 ]; then
  say "dry run: rebased onto $(git rev-parse --short upstream/dev), not pushed"
  exit 0
fi
if [ "$push_ref" = dev ]; then
  git push -q --force-with-lease=dev:"$(git rev-parse origin/dev)" origin HEAD:dev
else
  git push -q --force origin "HEAD:refs/heads/$push_ref"
fi
say "synced onto upstream $(git rev-parse --short upstream/dev); pushed $push_ref at $(git rev-parse --short HEAD)"
