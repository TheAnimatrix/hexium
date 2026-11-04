#!/bin/sh
# Weekly sync: replay Hexium's patch commits on top of upstream Hex.
#
#   hexium/sync.sh            # rebase, check, push
#   hexium/sync.sh --dry-run  # rebase and check only
#
# Layout (see hexium/README.md):
#   upstream/dev   anomalyco/hex
#   origin/dev     Hexium = upstream/dev + the "hexium: ..." commits
#   TCPP_DIR       a clone of TheAnimatrix/transcribe.cpp; its branch `hexium`
#                  is upstream's pinned transcribe.cpp revision + Hexium's patch
#
# Exit codes: 0 synced (or already current), 2 a conflict needs a person or an
# agent (the rebase is left in progress; fix it, `git rebase --continue`, and
# run this again), 1 anything else.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
TCPP_DIR=${TCPP_DIR:-"$root/../transcribe.cpp"}
dry_run=0
[ "${1:-}" = --dry-run ] && dry_run=1

say() { printf 'hexium-sync: %s\n' "$*"; }

git remote get-url upstream >/dev/null 2>&1 ||
  git remote add upstream https://github.com/anomalyco/hex.git
git fetch -q upstream dev
git fetch -q origin dev
git checkout -q dev
git reset -q --hard origin/dev

if git merge-base --is-ancestor upstream/dev HEAD; then
  say "already on top of upstream $(git rev-parse --short upstream/dev)"
  exit 0
fi

# 1. transcribe.cpp: upstream may have moved its pinned revision.
pin_of() { sed -n 's/^transcribe-cpp-sys = .*rev = "\([0-9a-f]*\)".*/\1/p'; }
upstream_rev=$(git show upstream/dev:Cargo.toml | pin_of)
old_rev=$(git show "$(git merge-base HEAD upstream/dev)":Cargo.toml | pin_of)
[ -d "$TCPP_DIR/.git" ] || git clone -q https://github.com/TheAnimatrix/transcribe.cpp.git "$TCPP_DIR"
git -C "$TCPP_DIR" remote get-url upstream >/dev/null 2>&1 ||
  git -C "$TCPP_DIR" remote add upstream https://github.com/anomalyco/transcribe.cpp.git
git -C "$TCPP_DIR" fetch -q origin hexium
if [ "$old_rev" != "$upstream_rev" ]; then
  say "transcribe.cpp pin moved ${old_rev%"${old_rev#???????}"} -> ${upstream_rev%"${upstream_rev#???????}"}; replaying the canary patch"
  git -C "$TCPP_DIR" fetch -q upstream "$old_rev" "$upstream_rev"
  git -C "$TCPP_DIR" checkout -q -B hexium origin/hexium
  if ! git -C "$TCPP_DIR" rebase -q --onto "$upstream_rev" "$old_rev" hexium; then
    say "transcribe.cpp patch conflicts in $TCPP_DIR; resolve it, push branch hexium, rerun"
    exit 2
  fi
  tcpp_sha=$(git -C "$TCPP_DIR" rev-parse hexium)
  if [ $dry_run = 0 ]; then
    git -C "$TCPP_DIR" push -q --force-with-lease origin hexium
    # Keep every pinned revision reachable for older releases.
    git -C "$TCPP_DIR" push -q origin "hexium:refs/tags/hexium-$(printf %.7s "$tcpp_sha")"
  fi
else
  tcpp_sha=$(git -C "$TCPP_DIR" rev-parse origin/hexium)
fi

# 2. Hex: replay the hexium commits; Cargo.toml/Cargo.lock conflicts (the
#    transcribe.cpp pin) are regenerated, anything else stops for review.
repin() {
  sed -i.bak \
    -e "s#^transcribe-cpp = .*#transcribe-cpp = { git = \"https://github.com/TheAnimatrix/transcribe.cpp\", rev = \"$tcpp_sha\" }#" \
    -e "s#^transcribe-cpp-sys = .*#transcribe-cpp-sys = { git = \"https://github.com/TheAnimatrix/transcribe.cpp\", rev = \"$tcpp_sha\" }#" \
    Cargo.toml && rm -f Cargo.toml.bak
  cargo update -q -p transcribe-cpp -p transcribe-cpp-sys
}
if ! git rebase -q upstream/dev; then
  while :; do
    conflicted=$(git diff --name-only --diff-filter=U)
    other=$(printf '%s\n' "$conflicted" | grep -v -x -e Cargo.toml -e Cargo.lock || true)
    if [ -n "$other" ]; then
      say "conflicts need review:"; printf '  %s\n' $other
      exit 2
    fi
    git checkout -q upstream/dev -- Cargo.toml Cargo.lock
    repin
    git add Cargo.toml Cargo.lock
    GIT_EDITOR=true git rebase --continue >/dev/null 2>&1 && break
    [ -d "$(git rev-parse --git-path rebase-merge)" ] || break
  done
fi
if [ "$(pin_of < Cargo.toml)" != "$tcpp_sha" ]; then
  repin
  git commit -q -am "hexium: pin transcribe.cpp $(printf %.7s "$tcpp_sha")"
fi

# 3. Checks, then publish. The release workflow builds on every push to dev.
# A macOS SDK older than 26.4 lacks SystemLanguageModel.contextSize; patch it
# for the local check only and always restore it.
swift=src/apple_intelligence.swift
if [ "$(uname)" = Darwin ] && ! grep -rq contextSize "$(xcrun --show-sdk-path)/System/Library/Frameworks/FoundationModels.framework" 2>/dev/null; then
  trap 'git checkout -q -- "$swift"' EXIT
  sed -i '' 's/SystemLanguageModel.default.contextSize/4096/' "$swift"
fi
cargo check --locked
cargo test --locked --release --bin hex -- transcription:: app_paths >/dev/null
if [ $dry_run = 1 ]; then
  say "dry run: rebased onto $(git rev-parse --short upstream/dev), not pushed"
  exit 0
fi
git push -q --force-with-lease origin dev
say "synced onto upstream $(git rev-parse --short upstream/dev); release build started"
