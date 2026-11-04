#!/bin/sh
# Hexium's test gate, judged against upstream: runs the suite on Hexium, and
# for anything that fails, runs the same tests on the upstream commit Hexium
# sits on. Only failures Hexium causes count; failures upstream has too are
# upstream's, and the ones Hexium means (hexium/expected-failures) are allowed.
# Golden scenes count one by one, as `golden:<scene>`.
#
#   hexium/check-tests.sh [NEXTEST ARGS...]   # default: --bin hex
set -eu
export LC_ALL=C

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
work=$(mktemp -d)
trap 'git worktree remove --force "$work/base" 2> /dev/null || true; rm -rf "$work"' EXIT
[ $# -gt 0 ] || set -- --bin hex
say() { printf 'hexium-check: %s\n' "$*"; }

# Failing tests and golden scenes in a nextest log, one per line.
failures() {
  sed 's/\x1b\[[0-9;]*m//g' "$1" | awk '
    $1 == "FAIL" && $2 ~ /^\[/ { print $NF }
    /^ *[A-Za-z0-9_-]+: [0-9]+ pixels differ/ { name = $1; sub(/:$/, "", name); print "golden:" name }
    /golden .* (is missing|missing)/ { print "golden-missing:" $0 }' | sort -u > "$1.list"
  # The goldens test is judged by its scenes when it names them.
  if grep -q '^golden:' "$1.list"; then
    grep -v -x 'rune_ui::tests::goldens' "$1.list" || true
  else
    cat "$1.list"
  fi
}

if cargo nextest run --locked --profile ci --no-fail-fast "$@" > "$work/hexium.log" 2>&1; then
  tail -n 3 "$work/hexium.log"
  say "all tests pass"
  exit 0
fi
tail -n 30 "$work/hexium.log"
failures "$work/hexium.log" > "$work/hexium.fail"
if [ ! -s "$work/hexium.fail" ]; then
  say "the test run failed before reporting a test (build error?)"
  exit 1
fi

# The upstream commit under Hexium's patches.
first=$(git log --reverse --format='%H %s' HEAD | awk '$2 == "hexium:" { print $1; exit }')
if [ -z "$first" ]; then
  say "no hexium: patches found; nothing to compare with"
  exit 1
fi
base=$(git rev-parse "$first^")
say "$(wc -l < "$work/hexium.fail" | tr -d ' ') failing; rerunning them on upstream $(git rev-parse --short "$base")"
git worktree add -q --detach "$work/base" "$base"
filter=$(grep -v '^golden' "$work/hexium.fail" | sed 's/.*/test(=&)/' | paste -sd '|' - || true)
if grep -q '^golden:' "$work/hexium.fail"; then
  filter="${filter:+$filter|}test(=rune_ui::tests::goldens)"
fi
(
  cd "$work/base"
  CARGO_TARGET_DIR="$root/target" cargo nextest run --locked --profile ci --no-fail-fast "$@" -E "$filter"
) > "$work/base.log" 2>&1 || true
failures "$work/base.log" > "$work/base.fail"
grep -v -e '^#' -e '^[[:space:]]*$' hexium/expected-failures 2> /dev/null | sed 's/[[:space:]]*#.*//' | sort -u > "$work/expected" || true

comm -12 "$work/hexium.fail" "$work/base.fail" > "$work/upstream"
comm -23 "$work/hexium.fail" "$work/base.fail" | comm -23 - "$work/expected" > "$work/ours"
if [ -s "$work/upstream" ]; then
  say "failing on upstream too (not Hexium's):"
  sed 's/^/  /' "$work/upstream"
fi
expected=$(comm -23 "$work/hexium.fail" "$work/base.fail" | comm -12 - "$work/expected")
if [ -n "$expected" ]; then
  say "different by design (hexium/expected-failures):"
  printf '%s\n' "$expected" | sed 's/^/  /'
fi
if [ -s "$work/ours" ]; then
  say "Hexium broke:"
  sed 's/^/  /' "$work/ours"
  say "fix them, or list one in hexium/expected-failures if Hexium changes it on purpose"
  exit 1
fi
say "no failures of Hexium's own"
