#!/usr/bin/env python3
"""Writes the update manifest of a Hexium release (stable.json) to stdout,
in the format of Hex's release protocol (src/updater/protocol.rs), from the
files the release publishes.

    hexium/release-manifest.py DIR VERSION TAG SEQUENCE

DIR holds the release's files (hexium-<VERSION>-windows-x64.exe and so on),
TAG is its tag and SEQUENCE a number that grows with every release. Hexium's
updater reads the newest release's copy through
https://github.com/TheAnimatrix/hexium/releases/latest/download/stable.json
and downloads the target for its platform, checking the size and SHA-256
named here. Linux packages are listed as distributions, so a .deb or .rpm
install offers the release page.
"""

import datetime
import hashlib
import json
import sys
from pathlib import Path

REPOSITORY = "TheAnimatrix/hexium"


def targets(version):
    """Target name -> file name, for what Hexium's updater installs."""
    files = {}
    for arch in ("x64", "arm64"):
        files[f"windows-{arch}"] = f"hexium-{version}-windows-{arch}.exe"
        files[f"windows-{arch}-cli"] = f"hexium-{version}-windows-{arch}.com"
        files[f"windows-{arch}-setup"] = f"hexium-setup-{version}-windows-{arch}.exe"
    files["macos-arm64"] = f"hexium-{version}-macos-arm64.zip"
    files["macos-arm64-dmg"] = f"hexium-{version}-macos-arm64.dmg"
    return files


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def manifest(directory, version, tag, sequence):
    directory = Path(directory)
    listed = {}
    for target, name in targets(version).items():
        path = directory / name
        if not path.is_file():
            # A platform whose build didn't make it (macOS runners come and
            # go) is simply not offered until a rerun adds it.
            continue
        listed[target] = {
            "url": f"/{REPOSITORY}/releases/download/{tag}/{name}",
            "size": path.stat().st_size,
            "sha256": sha256(path),
        }
    if not any(target.startswith("windows-") for target in listed):
        sys.exit("no Windows build to list")
    return {
        "schema": 1,
        "app": "hex",
        "artifact": "desktop",
        "channel": "stable",
        "sequence": int(sequence),
        "published_at": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "release": {
            "version": version,
            "notes_url": f"https://github.com/{REPOSITORY}/releases/tag/{tag}",
            "rollout_percent": 100,
            "required": False,
            "targets": listed,
            "distributions": [{"kind": "direct"}]
            + [
                {"kind": f"{kind}-{arch}", "version": version}
                for kind in ("deb", "rpm")
                for arch in ("x64", "arm64")
            ],
        },
    }


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    json.dump(manifest(*sys.argv[1:]), sys.stdout, indent=2)
    sys.stdout.write("\n")
