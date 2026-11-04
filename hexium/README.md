# How Hexium is kept

Hexium is [Hex](https://github.com/anomalyco/hex) plus a short stack of
`hexium: …` commits on `dev`. Each commit does one thing, so the stack replays
on new upstream code with few conflicts:

| Commit | What it changes |
| --- | --- |
| build against transcribe.cpp with Indic-Transcribe canary support | `Cargo.toml` / `Cargo.lock` pin [TheAnimatrix/transcribe.cpp](https://github.com/TheAnimatrix/transcribe.cpp) branch `hexium` |
| import Canary GGUFs and offer Indic languages | `custom_models.rs`, `languages.rs` |
| own identity; no self-updates | name, folders, Windows lock/login item/installer, macOS bundle id, updater off |
| new icon | `resources/windows/*`, `app/AppIcon.icon` (from `branding/make_icons.py`) |
| Indic-Transcribe-Flex as a built-in model | `artifacts.rs`, `models.rs`, picker blurb |
| no assistant integration | one switch in `app_paths.rs` plus hidden UI and docs |
| README, release workflow, sync | `README.md`, `CONTRIBUTING.md`, `.github/workflows/hexium-release.yml`, `hexium/` |

## Weekly sync

`hexium/sync.sh` fetches upstream `dev`, replays the stack, regenerates the
transcribe.cpp pin if upstream moved it (replaying the canary patch on the
fork first), runs `cargo check` and the transcription tests, and pushes `dev`.
A push to `dev` runs **Hexium release**, which builds Windows, Linux and macOS
and publishes a GitHub release. A conflict outside `Cargo.toml`/`Cargo.lock`
stops the script with exit code 2 and the rebase left open for a fix.

## The model

`TheAnimatrix/indic-transcribe-flex-gguf` on Hugging Face holds
`indic-transcribe-flex-Q8_0.gguf`, converted from Bodhan AI's published
`.nemo` with transcribe.cpp's `scripts/convert-indic-canary.py
--default-mode mixed` and `transcribe-quantize --quant Q8_0`, with the
Indic Open Model License and its credit. Built with Indic-Transcribe-Flex from
Bodhan AI / AI4Bharat.

## Releases

Unsigned. Windows: `hexium-setup-<version>-windows-<x64|arm64>.exe` installs
per user into `%APPDATA%\hexium` beside any Hex install. macOS:
`Hexium.app` (`com.avarnic.hexium`), ad-hoc signed.
