#!/usr/bin/env python3
"""Remove OpenCode from Markdown docs, mechanically, so the docs never block
the weekly sync. hexium/sync.sh runs it on a docs file that conflicts (after
taking upstream's version) and on all of docs/ after each sync.

Rules, per file:
- a heading that names OpenCode drops its whole section (up to the next
  heading of the same or a higher level);
- in a table, list or fenced block, only the rows, items or lines that name
  it go;
- any other paragraph that names it goes.

    hexium/scrub-docs.py [FILE ...]   # default: every .md under docs/
"""

import re
import sys
from pathlib import Path

NAME = re.compile(r"open\s*code|opencode", re.IGNORECASE)
HEADING = re.compile(r"^(#{1,6})\s")
ITEM = re.compile(r"^(\s*)([-*+]|\d+[.)])\s")
FENCE = re.compile(r"^\s*(```|~~~)")


def blocks(lines):
    """Paragraph-like blocks; a fenced code block is always one block."""
    block, fenced = [], False
    for line in lines:
        if FENCE.match(line):
            fenced = not fenced
        if not fenced and not line.strip() and not FENCE.match(line):
            if block:
                yield block
                block = []
            yield [line]
            continue
        block.append(line)
    if block:
        yield block


def scrub_block(block):
    if not any(NAME.search(line) for line in block):
        return block
    first = block[0]
    if FENCE.match(first):
        return [line for line in block if not NAME.search(line)]
    if first.lstrip().startswith("|"):
        return [line for line in block if not NAME.search(line)]
    if ITEM.match(first):
        items, current = [], []
        for line in block:
            if ITEM.match(line) and current:
                items.append(current)
                current = []
            current.append(line)
        items.append(current)
        kept = [item for item in items if not any(NAME.search(l) for l in item)]
        return [line for item in kept for line in item]
    return []


def scrub(text):
    lines = text.splitlines(keepends=True)
    out, drop_level = [], None
    for block in blocks(lines):
        heading = HEADING.match(block[0])
        if heading:
            level = len(heading.group(1))
            if drop_level is not None and level <= drop_level:
                drop_level = None
            if drop_level is None and NAME.search(block[0]):
                drop_level = level
        if drop_level is not None:
            continue
        out.extend(scrub_block(block))
    # Collapse the blank runs left behind.
    text = re.sub(r"\n{3,}", "\n\n", "".join(out))
    return text.lstrip("\n")


def main(paths):
    files = [Path(p) for p in paths] or sorted(Path("docs").rglob("*.md"))
    changed = 0
    for path in files:
        if not path.is_file():
            continue
        before = path.read_text()
        after = scrub(before)
        if after != before:
            path.write_text(after)
            changed += 1
            print(f"scrubbed {path}")
    return changed


if __name__ == "__main__":
    main(sys.argv[1:])
