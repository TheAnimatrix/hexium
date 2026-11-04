#!/usr/bin/env python3
"""Writes a Hexium release's notes (Markdown) to stdout: what changed since
the previous release, in Hexium's own patches and in Hex underneath.

    hexium/release-notes.py COMMIT VERSION HEX_VERSION [PREVIOUS]

COMMIT is the release's commit, VERSION its version (3.3.0-hexium.12), HEX_VERSION
the Hex version it is built on, and PREVIOUS the previous release's tag or
commit (none for the first release). The history of both must be local
(a full clone with tags).

Hexium is upstream Hex plus a queue of "hexium: " commits on top, so a
release's upstream base is the newest commit that isn't one of them:
- From Hex: upstream commits between the two bases, without CI, docs, site,
  iOS and other changes that never reach the app.
- Hexium: patches new since the previous release, and patches whose change
  is different now (by `git patch-id`), without the maintainers' tooling.
OpenCode is never named: Hexium ships without it.
"""

import re
import subprocess
import sys

HEX = "https://github.com/anomalyco/hex"
HEXIUM = "https://github.com/TheAnimatrix/hexium"
PATCH = re.compile(r"^hexium: ")
OPENCODE = re.compile(r"open\s*code", re.IGNORECASE)
# Upstream changes that never reach the desktop app Hexium ships.
NOT_THE_APP = re.compile(
    r"^(?:(?:ci|docs?|site|chore|tests?|bench|ios|nix|aur|release|scripts?)\b"
    r"|\w+\((?:ci|nix|site|docs?|ios|bench|tests?|release)\))",
    re.IGNORECASE,
)
# Conventional-commit prefixes: "fix(linux): x" reads as "Linux: x".
CONVENTIONAL = re.compile(r"^(?:fix|feat|perf|refactor)(?:\(([\w-]+)\))?!?:\s*", re.IGNORECASE)
# Hexium patches that only maintainers see.
TOOLING = re.compile(
    r"workflow|sync|README|contributing|docs\b|code-signing", re.IGNORECASE
)


def git(*args):
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout


def base(commit):
    """The upstream commit a release's patch queue sits on."""
    return git(
        "rev-list", "-1", "--invert-grep", "--grep", PATCH.pattern, commit
    ).strip()


def patches(commit):
    """Subject -> patch id for each Hexium patch in `commit`'s queue."""
    found = {}
    for line in git(
        "log", "--format=%H %s", f"{base(commit)}..{commit}"
    ).splitlines():
        sha, subject = line.split(" ", 1)
        if not PATCH.match(subject):
            continue
        diff = git("show", "--format=", sha)
        patch_id = subprocess.run(
            ["git", "patch-id", "--stable"],
            input=diff,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.split(" ", 1)[0]
        found[subject] = patch_id
    return found


def sentence(text):
    text = text.strip()
    return text[:1].upper() + text[1:]


def linked(subject):
    """Hex's "(#1234)" as links to its pull requests."""
    return re.sub(
        r"\(#(\d+)\)", lambda m: f"([#{m[1]}]({HEX}/pull/{m[1]}))", subject
    )


def from_hex(old, new):
    lines = []
    for subject in git(
        "log", "--no-merges", "--format=%s", f"{old}..{new}"
    ).splitlines():
        if NOT_THE_APP.match(subject) or OPENCODE.search(subject):
            continue
        if subject.startswith("Merge "):
            continue
        subject = CONVENTIONAL.sub(
            lambda m: f"{sentence(m[1])}: " if m[1] else "", subject
        )
        lines.append(f"- {linked(sentence(subject))}")
    return lines


def hexium(previous, commit):
    before = patches(previous) if previous else {}
    lines = []
    for subject, patch_id in reversed(list(patches(commit).items())):
        name = PATCH.sub("", subject)
        if TOOLING.search(name) or OPENCODE.search(name):
            continue
        if subject not in before:
            lines.append(f"- New: {sentence(name)}")
        elif before[subject] != patch_id:
            lines.append(f"- Updated: {sentence(name)}")
    return lines


def main(commit, version, hex_version, previous=None):
    commit = git("rev-parse", commit).strip()
    upstream = base(commit)
    out = [
        f"Hexium {version}, built on Hex {hex_version} "
        f"([{upstream[:7]}]({HEX}/commit/{upstream})) from "
        f"[{commit[:7]}]({HEXIUM}/commit/{commit}).",
        "",
        "**Updating:** Hexium updates itself on Windows and macOS: it downloads "
        "the new version in the background and shows **Restart to update** in "
        "the sidebar. A Hexium from before in-app updates needs this release "
        "installed once by hand. "
        f"Windows: `hexium-setup-{version}-windows-x64.exe` (or `-arm64`); "
        f"macOS: `hexium-{version}-macos-arm64.dmg`. Unsigned, so Windows "
        "SmartScreen and macOS Gatekeeper ask once.",
    ]
    if not previous:
        # The first release: what Hexium adds to Hex.
        out += ["", "## Hexium", "", "The first Hexium release, with:", ""]
        out += [line.replace("- New: ", "- ") for line in hexium(None, commit)]
        return "\n".join(out) + "\n"
    previous = git("rev-parse", f"{previous}^{{commit}}").strip()
    own = hexium(previous, commit)
    old_upstream = base(previous)
    hex_lines = from_hex(old_upstream, upstream)
    out += ["", "## Hexium", ""]
    out += own or ["- No changes of Hexium's own."]
    out += ["", f"## From Hex ({len(hex_lines)})", ""]
    if hex_lines:
        out += hex_lines
        out += [
            "",
            f"Every upstream commit: {HEX}/compare/{old_upstream[:12]}...{upstream[:12]}",
        ]
    else:
        out += ["- Same Hex as the previous release."]
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    if len(sys.argv) not in (4, 5):
        sys.exit(__doc__)
    sys.stdout.write(main(*sys.argv[1:]))
