# How Hexium is kept

Hexium keeps its changes the way [Zen](https://github.com/zen-browser/desktop)
keeps Firefox's: upstream stays pinned and untouched, Hexium's changes are a
short **patch queue** replayed on top, and a job every three days re-applies the queue to
the newest upstream, tests it, and ships it.

- **Upstream:** [Hex](https://github.com/anomalyco/hex) `dev`. The base of
  Hexium's `dev` is the pinned upstream commit.
- **The queue:** the `hexium: …` commits on `dev`, one change each, so a
  conflict names exactly one change. `git log upstream/dev..dev` lists them;
  `git format-patch upstream/dev` exports them as patch files.
- **Forked dependencies** ([`forks`](forks)): `transcribe.cpp` (the canary
  prompt slots Indic-Transcribe needs) and `rune` (fallback fonts for every
  Indian script). Each fork's branch `hexium` is the revision Hex pins plus
  its own queue. The Cargo pins to them are written by `sync.sh`, never by
  hand, so they never need a merge.

| Patch | What it changes |
| --- | --- |
| build against patched transcribe.cpp and Rune | `Cargo.toml` / `Cargo.lock` (written by `sync.sh`) |
| import Canary GGUFs and offer Indic languages | `custom_models.rs`, `languages.rs` |
| own identity | name, folders, Windows lock/login item/installer/publisher, macOS bundle id |
| new icon | `resources/windows/*`, `app/AppIcon.icon` (from `branding/make_icons.py`) |
| Indic-Transcribe-Flex as a built-in model | `artifacts.rs`, `models.rs`, picker blurb |
| Auto language for Indic-Transcribe-Flex | `language_id.rs` (VoxLingua107), one step in `gguf_session::run`, the identifier's download in `models.rs` |
| learn from fixes, quote what's on screen | `dictation/{fix_learning,screen_quotes,screen_text}.rs`, a screen reader per OS (`accessibility/`, `windows/`, `linux/atspi/`), two Labs switches, one step in `processor.rs` and one after each OS's paste |
| README, contributing, sync | `README.md`, `CONTRIBUTING.md`, `hexium/` |
| release workflow | `.github/workflows/hexium-release.yml`, the changelog (`hexium/release-notes.py`) and update manifest (`hexium/release-manifest.py`) |
| no assistant integration | one switch in `app_paths.rs` plus hidden UI and docs |
| update in the app | Hex's updater (`src/updater*`) reads Hexium's releases: manifest URL, one channel, no Anomaly signature, Hexium's names |
| say Hexium in the UI | `src/hexium_brand.rs`, one call in `desktop::i18n::tr` |
| sync and checks | `hexium/sync.sh`, `hexium/forks`, `hexium/scrub-docs.py`, `hexium/check-tests.sh`, `hexium/expected-failures`, `.github/workflows/hexium-{sync,check}.yml` |

## The sync

**Hexium sync** (`.github/workflows/hexium-sync.yml`, every three days at 08:00 IST, or
run it from the Actions tab) does everything:

1. **Replay.** `hexium/sync.sh` replays each fork's queue if Hex moved its
   pin, then the Hexium queue on upstream `dev`, rewrites the Cargo pins, and
   pushes the result to `sync/upstream`.
2. **Check.** **Hexium check** runs upstream's Linux suite on it: format,
   every unit and golden test, and a real dictation through a virtual
   microphone. Like Zen judging its patches against Firefox, a failure only
   counts when Hexium causes it: `check-tests.sh` reruns failing tests on the
   upstream commit underneath, and [`expected-failures`](expected-failures)
   lists the tests and screens Hexium changes on purpose. The same check runs
   on every pull request.
3. **Promote.** `dev` moves to the checked commit, and **Hexium release**
   builds Windows, Linux and macOS and publishes the release.

Conflicts in files only Hexium's tooling writes resolve themselves: the Cargo
pins are regenerated, `README.md`/`CONTRIBUTING.md` stay Hexium's, and a doc
under `docs/` takes upstream's text with OpenCode scrubbed out by
[`scrub-docs.py`](scrub-docs.py). Any other conflict changes nothing, and an
issue labelled `upstream-sync` shows the log. Fix it locally:

```sh
hexium/sync.sh                # stops at the conflict, rebase left open
# fix the files it names, `git add` them, then
sh .git/hexium-sync/sync.sh   # finishes the rebase, checks, pushes dev
```

The rerun uses the copy of the script that started the rebase, kept in
`.git/hexium-sync/`, because a rebase stopped at an older patch has an older
`hexium/` (or none).

Changing a fork: commit on its `hexium` branch, push it, then
`hexium/sync.sh --pins transcribe.cpp=<sha>` (or `rune=<sha>`) and commit the
result into the first patch.

## The model

`TheAnimatrix/indic-transcribe-flex-gguf` on Hugging Face holds
`indic-transcribe-flex-Q8_0.gguf`, converted from Bodhan AI's published
`.nemo` with transcribe.cpp's `scripts/convert-indic-canary.py
--default-mode mixed` and `transcribe-quantize --quant Q8_0`, with the
Indic Open Model License and its credit. Built with Indic-Transcribe-Flex from
Bodhan AI / AI4Bharat.

## Releases

Unsigned. Windows: `hexium-setup-<version>-windows-<x64|arm64>.exe` installs
per user into `%APPDATA%\hexium`. macOS:
`Hexium.app` (`com.avarnic.hexium`), ad-hoc signed.

A version is Hex's with the release workflow's run number
(`3.3.0-hexium.12`); the tag adds the commit (`v3.3.0-hexium.12-783bb3e`).
Each release's notes list
Hexium's new or changed patches and Hex's commits since the previous
release.

Hexium updates itself the way Hex does, from its own releases: every release
publishes `stable.json`, and the app reads the newest one through
`releases/latest/download/stable.json` ten seconds after launch, then hourly
and on Check for updates. On Windows (an install from `hexium-setup`) and
macOS (a `Hexium.app` the user can replace) it downloads the new build,
checks its size and SHA-256 against the manifest, and offers Restart to
update. Linux packages and copies that can't replace themselves get Download
update, which opens the release page.
