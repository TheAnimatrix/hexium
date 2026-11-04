# Hex Feature Map

What can the user do, how do they reach it, and what would prove it works?
This maps existing behavior. [ROADMAP.md](../../ROADMAP.md) owns future work;
[AGENTS.md](../../AGENTS.md) owns implementation invariants.

Initial source baseline: September 1, 2026, `9e9da53aa9ed`. Public macOS release:
[2.3.1](../releases/2.3.1.md), with custom-domain downloads, a GitHub DMG mirror,
and model-download recovery guidance. The initial map was source-only. Listed checks are
locators unless an executed result is explicitly recorded, as in the
[keyboard-layout verification](recovery.md#keyboard-layout-resolution).

## Product Map

```ts
Hex                                      // macOS release
├── Get ready                            // install, permissions, local model
├── Dictate                              // hold OR double-tap lock
├── Use the result                       // paste, paste-last, History
├── Adjust behavior                      // shortcut, microphone, recording preferences
├── Process text                         // Output tab + context-selected Modes
├── Opt in                               // Meetings: off by default
├── Recover                              // prove the next attempt works
└── Maintain the app                     // menu bar, login, updates, quit
```

[Dictation](dictation.md), [Meetings](meetings.md), and [Recovery](recovery.md)
have detailed maps.
[Accessibility](accessibility.md) maps what screen readers, Voice Control, and
automation (strobe's benches) read from the window, the HUD, and the capsules.
Agents can [read retained transcripts](transcript-cli.md) and
[control the running app](control-cli.md) with the release CLI.
[Labs](labs.md) gives every unfinished feature its own switch, off by
default, listed under collapsible groups (Dictation, Output, History,
Meetings, Annotate, Shortcuts, Models) with only this OS's labs.
Its Restore shortcut default toggle (off by default) puts a Restore default
button beside Paste last dictation's shortcut (#613); its Sort and filter
models toggle (off by default) adds a sort and a saved-language filter to the
Local transcription picker (#645); its Benchmark details toggle (off by
default) shows each model's raw WER or CER with its language, dataset, source,
and conditions (#647); its Settings search toggle (off by default) puts a
search field at the top of General that filters its rows; its Release notes
link toggle (off by default) puts a link to this build's release notes in the
sidebar footer.
General › Advanced › Interface language offers Português (Brasil), and Hex
uses it for any Portuguese system language (#867); it also offers Français,
used for any French system language (#892), and Русский, used for any
Russian system language (#1317). None of these has had a native speaker's
review yet. It offers 简体中文 (`zh`) and 繁體中文 (`zh-Hant`, #944);
Traditional Chinese is drafted from the Simplified strings by a character and
Taiwan-usage phrase converter (`desktop/i18n/traditional_chinese.rs`), not yet
reviewed by a native speaker.
The [vocabulary](vocabulary.md) biases transcription toward names you list.
[Your own speech models](own-models.md) covers adding a GGUF file and using
models another app already downloaded.
[Annotate](capture.md) (macOS, Linux,
and Windows) records the screen while you draw and talk and sends it to an agent.
[Screenshots](labs.md) (Labs, macOS) takes CleanShot-style ⌘⇧4 area and
window screenshots.
The other capabilities stay here until they need their own verification recipe.

## Get Ready

```ts
Download Hex                              // setup.download-recovery
├── anoma.ly/hex names the visitor's platform  // setup.download-button
│   ├── Mac -> DMG | Intel Mac -> disabled, "needs Apple silicon"
│   ├── Windows x64 / ARM64 (UA-CH) -> hex-setup-<version>-windows-<arch>.exe + SmartScreen note
│   ├── Linux -> copyable `curl … install-linux.sh | sh` + Arch/glibc requirement
│   ├── Phone -> Send yourself the link
│   └── Other platforms -> every build, Homebrew, the GitHub release
├── Website, README, installers, apps, feeds, appcast -> anoma.ly/hex/files -> R2 bytes (Worker hex-production-site)
├── Releases up to 2.3.1 -> downloads.hex.kitlangton.dev (Kit's frozen bucket) -> the same bytes
├── GitHub v<version> (app-v through 2.3.1) -> identical DMG, Setup.exe, and Linux bytes
└── Older app cannot reach its update feed -> manually install the DMG

Install on Windows                        // setup.windows-installer
├── Setup.exe, per user, no admin -> managed versions\<version>\hex.exe
│   ├── x64 without a Vulkan 1.2 driver -> installs; speech on the CPU + Settings note (#1240)
│   ├── another version installed (e.g. self-updated) -> install this one over it
│   └── finish -> Open Hex -> the first run downloads a model
└── Settings › Apps -> uninstall every version, shortcut, Launch at login; keep data

Install on Linux                          // setup.linux-installer
└── curl … | sh -> signed feed -> verified artifact -> launcher + hex.service
    ├── cannot start -> name the glibc floor or the missing Arch packages
    └── finish without wl-copy -> print apt/dnf/pacman commands for wl-clipboard and wtype

Prepare local models                      // separate network dependency
└── Dictation -> huggingface.co and its download CDN
    └── Transfer fails -> host-specific guidance -> restore access -> Retry
```

The website shows the pinned macOS version and platform requirements beside its
download links. App mirrors use explicit `v…` release tags (`app-v…` through 2.3.1) rather than GitHub's
repository-wide latest release, which may refer to the SDK. Builds on the release
service update from `https://anoma.ly/release` (docs/releasing.md); the 2.3.1
feeds, appcast, and the legacy `r2.dev` endpoint stay frozen for installed
clients. Hosting does not guarantee access through every corporate policy.

The Windows installer is silently installed, reinstalled, installed over
another version, and uninstalled on x64 and ARM64 runners by
[test-windows-installer.ps1](../../scripts/test-windows-installer.ps1); no
interactive install, SmartScreen prompt, or first launch on a real PC has been
observed. The Linux library diagnosis has no automated check. The site's
platform states were captured once in headless Chromium with `?platform=`.

Sources: [website](../../site/src/Download.tsx), [installer](../../packaging/windows/hex.iss),
[release publisher](../../.github/release/release-service.sh),
[mirror publisher](../../scripts/publish-app-mirror.sh),
[macOS release publisher](../../scripts/release-app.sh),
[Linux installer](../../.github/release/install-linux.sh), and
[updater](../../src/updater.rs).
Model transfer failures in [transcription/models.rs](../../src/transcription/models.rs)
explain the source host, retry, and
separate prerequisites. These messages do not diagnose a particular firewall
or establish a successful subsequent model download.
Executed hosting checks and deployment details are recorded in
[Download Hosting](../downloads.md). In-app feed defaults and recovery copy ship
with 2.1.17; the initial 2.1.16 mirror preserves that release's original bytes.

```ts
Launch Hex, setup incomplete              // setup.tour; every build and OS, full window
├── One onboarding: the first-run story (rune_ui/onboarding_kit/live.rs) over
│   the tour state machine (desktop::onboarding::Flow); no flag, no other tour
├── Skip (quiet, top-right, only on the demo and the practice)    // setup.skip
│   ├── Passes over that part to the next step setup still needs: permissions,
│   │   then Almost ready while the model downloads, else Hex is ready
│   ├── Never closes onboarding or records it done
│   └── No Skip on permissions, the model step or download, or the paste
│       permission. Check: `onboarding_skip` (scripts/check-onboarding.sh)
├── GNOME/KDE Wayland (pastes through the RemoteDesktop portal, no wtype)
│   and no saved grant -> before Your turn, "Let Hex paste for you"     // setup.paste-access
│   ├── Says why: Hex only sends the paste keystroke, never sees the screen;
│   │   the desktop asks once and may call it remote desktop (#932)
│   ├── Allow -> opens the desktop's dialog; the grant's restore token is saved,
│   │   so the first paste needs no dialog
│   ├── Refused -> Try again, or Continue (the desktop asks at the first paste)
│   ├── X11, wlroots with wtype, macOS, Windows -> no page
│   └── Skipped or refused -> Settings › General shows it under "Permissions
│       needed" until allowed. Check: golden `onboarding-paste`; the real
│       portal dialog is not exercised
├── 2.3.1 or Swift Hex -> Sparkle offers the new Hex -> Install Update   // setup.legacy-update; macOS
│   ├── Replaces the old app in place (its file name kept) and opens the new one
│   ├── Accepted only because app/Info.plist keeps the old SUPublicEDKey;
│   │   scripts/validate-app.sh requires it. Feed items: docs/plans/swift-app-handoff.md
│   └── No automated check; a clean-Mac install from each old app is the proof
├── 2.3.1 Linux or Windows updates into this build     // setup.legacy-adopt
│   ├── It lands in <data>/voice-control/versions/<version>; its first app or
│   │   `service` launch from there moves it into the managed layout, then
│   │   relaunches from it (Linux: exec, so the old unit keeps its process)
│   ├── Linux -> 2.3.1's marked unit, ~/.local/bin/hex link, and hex.desktop
│   │   entries move aside -> the built-in .github/release/install-linux.sh
│   │   installs this executable -> session.env copied; failure puts them back
│   ├── Windows -> %APPDATA%\hex\versions, receipt, Hex.lnk recreated,
│   │   Launch at login repointed, 2.3.1's Apps & Features entry removed
│   ├── Failure -> logged, runs in place; the next launch tries again
│   ├── Settings, history, and models in voice-control are not carried over
│   └── Check: `a_legacy_linux_install_moves_into_the_installers_layout`,
│       `a_failed_move_puts_the_legacy_files_back`; Windows and a real 2.3.1
│       feed update are unverified
├── Upgrading from 2.3.1 or earlier (same `hex` folder)     // setup.upgrade; macOS
│   ├── `anomaly.hex` is a new identity: macOS kept none of the old grants
│   ├── Settings without `anomaly-hex-identity` in `migrations` -> checked once at launch
│   │   ├── Permissions or the selected model not ready -> the old completion
│   │   │   no longer counts -> onboarding opens from the start
│   │   └── Everything ready -> no onboarding; nothing but the ID changes
│   ├── Saved settings are kept; the ID is recorded either way
│   └── Setup ready, listener starting -> a running 2.3.1 or Swift Hex   // setup.quit-legacy
│       (retired bundle IDs) is asked to quit, so both never answer Option;
│       it stays installed. No check; `macos_rune::quit_legacy_hex`
├── Setting up Hex -> speech model download starts at once  // setup.background-download
│   ├── Model -> the selection recommended for the system's first preferred
│   │            language (English -> Parakeet Unified English; e.g. French ->
│   │            Parakeet v3, Japanese -> Whisper large-v3-turbo); when that
│   │            language isn't English, the language capsule switches between
│   │            English and its recommended model with one press, with no
│   │            dropdown chevron; with English as the system language there is
│   │            nothing to switch to, so the capsule just shows English,
│   │            whatever English model is selected
│   ├── The download is held while the Hugging Face scan runs (headers only)
│   ├── Models found here -> the model step instead      // setup.found-choice
│   │   ├── "Choose a speech model": one list, ours first and selected, then the
│   │   │   found files; a pick starts at once (ours downloads, a found one is
│   │   │   checked and used in place); Continue starts the selected row
│   │   ├── A found pick moves on with no download page or pill
│   │   └── Back on the pages after it returns to the list to change the pick
│   ├── Nothing found -> this page as always, the recommended model downloading
│   ├── Progress moves to the top pill once the story moves on; failure ->
│   │   why, in one line (a proxy or certificate, #894), + Use a model
│   │   file… + Try again
│   ├── Failed, or stalled 30 s without bytes, with every permission ready
│   │   -> Continue without a model (quiet), saying dictation won't work
│   │   until a model is added with Add model file…      // setup.without-model
│   │   └── Records completion and closes onboarding; Settings › General's
│   │       Speech model row says no model is installed (with the download's
│   │       error), + Download + Add model file…. Check: golden
│   │       `onboarding-no-model`, `onboarding_continue_without_model`
│   └── Change or delete the model later in Settings      // setup.delete-model
├── The demo: Without Hex (typed, with typos) -> With Hex (spoken) -> Continue
├── Quit part-way -> next launch resumes at the saved step  // setup.resume
│   └── Permissions are read again at launch: a setup they now finish (macOS's
│       Quit & Reopen after a grant) is passed; past setup, it waits until they do
├── Permissions                           // setup.checklist; skipped when granted
│   ├── macOS: "Let Hex hear you and type for you", Microphone and Accessibility
│   │   ├── No Input Monitoring row: Accessibility also lets the shortcut taps listen
│   │   ├── Microphone -> macOS prompt; refused -> "Open System Settings" + guide
│   │   ├── Accessibility -> Privacy pane + drag guide
│   │   │   ├── Retired Hex bundle IDs (`com.kitlangton.hex2`, `com.kitlangton.Hex`,
│   │   │   │   `com.kitlangton.Hex.debug`) cleared once in the background (`tccutil reset`,
│   │   │   │   ignoring `OSStatus -10814` when no bundle is on disk)
│   │   │   ├── Returned from System Settings still untrusted (2 s, or 6 s side-by-side) ->
│   │   │   │   "An older Hex entry can keep this off" + "Still grey? Fix it"
│   │   │   │   -> resets the running bundle ID + retired IDs, prompts, and reopens the guide
│   │   │   └── macOS 27 names the list "Device Control and Data Access"; the row follows
│   │   ├── On in System Settings, unusable until relaunch -> "Needs a restart"
│   │   │   └── Every permission on -> "Restart Hex" -> relaunch at setup  // setup.restart
│   │   └── Last grant -> 0.7 s beat -> the story moves on
│   ├── Windows: only while the microphone privacy switch is off: "Let Hex hear
│   │   you", one Microphone row -> Open Settings (ms-settings:privacy-microphone)
│   └── Linux: no page
├── Almost ready: the model still downloads -> waits, then Your turn
├── Your turn: the practice game          // setup.practice; every OS
│   ├── "Clear your messages": hold the saved shortcut and answer the lit card;
│   │   Hex's own dictation (macOS listener, Linux and Windows event log) holds
│   │   the keys, flies the live words, and lightning burns the answered card
│   ├── A short, silent, or failed try -> a hint under the field
│   ├── Listener running but still loading its model, microphone, or shortcuts
│   │   -> "Getting Hex ready to listen…" in place of the hold prompt until its
│   │   first session starts (desktop/host.rs `warming`)
│   │   // setup.warming; Check: `the_practice_page_waits_for_the_listener`
│   └── Done at any time -> Hex is ready  // practice is never required
├── Listener cannot start after setup     // setup.relaunch; event tap refused
│   ├── First time -> Hex records a marker, reopens itself, resumes at the practice
│   └── Within 2 min of that relaunch -> "Hex's shortcut didn't start." + Reopen Hex
└── Hex is ready -> where Hex lives (menu bar, system tray, or app launcher)
    ├── Try again -> back to the practice game
    └── Start using Hex -> records completion once setup is ready; closes onboarding
Run setup again (Labs) or `hex onboarding reset` -> the same story from the start,
on every build; settings, models, and permissions stay, so their pages pass
```

General > Speech model, selected model not downloaded     // settings.model-missing (#676)
├── Muted "Hex can't listen until the speech model has downloaded"
├── Downloading -> progress bar beside the model
└── No download running -> Download (the same ChooseModel); a failure shows its error

General > Speech model or Language
  -> Prepare candidate
     ├── Success -> Save selection
     └── Failure -> Preserve previous selection and show error

Permissions revoked later
  -> General > Permissions needed: Allow rows like onboarding's
     ├── Returned from System Settings still untrusted -> "Still grey? Fix it"
     └── On, waiting for a relaunch -> "Restart Hex"
```

Observed on macOS 26.6.2 and macOS 27.0 VMs (2026-10-31, SIP enabled): unprivileged
`/usr/bin/tccutil reset` from inside `anomaly.hex` clears `Accessibility`,
`ScreenCapture`, and `AudioCapture` entries for retired Hex bundles on disk with
no password prompt (`rc=64` / `OSStatus -10814` when absent from disk), and
resetting `anomaly.hex` followed by `AXIsProcessTrustedWithOptions` /
`CGRequestScreenCaptureAccess` replaces a stale `CDHash` entry that stayed grey
or untrusted in System Settings. Checks:
`tccutil_exit_codes_distinguish_cleared_absent_and_failed_bundles`,
`stale_tracker_waits_for_return_from_system_settings_and_ignores_touch_id_focus`,
`stale_accessibility_row_resets_permission_and_returns_to_waiting` in
[desktop/onboarding.rs](../../src/desktop/onboarding.rs), and
`stale_permissions_offer_fix_it_and_reset_entries_across_panes` in
[rune_ui/tests/panes.rs](../../src/rune_ui/tests/panes.rs). Scenes:
`mac-onboarding-stale`, `mac-settings-permissions-stale`.

Apple silicon, macOS 15+. Choices come from the compiled model/language catalog;
Apple Speech is currently excluded from the desktop picker.

```ts
General > Dictation > Speech model, then Language below it   // setup.language-select; every desktop root
├── Speech model -> Picker: Compare, then every model; never changes the language
│   ├── Compare -> "Speech model for Japanese": three columns from the saved
│   │   language's models, recommendations first; "{n} more models" -> every model
│   ├── Every model -> the saved language's models, Your model files, then a fold
│   │   "{n} more that don't speak Japanese"
│   ├── Model without the saved language -> Row: "Choosing it switches the
│   │   language to English"
│   └── Choose it -> ChooseModel(model, model's default: English, else its first)
│       ├── Warmed on silence without the vocabulary prompt; a warm-up that
│       │   stops at its output limit still counts as warm (same as the menu bar)
│       └── Saved -> Language row: "<Model> doesn't support Japanese, so the
│           language changed to English" until the next choice
├── Language -> Select of the saved model's languages (catalog order, Auto first
│   when it detects, including a model that takes no language input); one
│   language -> the field shows it, disabled
│   └── Choose -> ChooseModel(same model, language): the same preparation as a
│       model choice, so per-language assets (Apple Speech locales) install
└── The saved TranscriptionSelection is always a pair the model supports
```

Sources: [shared picker](../../src/rune_ui/picker.rs) (`choose`,
`choose_language`, `language_row`) and `picker_models`, `language_choices`,
`language_for` in [transcription/models.rs](../../src/transcription/models.rs).
Checks: `the_picker_lists_every_model_with_the_language_s_recommendations_first`,
`a_model_switch_keeps_a_supported_language_or_falls_back_to_the_model_s_default`,
`each_model_offers_exactly_the_languages_it_supports`, and the Rune tests
`a_model_without_the_saved_language_falls_back_to_its_own_visibly`,
`browsing_the_picker_never_changes_the_language`,
`changing_only_the_language_prepares_the_same_model_again`, and
`settings_opens_the_picker_and_names_the_model_and_language_apart`. Scenes:
`picker-japanese`, `picker-japanese-all`, `language-fallback`, `language-menu`. Fixture renders and
recorded requests only; a live model switch with a fallback has not been
observed. Apple Speech stays excluded, so its per-locale install through the
Language row is unexercised.

```ts
Choose a supported language              // setup.language-conditioning; macOS + Linux picker
├── Parakeet v3, Orukeet, and Parakeet Ultra -> Detect language from audio;
│   also offered under Auto; the selected language does not constrain output
├── Granite Speech 5.0 -> No language input; transcribes whichever of en, fr,
│   de, es, pt, it, ja it hears, so it is offered under Auto too (#1252)
├── Whisper -> Use selected language to guide decoding; accuracy is not guaranteed
├── Whisper (ivrit.ai's and NB-Whisper too), Qwen3-ASR 0.6B, QwenCleo-ASR 1.7B -> Vocabulary ->
│   context prompt (vocabulary.md); output that only repeats it is dropped
├── Hebrew -> ivrit.ai Whisper large-v3-turbo, always with the `he` hint (it
│   lost Whisper's language detection); stock Whisper turbo also offers it
├── Russian -> GigaAM v3 (Russian only, no language input); Parakeet v3 and
│   Whisper turbo also offer it
├── Norwegian -> NB-Whisper Medium, always with the `no` hint (Bokmål is all
│   Hex measured); stock Whisper turbo also offers it (#1150)
└── Bangla -> Whisper only, with the `bn` hint; Auto lets Whisper guess per clip
```

Whisper and Qwen3-ASR (including QwenCleo) read the shared
[Vocabulary](vocabulary.md) as their context prompt; recognition hints, a
separate per-model box, merged into it ([#129](https://github.com/anomalyco/hex/issues/129)).
Observed on apollo (Vulkan, 2026-10-31, LibriSpeech test-clean), when they were
still hints: with Qwen3-ASR 0.6B, "Kaffar, Rodolfo, Bragelonne, …" fixed Caffer,
Rudolpho, Redolfo, Rudolph, and Bragolon in 10 clips without changing the others
checked. Both families, on silence or noise, often return the prompt verbatim
(Qwen3-ASR 0.6B did on 3 s of silence and of noise). A segment whose text
equals the prompt after normalization (case, ASCII punctuation, spelled-out
digits; commas, semicolons and line breaks separate words) is removed, in
dictation, meetings, and the local API alike. The filter needs at least two
terms, so dictating the one listed name still pastes it; speech that contains
a term among other words is kept, and so is a repeat of only some terms.
Checks: `prompt_echo_is_dropped`, `speech_containing_the_hints_is_kept`,
`only_the_echo_segment_is_removed`, `single_hint_dictation_is_kept`, and
`hints_without_spaces_after_commas_are_matched` in
[transcription/gguf_session.rs](../../src/transcription/gguf_session.rs).

Models trained on subtitled video can end a clip with a subtitle credit nobody
said, such as "Sous-titrage Société Radio-Canada" from Cohere Transcribe in
French after a trailing pause (#1141). Every GGUF run and live stream drops a
known credit only when it is the very end of the text (letters and digits
compared, case and punctuation ignored), with its segment and words, in
dictation, meetings, and the local API alike; the same words elsewhere stay.
The list is in
[transcription/subtitle_credits.rs](../../src/transcription/subtitle_credits.rs).
Checks: `a_trailing_subtitle_credit_is_removed`,
`speech_without_a_trailing_credit_is_kept`, and
`a_trailing_subtitle_credit_is_dropped_from_text_segments_and_words`. No
inference reproduces the credit.

```ts
Settings picker > Compare                 // setup.model-card; shared by every desktop root
├── Column head -> Role (Recommended, Fewest wrong words, Fastest, Most languages,
│   Reads your vocabulary; from the catalog for the saved language), name, one-line blurb
├── Rows -> Wait when you stop (30 s ÷ realtime; "None" when it streams), Wrong
│   words or characters "1 in N" (benchmark in the saved language, else "Not
│   measured"; row hidden when none is measured), Languages, Reads your vocabulary,
│   Download size; one strictly best value is medium weight, ties get none
├── Action cell -> exactly one: In use dot, progress with Cancel, Use, or
│   Download <size>; Primary only on the recommended column when it can be chosen
├── The model in use or preparing replaces the last column when it would be left out
└── "{n} more models" -> Every model

Settings picker > Every model row          // fixed height; scrolls
├── Name, role, blurb, then "No wait · 1 in 50 words wrong · 25 languages · 2.5 GB"
├── Saved language unsupported -> Warning line naming the language it switches to
├── Trailing edge -> In use dot; progress with Cancel; else Use/Download; Delete when downloaded
├── Labs > Benchmark details (off) -> raw error rate "7.57% CER", a wrapping line:
│   language · dataset/split · source · conditions or "unknown" ·
│   speed qualified; "Most accurate" only between comparable records (labs.md)
├── Labs > Parakeet Ultra (off) -> its row with a Labs tag and Details: languages,  // setup.parakeet-ultra
│   measured memory, the CC BY 4.0 credit, notice, Source/License links, folded
│   published benchmarks "not measured in Hex" (labs.md)
├── Orukeet -> ordinary row with Details: languages, the CC BY-SA 4.0 credit  // setup.orukeet
│   and notice, Source/License links, folded published FLEURS benchmarks
│   "not measured in Hex"; never recommended or badged. Checks:
│   `orukeet_is_offered_for_exactly_parakeet_v3s_languages_and_never_badged`,
│   `orukeets_details_credit_its_base_and_cover_its_languages_once`
├── "Add model file…" in the header -> imported cards after the catalog:  // setup.custom-models
│   Imported · family · languages · Not benchmarked · file · licence · SHA-256
│   prefix; Rename, Remove (not the active
│   model); missing or changed -> warning line and no Use (own-models.md)
└── Found on this computer -> a scan of the Hugging Face cache when the picker  // setup.found-models
    opens: other compatible files at the top (Not tested by Hex, Use); a catalog
    model already downloaded there says so and offers Use; a found file's
    Delete is Forget (own-models.md)
```

Parakeet Unified English (chunked attention) and Nemotron 3.5 ASR (cache-aware)
are the models marked `streams`
(`only_the_streaming_parakeet_artifacts_are_marked_streaming`). The mark describes
the artifact; dictation still streams only after the load probe succeeds on a GPU
device ([Streaming Finish](dictation.md#streaming-finish)). The menu bar lists
names only and shows no tags. Scenes: `hex app -- --snap OUT --scene picker`
(also `picker-downloading`, `picker-japanese`, `picker-japanese-all`,
`picker-custom-models`, `picker-found`).

Nemotron 3.5 ASR (#834; NVIDIA's `nemotron-3.5-asr-streaming-0.6b`, OpenMDW-1.1)
streams in 27 of Hex's languages and detects the language with Auto: Hex sends
its language as the locale its prompt takes (`de` -> `de-DE`,
`runtime_language`), and Auto sends none. It is not anyone's recommendation:
Hex's own runs of transcribe.cpp's WER harness on apollo (Vulkan, 7045d1c) put
it behind Parakeet v3 on every set measured: LibriSpeech test-clean 3.05%
against 1.95%, FLEURS English 7.91% against 4.88%, and FLEURS German 10.39%
against 5.17% (8.77% and 10.46% with Auto). It is there to stream with the
language set, or across English and German in one model. Hex converted the
pinned `.nemo` itself; the shipped file is handy-computer's, whose 653 tensors
match that conversion byte for byte (`NEMOTRON_3_5_ASR_ARTIFACT`). Checks:
`nemotron_takes_its_languages_as_locales_and_detects_with_auto`. Not observed:
macOS (Metal), Windows, and Japanese, Korean, Chinese, or Vietnamese audio.

Parakeet v3 remains available for Portuguese and its other existing language
choices. Its card explains the limitation and suggests Whisper for
language-guided transcription. It is also offered under Auto: it detects the
language from the audio and takes no hint, as do Orukeet, Parakeet Ultra, and
Granite Speech 5.0. A hint-taking model without `stt.capability.lang_detect`
stays out of Auto. Multilingual audio support is not language conditioning:
the pinned v3 GGUF has no `stt.parakeet.prompt.num_prompts` metadata, which gates
language prompting in `transcribe-cpp-sys` 0.1.3.

Sources: [model catalog](../../src/transcription/models.rs),
[shared picker](../../src/rune_ui/picker.rs), and the runtime
options in [parakeet.rs](../../src/parakeet.rs) and
[transcription/local_transcriber.rs](../../src/transcription/local_transcriber.rs). The regression
`parakeet_v3_supports_portuguese_without_language_conditioning` checks that the
Portuguese selection remains valid and available, sends no runtime language
hint, and contrasts Whisper's `pt` hint. It does not run inference.
The regression was observed failing with the old metadata and passing after the
correction. The picker's layout is checked by Rune goldens (`rune-picker-*`), not on a
Mac screen.

**Unreproduced recognition report:** [#68](https://github.com/anomalyco/hex/issues/68)
reports Portuguese speech misrecognized as English. Source and pinned-model
metadata inspection establish the prompting limitation, not the cause or repair
of those transcripts. No v3 audio reproduction or transcription-quality fix has
been demonstrated.

Bangla (`bn`) is offered because Whisper large-v3-turbo was trained on it; no
other catalog model supports it, so Whisper is its only, recommended choice and a
Mac whose first preferred language is Bangla starts on Whisper. Auto is not a
substitute: in a manual test, Whisper's per-clip detection labeled Bangla speech
as Arabic, Turkish, and Spanish. `bangla_is_transcribed_by_whisper_with_its_language_hint`
checks the recommendation, the `bn` hint, and that Qwen3-ASR rejects it. It
runs no inference, and Bangla accuracy has not been measured.

Checks start in [desktop/onboarding.rs](../../src/desktop/onboarding.rs),
[transcription.rs](../../src/transcription.rs), and
[transcription/models.rs](../../src/transcription/models.rs). Setup/picker previews
prove rendering, not clean-account permission grants or model preparation.

The model step's list and its rules are in
[own-models.md](own-models.md#onboarding-choice) (scenes
`onboarding-model-found`, `-downloading`, `-recommended`, `-back`).

The tour state is the pure `desktop::onboarding::Flow` (steps, row states,
background download, practice outcomes, one-relaunch guard), fed by
[rune_ui/onboarding.rs](../../src/rune_ui/onboarding.rs); what the person sees
is the story in [rune_ui/onboarding_kit](../../src/rune_ui/onboarding_kit.rs),
opened at launch by `Ui::new` and replayed by Run setup again. The guide
beside System Settings is [permission_guide.m](../../native/permission_guide.m).
On Windows the Microphone row shows only while the microphone switch in
Settings › Privacy & security is off (device, user, or desktop apps;
`CapabilityAccessManager\ConsentStore\microphone` says `Deny`); setup
finishes once it is on (`a_switched_off_windows_microphone_adds_its_row_until_setup_ends`,
`windows_onboarding_asks_only_for_the_microphone_switch`; the registry read is
cross-compiled only, not run on a Windows PC).
Unit tests (`desktop::onboarding::tests`) cover step order, the download starting on the
welcome screen and retrying from any step, the auto-advance beat, live row
flips, a refused microphone, practice outcomes, resume, the relaunch guard and
resume marker, resuming past a setup the relaunch finished
(`resuming_at_setup_after_a_relaunch_passes_grants_that_are_now_real`),
grants waiting for a restart
(`a_grant_the_process_cannot_use_yet_waits_for_a_restart`,
`setup_offers_a_restart_once_every_permission_is_on`), an Accessibility grant
advancing setup without one (`an_accessibility_grant_applies_without_a_restart`),
the saved step (`the_tour_step_is_saved_for_the_next_launch`), and the upgrade
check (`a_fresh_install_onboards_without_the_identity_check`,
`an_upgrader_without_permissions_gets_the_tour_once`,
`an_upgrader_whose_setup_is_ready_skips_the_tour`; a real 2.3.1 to
`anomaly.hex` upgrade on a Mac is not yet observed). Rune tests drive the
first run (`a_new_install_opens_the_first_run`,
`the_mac_first_run_asks_each_permission_then_practices`,
`the_practice_game_follows_hex_s_own_dictation_on_every_os`,
`a_listener_that_will_not_start_relaunches_once_then_asks_to_reopen`,
`a_refused_microphone_points_to_system_settings`,
`a_grant_waiting_for_a_restart_makes_setup_restart_hex`,
`onboarding_downloads_the_model_through_the_service`,
`a_found_model_moves_on_without_a_download_and_back_returns_to_it`), and every
page is a scene whose goldens `scripts/check-onboarding.sh` checks (its
`SCENES` list in `rune_ui/tests/onboarding_check.rs`). Scenes are fixtures
played on the story's own clock to one settled moment, not native proof.
The default model comes from `desktop::i18n::preferred_language` (macOS:
`NSLocale.preferredLanguages`) through `recommended_selection`; existing
settings keep their saved model.
Observed on Linux (apollo, 2026-11-01): a clean first run in a nested
session, from Setting up Hex through the real download to Hex is ready. Not
yet observed: a clean-account Mac run with real grants, a real practice
answer on macOS or Windows, or a real self-relaunch after an event-tap
refusal. The story's demo cards and the game's messages stay in English in
every interface language; its headings, buttons, and hints are translated.

```ts
Menu bar > Speech Model                   // setup.quick-model-switch; macOS
├── Downloaded models -> Last selected language shown beside each model
├── Choose -> Verify installed artifact -> Prewarm -> Save selection
│   ├── Prewarm runs silence without the vocabulary prompt, so a term list
│   │   can't be read back to Qwen3-ASR's token cap
│   ├── Warm-up still stops at its output limit (Qwen3-ASR 1.7B) -> Counts as
│   │   warm; saved (same as Linux and Windows, and as the Settings picker)
│   │   // gguf_session::RunKind; checks:
│   │   // a_warmup_leaves_the_vocabulary_out_and_counts_when_truncated,
│   │   // a_decode_stopped_at_its_output_limit_is_recognised_through_its_context
│   │   // Proof limit: the classifier test covers only error recognition and
│   │   // the warm-up test only the option choice; accepting a Mac prewarm has
│   │   // no automated check, and no Mac run was observed.
│   └── Failure -> Previous selection remains; open Settings for the error
├── Preparing -> Cancel Model Switch      // never cancels dictation/output jobs
└── Manage Models… -> Settings picker      // downloads; languages in the Language row
```

The menu does not open Settings merely to switch models, download a missing model,
or replace a corrupt artifact. Each model retains one last-used language/hints
selection in `transcription_recents`; the current selection takes precedence.
Older settings start with no remembered alternatives. Select an alternative's
language in Settings' Language row once to establish it. No additional model is loaded at startup
just to populate the menu, and the checkmark means selected, not live-worker readiness.

[transcription/preparation.rs](../../src/transcription/preparation.rs) is shared
by Settings and the menu bar: one worker plus one replaceable pending choice.
Closing Settings does not abandon preparation. A superseded/cancelled completion
cannot save, and delivered menu controls are drained before accepting completions.
The desktop root commits through an existing editor to retain unrelated unsaved
settings, or through persisted settings when no window exists.

Sources: [macos_rune/status_item.rs](../../src/macos_rune/status_item.rs) and
[macos_rune.rs](../../src/macos_rune.rs). Key checks are
`preparation_has_one_worker_and_only_keeps_the_latest_pending_choice`,
`cancelling_a_completed_unpolled_preparation_never_commits_it`,
`model_switches_remember_language_and_hints_without_changing_other_settings`, and
`failed_model_selection_save_preserves_selection_recents_and_unsaved_edits` in
the preparation owner and [app_settings.rs](../../src/app_settings.rs).
Additional checks cover worker failure/retry, installed-only checksum verification,
and menu choice/tag projection. These do not establish a physical menu click or
an installed-app switch. Remembering a language does not change the language
conditioning limitations described above.

**Observed September 4, 2026 (2.1.16), existing model picker:** all 455 Rust tests
passed in debug and release, with ten opt-in tests skipped per profile. All twelve
keyboard-layout child scenarios passed per profile; formatting, strict
all-target/all-feature Clippy in both profiles, and the release build passed.
The Cohere fixture inference check was run separately; see
[dictation model windows](dictation.md#ongoing-jobs-and-output). Native menu
interaction, installed-app switching, and Linux runtime behavior were not exercised.

### Switch models with keyboard language

```ts
Labs > Switch model with keyboard language      // setup.input-source-model; macOS, Windows, Linux X11; off by default; General shows the mappings while on
├── Each enabled keyboard input source -> Don't switch, or a downloaded model and language
│   ├── New mapping -> The source's own language when the model has it
│   ├── One-language model -> Its language shown fixed
│   └── New/unmapped source -> Don't switch; keep the current model
├── Source changes (shortcut, Input menu, automatic app/document switch)
│   └── 300 ms after the last change -> Existing installed-only picker preparation
│       ├── Same model and language already selected -> Nothing loads
│       ├── Success -> Save the selection, keeping the model's remembered hints
│       └── Failure -> Existing picker error; no download or guessed fallback
├── Manual model choice -> Supersede pending automatic work; do not reapply at recording start
├── Enable -> Apply the current source's mapping once it is observed
├── Edit the active mapping -> Apply now
├── Disable -> Cancel automatic work; stop observing; leave the selected model alone
└── Relaunch -> Observe current source without applying it; preserve the manual choice
```

Mappings are keyed by native input-source IDs (macOS input-source IDs, Windows
keyboard layout handles, X11 XKB layouts such as `de(neo)`), not inferred speech languages or
physical keyboard keys, and store a model with a language it supports. Only
downloaded models from the picker are offered (Apple Speech is never mapped);
downloads stay in the picker. A mapped model that is no longer downloaded is
labeled Not installed and can be cleared; a mapping to a model or language Hex no
longer offers is dropped when settings load. Removing a source hides its row
without erasing its saved mapping.

Hex reads input sources only while the setting is on: the Windows runtime's poll
of the foreground window's layout and the Linux host's XKB connection (polled
every 250 ms) exist only then, and the macOS main-thread TIS
distributed-notification observer, which always runs to rebuild the keyboard
layout snapshot ([recovery](recovery.md#keyboard-layout-resolution)), reports
the selected source only then; all work without a Settings window, and the
window lists enabled sources only then. Windows lists the language bar's layouts, named by language and
keyboard. On Linux the row appears only in X11 sessions: Wayland compositors keep
the active layout to themselves, with no general protocol (IBus and Fcitx report
input-method engines, not the compositor's layout), so Wayland is unavailable. Source changes cancel
obsolete automatic work but keep manual preparation until a mapped choice is
due. An unmapped source cancels only automatic work. This reuses the shared model
selection, recording/stream fallback, and meeting-engine behavior; it adds no
recording-time routing, model cache, or notifications.

Sources: observers for [macOS](../../src/keyboard_input_sources/macos.rs),
[Windows](../../src/keyboard_input_sources/windows.rs), and
[X11](../../src/keyboard_input_sources/x11.rs),
[switch policy](../../src/transcription/keyboard_model_switching.rs),
runtimes ([macOS](../../src/macos_rune.rs),
[Windows](../../src/windows/app.rs), [Linux](../../src/linux/app.rs)), and
[settings UI](../../src/rune_ui/settings.rs).
Checks: the switch policy and settings/CLI validation tests, the runtime
scheduling tests in [macos_rune/tests.rs](../../src/macos_rune/tests.rs) (macOS),
the settings interaction tests in
[keyboard_model_tests.rs](../../src/rune_ui/keyboard_model_tests.rs), and the
`mac-input-source-models` golden, and the XKB symbols-name tests in the X11
observer. No check delivers native input-source notifications or switches
sources through the Input menu or a shortcut.

**Observed October 30, 2026, X11 observer:** on a headless Xvnc with
`setxkbmap -layout us,ru,de -variant ,,neo`, Hex listed English (US), Russian,
and German (Neo 2) as `us`, `ru`, `de(neo)` with languages en, ru, de, and
reported `us`, then `ru` after an XKB group lock. The Windows observer and both
new runtime paths compile (x64 clippy) but were not run on Windows or a live X11
desktop.

### Delete downloaded models

```ts
Settings picker > Every model > Downloaded row   // setup.delete-model; macOS
├── Delete -> "Delete <size>?" -> Delete again
│   ├── Success -> Model file, receipt, and partial download removed; row offers Download
│   └── Download in progress -> Nothing removed; "Model could not be deleted." with reason
├── Selected dictation model (any language) -> No Delete
├── Preparing or Apple Speech model -> No Delete
├── Model choice or closing the picker -> Confirmation cleared
└── Labs > Downloaded models only (off) -> "Downloaded models take <size>" (Hex's own   // labs.picker-downloaded
    files) + "Only downloaded models", which leaves just the downloaded rows, each
    with its Delete; reset when the picker opens (labs.md, #1018)
```

Deletion takes the shared model-directory download lock without waiting, so it
cannot race a Settings, menu-bar, or local API download. The menu bar drops a
deleted model because it reads installation state when it builds the menu. Linux
Settings does not offer deletion: the service owns model files and has no delete
request yet.

Sources: [shared picker](../../src/rune_ui/picker.rs) and `remove_installed`
in [transcription/models.rs](../../src/transcription/models.rs). The check is
`removing_a_model_deletes_its_artifacts_unless_a_download_is_active` (real files
and lock); no Rune test covers the picker's two-step confirmation yet.

**Known gap:** `is_installed` checks size, not checksum. Not every native-load
path requires verification. Picker Active means selected/installed, not live
worker readiness; a later worker-load failure does not roll back saved selection.

## Dictate

```ts
Saved shortcut                           // dictation; Option by default; Windows and X11 accept mouse buttons, macOS with Labs > Mouse shortcuts
├── Hold -> Speak -> Release
├── Tap, tap -> Speak hands-free -> Press again   // Shortcut mode: Hold or double-tap
├── Tap mode (Toggle) -> Cancel shortcut, Escape by default, while recording
│   -> Discarded: nothing pasted or saved; transcribing jobs finish (#1022)
├── Hold modes -> no cancel: Escape reaches the app, every dictation pastes (#786)
└── macOS recorder: a key alone needs a modifier, except F1–F20 and Home, End,
    Page Up, Page Down, Help (#1138; keys_without_modifiers_need_one_except_function_keys)

General > Shortcut mode -> Hold or double-tap (default), Hold, Tap, Double-tap
  -> Modal: each choice with its keys and a looping key/recording timeline
  -> Double-tap needs a key binding; with a lone modifier it says so

Labs > Second dictation shortcut (macOS, off)   // labs.md; issue 627
  -> General shows "Second dictation shortcut" under Shortcut (unbound, clearable)
  -> it dictates like the shortcut: Hold/Toggle, double-tap lock, release
  -> the shortcut that starts a capture finishes it; the other stands down meanwhile

Labs > Terminal paste shortcut (Windows, Linux; off)   // labs.md
  -> General shows "Terminal paste shortcut" under Shortcut (unbound, clearable)
  -> it dictates like the shortcut and always pastes with Ctrl+Shift+V
  -> Paste last dictation keeps the global Terminal paste setting

Labs > Caps Lock dictation (macOS, off)   // labs.md
  -> a Caps Lock press starts a hands-free capture; the next press finishes it
  -> needs both the HID Caps Lock key and its AlphaShift edge; remapped to
     Control or Escape changes no AlphaShift, so it never toggles dictation
  -> Caps Lock stays off so typing is unchanged; its light is solid while any
     capture records (normal-shortcut and Annotate too), blinks while Hex
     transcribes or processes, off when done
  -> Escape cancels in any mode; in Toggle mode only Escape cancels a Caps
     Lock capture; the normal shortcut stands down while one runs; off or
     quit clears the light and restores Caps Lock

Labs > Unload model when idle (off; 5/15/30/60 min, default 15)   // labs.md; transcription/engine
  -> nothing used the model that long and nothing records -> the model unloads
  -> next press, meeting chunk, or API request -> reloads first; audio waits, nothing lost
```

The [detailed map](dictation.md) covers timing, successive captures, output
commitment, named checks, and native-proof gaps.
The macOS CLI entry is `hex listen`, with an optional authoritative `--device`.
Do not start it beside an existing listener.

## Use The Result

```ts
Completed output                         // output
├── Paste -> Current focus, in submission order
│   ├── Previous clipboard restored 500 ms later, unless something newer replaced it
│   ├── Windows -> the transcript is marked out of clipboard history (Win+V) and cloud sync (#851)
│   ├── Labs > Keep dictation on clipboard (macOS, Windows, off) -> the dictation stays on the clipboard (labs.md)
│   │   └── Windows -> kept as an ordinary entry, in history too
│   └── Labs > Copy dictation only (macOS, Windows, off) -> clipboard only: no paste, typing, or restore (labs.md)
├── Paste Last Dictation                  // menu or Option-Shift-V / saved binding
│   ├── Last completed dictation
│   └── macOS menu item shows the saved binding (⌥⇧V by default); unbound,
│       modifier-only, or a mouse button -> no shortcut shown
└── History
    ├── Inspect / search / copy (row hover or the card) / delete
    ├── Retention ("Kept 7 days ▾" menu) -> Seven days by default, always hard-capped
    ├── Off ("History off") -> Stop new entries; preserve existing entries
    ├── Same menu > Clear all history… -> asks in the header; only the red confirm clears
    └── Labs > Keep recordings (off) -> new entries keep their 16 kHz audio   // labs.keep-recordings
        -> also "Keep audio too" in the retention menu while the lab is offered
        -> the card's "Transcribe again" menu runs it through any installed model
        -> the new text with Copy and Replace; Clear all and retention delete the audio (labs.md, #1032)
        -> Replace keeps the old text as one of 5 earlier versions: Copy, Use this version (#1177)
Every entry since #1177 -> "Speech model" fact: the model that made its text (+ language when chosen)
```

Checks in [paste.rs](../../src/paste.rs), [parakeet.rs](../../src/parakeet.rs), and
[history.rs](../../src/history.rs) cover output and retention boundaries.
They do not prove that an arbitrary target consumed its clipboard contents.
Send is not an atomic target-app transaction. Chromium 155 and 156 get the text
typed instead of pasted for a [Chrome paste bug](dictation.md#ongoing-jobs-and-output).

**Easy to misread:** History Off does not stop diagnostic logging. Diagnostic
logs are separate and currently unbounded on disk, but they never hold what was
said: `live.ndjson` records each dictation's phase and timing with its text
left out (`events::VoiceEvent::without_words`,
`the_log_never_keeps_what_was_said`), so History is the only place dictated
words are kept. They are readable
only by their owner on macOS and Linux: Hex creates and tightens the logs folder
to 0700 and `live.ndjson` and `process.log` to 0600 (`app_paths::create_private_dir`,
`open_private_append`; `the_logs_folder_is_private`, `the_log_is_private`;
tightening is best-effort, never fatal), so other
accounts can't read them through a world-readable home folder.

The shared `EventReader` streams a file-length snapshot in 16 KiB chunks rather
than copying the entire log into memory. It retains up to 1,024 event rows and
the latest context, even when that context predates the retained rows.
Diagnostic records over 1 MiB are skipped through their terminating newline;
the original log and dictation output remain intact. Partial UTF-8 records carry
over to the next refresh, and truncation or file replacement resets the reader.

Checks: the `reader_*` regressions in [desktop/events.rs](../../src/desktop/events.rs). These
exercise synthetic files, not an installed app. The GPUI window's
on-demand-loading regression went with GPUI; when the Rune window reads the
log is not covered by a test. A first Commands/Activity refresh still scans the
full log snapshot; disk rotation and background parsing remain separate work.

Observed September 26, 2026: the reader and pane-loading regressions passed.
An optimized isolated harness using the production reader and a synthetic
64 MiB log reduced peak process RSS from about 204 MB to 2.4 MB; refresh took
254 ms before and 222 ms after. This is reader evidence, not a native Settings
latency measurement.

## Adjust Behavior

```ts
Sidebar                                  // settings.nav
├── General, Output, History, then each feature that is on: Meetings and
│   Voice Filter (Labs toggles), Annotate, Playground (Labs, an item)
└── Labs -> a store of unfinished features, each off by default: search, categories, cards with their own settings inline; hex://labs/<feature> opens its group (labs.md)
    // Command-1…9 follow the panes' order (labs::order)

General                                  // settings
├── Basics card -> Shortcut, Speech model, Language, Microphone, Launch at login
│   ├── Each row leads with an icon; choices are inline dropdowns, not segments
│   ├── Language -> fixed text for a one-language model, else a dropdown
│   └── A row that hides or shows (Advanced, a microphone error)
│       -> Folds under a clip as the rows below slide over it; no fade in place
├── Microphone -> Value names the input in use, with live level bars // settings.microphone
│   ├── "System default · Kit Pods", or a saved device
│   ├── Level bars -> peak of the audio Hex receives, ~30 fps while General shows them above rest // input_level
│   │   ├── Peak measured around each chunk's mean, so a microphone's DC offset never lights them
│   │   ├── Read from one atomic the audio owner stores per chunk; never opens or blocks the mic
│   │   ├── Quiet room (bars at rest) -> no 30 fps frames; the window's 250 ms tick notices a rise.
│   │   │   An open General window with a warm mic woke 34.5 times a second in a quiet room before
│   │   │   (dev-364, headless sway on apollo, Oct 31) and 4.1 after; macOS was reasoned about only
│   │   ├── No audio for 250 ms (released while idle, failed, not allowed) -> flat, dimmed
│   │   └── Scenes: mac-settings-mic-level (a recorded WAV, `--video`), mac-settings-mic-released
│   │       // check: the_microphone_level_follows_the_voice (Linux)
│   ├── Re-listed on a device or default change while General shows (Windows: also every 3 s)
│   ├── System default -> The OS default input, followed live
│   │   ├── Default changes -> Switch once capture is idle; new stream opens before the old drops
│   │   ├── Default is Hex Microphone -> Built-in mic, else first real input; row says so
│   │   ├── Stand-ins never include virtual or aggregate inputs (macOS: e.g. Microsoft Teams Audio)
│   │   │   ├── Nothing real left -> "lid is closed" / "no microphone" error, not silence
│   │   │   └── A virtual default, saved device, or `--device` is still used as named
│   │   └── Default is Bluetooth (Advanced switch on) -> Built-in mic, else first wired/USB input
│   │       ├── Row says "KIT PODS is Bluetooth, so Hex listens to MacBook Pro Microphone"
│   │       ├── Windows: WASAPI `BTHENUM`, or a headset/handset endpoint of unknown bus (`BTHHFENUM`)
│   │       ├── Linux: a `bluez5` default source; Hex opens ALSA's default, then moves its own
│   │       │   stream to the PCI input, else the first other hardware source (pactl, PipeWire);
│   │       │   the stream is marked so WirePlumber never remembers the move. A Bluetooth
│   │       │   default chosen while Hex's stream is open is followed until the next open
│   │       ├── No other usable input (or lid closed, Bluetooth only) -> the Bluetooth default
│   │       ├── A saved Bluetooth mic or `--device` is always used as named
│   │       └── Dictation only: meeting recording keeps the Bluetooth default, which
│   │           a call app on the headset is using (#1086;
│   │           a_meeting_keeps_the_bluetooth_default_a_call_uses)
│   ├── Saved device missing -> System default stands in, value names it
│   │   ├── Row says "SHURE MV7 isn't connected; using your Mac's input"
│   │   ├── Menu keeps "SHURE MV7" checked with a faint "not connected"
│   │   └── Device returns -> Switch back once idle; the setting never changes
│   ├── Menu lists real inputs only                                // never Hex Microphone
│   └── Open menu covers the controls beneath it; click elsewhere dismisses
├── Advanced ▸ -> one fold per pane, closed by default, even while a setting in it is in use
│   ├── Collapse -> drops a pending release confirmation
│   ├── Dictation -> Shortcut side, keyboard-language mappings (while its Labs toggle is on),
│   │   Use the Mac's microphone instead of Bluetooth (Linux, Windows: "Use another
│   │   microphone instead of Bluetooth")
│   ├── Output -> Paste last shortcut, While dictating, Dictation HUD, Terminal paste (Linux)
│   │   (Dictate without processing sits in the Output pane; see dictation.md)
│   └── Application -> Enable on launch, Show Dock icon, Sound volume, Sounds,
│       Interface language, Update channel (self-updating installs)
│       // Interface language System: macOS's first preferred language (zh-Hans -> 简体中文,
│       // zh-Hant, zh-TW, zh-HK, zh-MO -> 繁體中文), Linux LC_ALL/LC_MESSAGES/LANG
│       // (zh_TW.UTF-8 -> 繁體中文), Windows the user locale; unknown -> English
│       // Text language (rune::text::set_language): 日本語 -> ja, 简体中文 -> zh-Hans,
│       // 繁體中文 -> zh-Hant (zh-HK in Hong Kong/Macau); another interface on a
│       // ja/ko/zh system locale -> that locale. Han text falls back to its fonts
│       // (Hiragino Sans, Yu Gothic, Noto Sans CJK JP for ja) and shapes with its
│       // locl forms (#1382)
├── Use the Mac's / another microphone instead of Bluetooth (Advanced, on)
│   └── Off -> System default follows a Bluetooth default too; re-resolves once idle
├── Labs > Share dictation shortcut with other apps (Windows, off)
│   └── On -> the hook passes the shortcut's key-down/up on instead of consuming it
├── Terminal paste (Advanced on Linux, Labs on Windows; off)
│   └── On -> paste sends Ctrl+Shift+V instead of Ctrl+V; applies to the next paste
├── Labs > Terminal paste shortcut (Windows, Linux; off)
│   └── On -> General shows its recorder; a capture it starts pastes with
│       Ctrl+Shift+V whatever Terminal paste says
├── Turn off the mic when not dictating (General, under Microphone; off)   // release_microphone_while_idle
│   ├── One switch, no confirmation; its line says what the state means (issue 61), on every OS
│   ├── Off (default) -> "The mic stays ready so dictation starts instantly; your system shows it in use"
│   │   Open while idle; pre-roll available
│   └── On -> "The mic opens only while you dictate; the first moment can be clipped"
│       Open on press; no pre-roll; startup delay
├── While dictating -> Mute / Pause media / Lower audio / Do nothing
│   ├── Lower audio -> Lower to slider, 0–100% of the previous volume (default 20%)
│   ├── Intentional capture only, not ordinary shortcut chords
│   ├── Pause/resume only players already reported running by macOS
│   ├── Linux restore -> Waits 300 ms; a new capture keeps it suppressed // double-tap lock
│   └── Labs > Also pause the browser (macOS, Pause media only; off by default); its permission row shows here while on
│       ├── Turning it on without System Audio Recording -> ask now, never while dictating
│       ├── On -> Screen & System Audio Recording row: Allow / waiting / Allowed
│       └── Dictation -> browser_media sequence below
├── Sound volume -> immediate feedback; zero suppresses tones
│   └── Default output changes or disappears (macOS) -> next tone reopens on the current default
├── Sounds -> Sheet: hear each set on Hex's real HUD, pick one
└── Linux / Windows -> listener card and desktop commands below the basics

Pause media + Also pause the browser     // browser_media
├── A browser uses the microphone (a call) -> leave it alone
├── No browser process has its audio output open -> leave it alone (no tap)
├── Listen to those processes' own output (private process tap), 3 windows of 100 ms
│   └── Any window at or below -60 dBFS, or no level -> leave it alone
├── Send the system pause command (MediaRemote 1; never the toggle)
├── Poll 5 windows of 100 ms
│   ├── 2 windows in a row below -90 dBFS -> Hex paused it
│   └── Otherwise (another tab, a sound that ended later, or the pause went elsewhere) -> no play later
└── Dictation ends and Hex paused it -> first, before the players: listen 300 ms to every browser process
    ├── Above -60 dBFS (you started it again) -> no play
    ├── No level, a browser runs and every one has closed its output -> still paused: play
    │   (a pause longer than about 10 s closes the output; a tap on it delivers nothing)
    ├── No level otherwise (browser quit, tap failed) -> no play
    └── Otherwise -> send the system play command (MediaRemote 0), then poll 20 windows of 100 ms
        ├── A window above -60 dBFS -> resumed
        ├── No level before the first level (the output is opening again) -> keep polling
        ├── No level after a level -> stop listening
        ├── No browser output open after 10 windows (tab closed: the play reached another app) -> send pause once
        ├── Never a level and a browser opened its output -> stop listening
        └── Silent throughout (tab closed or reloaded: the play reached another app) -> send pause once

Intentional recording                    // recording.environment, idle_sleep
├── Prevent idle sleep
│   ├── macOS -> IOKit no-idle-sleep assertion
│   ├── Linux -> logind `idle:sleep` inhibitor; `idle` only where polkit refuses a
│   │   process outside the seat session (Hex's user service) to block sleep
│   ├── Windows -> system-required power request (from the press, as its While dictating does)
│   └── Failure -> Warn and continue recording
└── Finish/cancel -> Release it
    // no caffeinate child process or executable-policy dialog
```

Feedback tones keep their output open briefly after the last one: about ten
seconds on macOS, where an open output is a live CoreAudio client coreaudiod
services and holds a sleep assertion for, and five idle minutes on Windows.
On Linux, where an open output stays listed in the desktop's sound controls,
it closes about a second after the last tone ends
([#720](https://github.com/anomalyco/hex/issues/720)); the existing
`output_opens_for_a_tone_and_releases_after_the_idle_grace` check covers the
grace logic, not a real PipeWire stream list. A fatal stream error (the output
became unavailable or invalid) sets a per-sink flag from the stream error
callback, which may run on a CoreAudio thread; overload glitches (`Xrun`) do
not. The worker drops a flagged sink on its next 250 ms observation or next
tone, and the next tone opens a fresh one
(`an_errored_sink_is_dropped_and_reopened_on_the_next_tone`). A default-output
change is not fatal on macOS, where cpal reports `DeviceChanged` after the
default-output unit has already rerouted the stream; on Windows cpal's WASAPI
backend never rebinds, so a default-device change ends the stream with
`StreamInvalidated`, which is fatal: the output is dropped and the next tone
opens the new default. On macOS the CoreAudio listeners on the default output,
the device list, and an audio service restart advance an output route
generation; the next tone drops an output opened under an older one and opens
the current default at once, so unplugging a USB or Bluetooth output no longer
silences tones until Hex restarts
([#428](https://github.com/anomalyco/hex/issues/428)). Check:
`a_route_change_reopens_the_output_on_the_current_default` in
[feedback.rs](../../src/feedback.rs), with injected opens; a real unplug has not
been observed by a test.

**Lower audio** (`recording.environment.duck`) is an explicit fourth
choice, off by default, on every OS. The slider appears only while selected and saves on
release. It remembers its value across behavior changes. Intentional captures
lower the original output channels to that fraction in about 120 ms,
preserving balance and silent channels, then restore on finish, cancel, or
orderly shutdown by easing back over one second. Changes apply to the next
capture. The stop tone waits for restoration. A manually changed volume after
settling is left alone;
a newly selected output device is not lowered. Fixed-volume devices and outputs
whose channel balance cannot be safely restored are left unchanged. Meetings do
not acquire this dictation environment. Linux lowers the default sink's
channels through `pactl` (one volume through `wpctl` without it), so Hex's own
tones are lowered too and the stop tone waits, as on macOS. Windows lowers
every other app's audio session instead, leaving Hex's tones at full volume;
sessions that start while recording are left alone.

HEX snapshots the volumes and applies a smooth ramp in
[audio/output_ducking.rs](../../src/audio/output_ducking.rs) over the CoreAudio adapter in
[ducking.rs](../../src/audio/recording_environment/ducking.rs), the sink adapter in
[linux/audio_control.rs](../../src/linux/audio_control.rs), and the session
adapter in [windows/audio_control.rs](../../src/windows/audio_control.rs). The worker runs inside the app and cannot restore volume after a
hard process crash or SIGKILL. Changes during the ramp itself cannot be
distinguished from delayed Bluetooth acknowledgements. With the shorter
lowering ramp, a Bluetooth output that acknowledges an earlier ramp step late
could look like a manual change and stay lowered; not observed on a device.
Fake-device tests cover relative volume, balance, silence, interrupted ramps,
manual changes after settling, and partial-write rollback; settings/UI tests
cover defaults, bounds, persistence, conditional visibility and platform gating.

**Changed November 7, 2026 (issue 1468):** with Lower audio, lowering used to
take a full second, so the start of every dictation stayed loud, and Duck also
kept the 450 ms pre-roll, which captured audio from before the lower took
effect. It now reaches the ducked volume in about 120 ms (`LOWER` in
[audio/output_ducking.rs](../../src/audio/output_ducking.rs)), keeps no
pre-roll like Mute and Pause media, and still eases back over one second when
restored. Checks: `ducking_preserves_balance_silence_and_restores` and
`lowering_finishes_at_its_own_duration_while_restoring_is_still_ramping` in
that module, and `suppressing_audio_while_dictating_keeps_no_pre_roll` in
[dictation.rs](../../src/dictation.rs) (constructed audio; not run on a device
or physical output).

On 2026-10-25, the macOS logic/settings checks and strict Clippy passed.
The seven reference images were regenerated on Ubuntu 24.04 ARM64 with Mesa
lavapipe, using the unmodified Linux-only Rune test module and
`HEX_UPDATE_GOLDENS=1 scripts/check ducking`. A subsequent
`scripts/check rune_ui --nocapture` without the update flag passed all 113 tests,
including the unchanged control goldens; no golden skipped for lack of a GPU
adapter. The five existing images differ from the base branch above the existing
12-level channel tolerance only within the While dictating control; the sidebar,
Quit Hex, and shortcut layouts remain unchanged. Dark and light ducking previews
were visually inspected. The earlier macOS-generated references were replaced
because these fixtures retain build-OS-dependent layout and defaults. This
verifies headless Linux rendering, not audible playback, physical devices,
crash recovery, or native Windows execution.

The Linux and Windows ports (2026-10-30) were checked by unit tests (pactl and
wpctl volume parsing, the Linux Bluetooth stand-in choice), the Settings test
that Lower audio and its slider appear on every OS, Linux Clippy, and Windows
x86_64 cross-Clippy. On apollo (niri, PipeWire), the ignored
`native_idle_sleep_prevention_acquires_and_releases` took a logind `idle`
inhibitor (polkit refused `idle:sleep` over SSH), and a marked `arecord`
stream moved with `pactl move-source-output` without WirePlumber storing the
target. Not verified: audible Linux or Windows ducking, Windows idle-sleep
and Bluetooth detection on hardware, a real Bluetooth headset on Linux, and
Windows ARM64 (its cross-build stops in `rquickjs-sys` on this host).

Successful settings saves persist and project changes at safe runtime boundaries.
An invalid saved field falls back independently at launch, with its key and
reason in the log; the next save preserves the original first. See
[settings recovery](recovery.md#load-settings-with-invalid-fields).
Immediate settings controls commit a candidate before changing the saved UI
choice or applying dependent native/history effects. A failed shortcut save
stays in capture instead of showing Saved; failed retention saves keep the old
choice and surface the error. Failed debounced text saves retain their drafts
for a subsequent edit or close-time flush; no Rune test injects a save failure
yet.
Persistence, conflict, and ownership checks live in
[app_settings.rs](../../src/app_settings.rs),
[audio/recording_environment.rs](../../src/audio/recording_environment.rs), and
[audio.rs](../../src/audio.rs). Settings previews do not prove physical device
switching or native mute support; muting is best-effort, not universal. The
recording-environment ownership checks cover overlapping acquisition and release,
not a managed-device policy or native power assertion. The pause-script regression
checks that inactive players are absent before AppleScript application resolution;
when no supported player is running, acquisition skips AppleScript entirely.
The script check does not prove subprocess avoidance or exercise Music, Spotify,
or VLC through Automation.
The browser pause runs after the AppleScript players and only when System Audio
Recording is reported granted (an unknown answer skips it, because the tap would
open the macOS dialog mid-dictation). macOS 15.4 and later no longer tell other apps
what is playing, so Hex hears the browsers through a private process tap
(aggregate `anomaly.hex.browser-tap.<pid>`, apart from the meeting tap) and sends
play only after it saw the browser go silent right after its own pause. The
sequence checks in [audio/browser_media.rs](../../src/audio/browser_media.rs) run on fakes:
`a_confirmed_pause_is_resumed`, `two_sounding_tabs_never_get_play`,
`a_browser_already_playing_gets_no_play`, `a_call_is_never_paused`,
`no_level_sends_nothing`, `a_play_the_browser_never_takes_is_paused_again`,
`a_browser_that_closed_its_output_gets_play`,
`a_closed_browser_that_never_opens_after_play_is_paused_again`; the ignored `native_tap_hears_a_playing_browser`
heard Brave at -25.4 dBFS on macOS 27.0.1 (2026-10-28). Which app receives the
system command is macOS's Now Playing choice; Safari is heard only when its
WebKit process reports Safari's name. An opt-in native
assertion smoke exercises IOKit without starting microphone capture.

**Changed after 2.3.1:** Automatic was a compiled preference list (Universal
Audio Thunderbolt, Studio Display Microphone), then the macOS default with a
Bluetooth default swapped for the built-in microphone (the 2.1.22 fix for
[#106](https://github.com/anomalyco/hex/issues/106) and
[#88](https://github.com/anomalyco/hex/issues/88)). That skipped a user's
AirPods default, and Hex Microphone then passed the wrong microphone on to
calls. Automatic is now System default: the OS default input, followed live.
A Bluetooth default (CoreAudio transport Bluetooth or Bluetooth LE) is passed
over for the built-in microphone, else the first other real input, so AirPods
keep their music out of call quality while you dictate; it is kept when nothing
else is usable. Only System default is affected: a saved Bluetooth microphone
and `--device` are honoured, and General › Advanced › "Use the Mac's microphone
instead of Bluetooth" (`avoid_bluetooth_microphone`, default on) turns it off.
Hex Microphone carries whichever input Hex records from. Linux and Windows
don't detect Bluetooth inputs yet and follow the default. Checks in
[audio.rs](../../src/audio.rs):
`system_default_passes_over_a_bluetooth_default`,
`a_bluetooth_default_is_kept_when_nothing_else_is_usable`,
`a_virtual_input_never_stands_in` (CoreAudio transport Virtual, Aggregate, or
AutoAggregate; [#429](https://github.com/anomalyco/hex/issues/429)),
`inputs_resolve_to_the_selection_then_the_system_default`,
`device_override_wins_and_never_falls_back`,
`a_default_change_switches_only_while_capture_is_idle`, and
`a_missing_selection_uses_the_default_until_it_returns`, with injected device
lists and opens; scene `mac-microphone-bluetooth` shows the row and switch. Not
verified: the CoreAudio listeners against a real default change, the transport
read against real AirPods, or Windows' default-change report on a real PC.

**Fixed after 2.1.20:** through 2.1.20 the microphone menu was painted without
occluding the panel, so hovering or clicking a device also reached the setting
under the pointer ([#94](https://github.com/anomalyco/hex/issues/94)), and it
only closed by choosing or reopening. The menu is now anchored to its button,
occludes the controls beneath it, and closes on any outside mouse-down. In the
Rune window the menu is a rune-kit `Menu` in the overlay layer (scene `menus`);
the GPUI occlusion regression went with GPUI and has no Rune counterpart.

**Easy to misread:** an open microphone is not an active recording.

### Scroll without a wheel

```ts
A pane, detail card or dropdown longer than its space    // ui.scroll-bar (#1209)
├── Pane and card bodies -> a wider, clearly visible thumb on the right edge, only while the content overflows
│   ├── Drag the thumb -> the content follows it
│   └── Press above or below it -> jumps there
├── Dropdowns and menus -> the kit's thin thumb, with the same drag and track press
├── Page Up / Page Down, Home / End -> page the pane around focus or the pointer
├── Open menu or dropdown -> ↑ ↓ move the highlight, Page Up / Down by the rows it
│   shows, Home / End to the ends; the highlight stays in view
├── Model search picker -> the same; Home / End only while the search is empty
└── Wheel and trackpad -> unchanged
```

The bar is Rune's `rune_kit::scroll_bar`, used by `scroll_body`, `faded_body`
and every kit `Menu`, `Select` and `Combobox`. Hex's pane and card bodies call
`scroll_bar_styled` instead, with an 8 pt `text_faint` thumb that turns
`text_muted` and widens to 10 pt under the pointer; dropdowns keep the kit's
thin thumb. Checks:
`a_long_transcript_scrolls_by_dragging_its_scroll_bar`,
`a_long_labs_store_scrolls_by_dragging_its_scroll_bar`, and in Rune the
`scroll_bar` tests and `page_keys_move_a_scrolling_listbox_highlight_by_what_it_shows`.
A touchscreen (Wayland, X11, Windows; a pen where the system reports it as a
touch) acts as the primary button (#1337): a tap clicks, dragging a slider or
the bar drags it, and any other drag pans the scroll area under the finger and
flings on; a second finger is ignored. Checks:
`a_finger_pans_the_transcript_and_taps_the_sidebar`, and Rune's touch tests.
Gaps: the bar and touch are not verified on a real Windows PC or touchscreen.

## Process Text With Modes

```ts
Output (main sidebar, replaces the Modes editor) // rune_ui/output.rs; scenes output*
├── Trust prompt -> Allow a config that uses fetch or programs (only when one asks)
├── Edit with agent -> Copy a prompt naming the steps' keys (the AI rewrite too), `hex settings`,
│   `hex vocabulary` for names, and the transforms docs
├── Open file -> settings.json in the default editor
├── Every dictation -> Global's steps, numbered, read-only
├── Try -> Global's corrections and built-ins on a typed sentence
│   ├── Scripts and rewriting run only when dictating
│   └── Labs > Fix results in Output (off, #989) -> Vocabulary (the shared sheet) and,
│       with a sentence typed, Fix this: copies Edit with agent's prompt ending
│       `When I say "<sentence>", Hex pastes "<result>". It should paste: ` (labs.md)
├── Labs > Transform typed text (Linux, off) -> a Typed text section with typed_text_hotkey,
│   unbound by default: copies the focused field, runs its Output steps, re-checks the
│   selection, and replaces it (labs.md, dictation.md)
└── In some apps -> Each mode's apps and its steps (Only: …), opens to list them
    // the old Modes editor stays in the code for its scenes, unreachable

Foreground context -> Select processing
├── Matching website                     // macOS: scriptable browsers, then AXWebArea; Windows: UIA; Linux: AT-SPI
├── Otherwise matching application
├── Hex itself in front                  // application only: Hex never reads its own AX tree (#1402)
└── Otherwise Global

Menu bar and Settings                    // maintenance
├── Menu (macOS), top to bottom: now, things to do, the app
│   ├── Meetings on or a meeting under way: Recording · <title> · 12:34 (opens
│   │   the Meetings pane), Stop & Transcribe, Copy Section ▸; idle: Record Meeting
│   ├── Paste Last Dictation ⌥⇧V (saved binding), Copy Recent Dictation ▸ (Labs),
│   │   Copy Last Meeting Transcript (Meetings on), Speech Model ▸
│   ├── Settings… ⌘, · Check for Updates… · Quit Hex ⌘Q
│   │   └── Labs > ⌘Q closes Settings (off, #1473) -> ⌘Q closes the Settings
│   │       window instead; dictation keeps running, the menu bar icon quits (labs.md)
│   ├── No Meetings… (Settings… opens the same window) and no Show Hex in
│   │   Screen Recordings (Annotate's pane has it)
│   └── check: the_menu_reads_meeting_then_actions_then_the_app,
│       paste_last_shows_the_saved_binding_or_nothing (status_item.rs; the
│       row order and key equivalents only, no native menu is drawn)
├── Icon -> 18 pt monochrome Hex mark template (recording variant + elapsed time)
│   └── Labs > Hide menu bar icon (off, #1064) -> hidden unless a meeting is under
│       way; shortcuts keep working; Finder or Spotlight reopen -> Settings (labs.md)
├── Settings / Paste Last -> Reachable with Dock icon hidden
├── Launch at Login -> macOS owns registration state
├── Check for Updates -> The updater (maintain.updates); opens the window, whose
│   footer then says "Checking for updates…", "Hex is up to date", or
│   "Couldn't check for updates" (#1120)
└── Quit -> Orderly shutdown (⌘Q closes Settings instead while its Lab is on)
```

```ts
Launch Hex                               // maintain.startup
├── `hex app --hidden` + usable menu-bar item -> No startup window, even before setup
│   └── check: login_item_launch_starts_without_the_window (the decision only); not run on a Mac
├── Setup incomplete / permissions or model missing -> Open setup or Settings
├── Show Dock icon on -> Open Settings
│   └── Started by Launch at login -> No startup window (#1321)
│       ├── macOS marks the open-application Apple event as a login-item launch
│       ├── The activation that comes with that launch doesn't reopen the window (#1298)
│       └── check: login_item_launch_starts_without_the_window (the decision only); not run on a Mac
└── Show Dock icon off + usable menu-bar item -> No startup window
    ├── Dictation starts independently of the Settings window
    └── Menu-bar Settings / Finder or Spotlight reopen -> Open or focus the window
        └── On the active Space, never switching back to where it last showed

Menu-bar installation fails -> Show Dock icon and open the window // recovery access

Show Dock icon off                       // maintain.dock-policy
├── Settings window open -> Dock icon stays // hiding it would deactivate Hex
└── Settings window closes -> Menu-bar only

⌘Tab to Hex while it's in the Dock and its window is closed   // maintain.app-switcher (#1298)
└── Opens the window in front, on the pane it last showed, as the Dock icon does
    ├── No reopen Apple event comes with ⌘Tab; Hex watches its own activation
    └── Menu-bar only, an overlay that took the keyboard, or the window already open -> nothing
    // check: switching_to_hex_opens_its_closed_window (the decision only); not run on a Mac
```

```ts
General > Launch at login                // maintain.login-item
├── Open -> Checking until macOS returns the initial state
├── Refresh / toggle / Open settings -> One persistent background worker
│   └── One active request; latest pending user action precedes the next refresh
├── Toggle -> Switch moves at once; macOS's answer settles it
│   ├── Answers behind a later click -> Switch keeps the last click
│   └── Settled answer differs -> Switch springs back
├── Failed toggle -> Show error and reconcile with actual macOS state when available
└── Close Settings -> Release channels without waiting for an active native call
```

[desktop/login_item.rs](../../src/desktop/login_item.rs) serializes ServiceManagement calls on one
window-owned worker, with bounded request and response channels and an autorelease
pool per operation. Settings reads responses without blocking on its 250 ms tick;
refreshes do not accumulate while macOS is busy. The toggle is unavailable until
the first successful status lookup. A click is held in memory
(`PlatformState::login_item_requested`) only until the worker is idle again, so
the switch need not wait for `SMAppService` (about 120 ms per toggle, measured
October 24, 2026, plus up to one tick). macOS owns registration state; no login
Boolean is persisted. Scenes never start the worker or call macOS.

The worker tests in `desktop/login_item.rs` exercise thread reuse, nonblocking polling,
latest-action coalescing, action priority, and closing during a blocked call.
`launch_at_login_asks_the_login_item_and_needs_approval_first` in
[rune_ui/tests/panes.rs](../../src/rune_ui/tests/panes.rs) checks the Rune row's
requests and the approval state, and the `login_item_*` tests there check the
switch while macOS answers: confirmed, refused, a status answer behind a click,
and rapid clicks. The GPUI regression for error rollback and
external enablement went with GPUI. Native `RequiresApproval` and error transitions remain unverified;
the simulated checks do not establish Developer ID signed distribution behavior.

Observed September 23, 2026, on `d1930b5` after the final simplifications: the
full debug suite passed 486 tests and the release suite passed 485, each with
twelve opt-in tests ignored and all twelve keyboard-layout child scenarios
passing. Formatting, strict debug all-target/all-feature Clippy, and diff
whitespace checks passed.

Computer-use checks on an ad-hoc signed temporary bundle of the optimized build
confirmed shortcut capture/cancellation, Settings/Modes navigation, registration
and unregistration, the final state after three consecutive toggle clicks, and
refresh after removing that bundle's login item in macOS Settings. The temporary
item was left disabled/removed; no new microphone, input, or Accessibility grants
were made. A five-second `sample` profile found `SMAppService` frames on the
`login-item` worker and none on the main thread. This is sampled-stack evidence,
not an input-to-frame latency measurement or a signed-release smoke test.

Closing Settings left the app process alive with zero `login-item` threads;
reopening produced one new worker. An earlier apparent surviving worker came
from computer-use observation reopening the window. Cmd+Q and Quit Hex each
terminated the process. Local profiles are `/tmp/hex-login-worker-current.sample.txt`
and `/tmp/hex-login-cycle-{before,closed-no-observe,reobserved}.sample.txt`.
The tested temporary executable's SHA-256 is
`d1a3be1e219e7371726273937eeef6c7e16434637e4297ae183f9bdaf4d28b04`.
No Developer ID signing identity was available for a new distribution build.

The existing **Show Dock icon** preference controls quiet startup; there is no
additional launch-window setting. This applies to normal and login launches,
does not change login registration, and does not hide an already open window
when the preference changes. Any open Settings window keeps the Dock icon: macOS
deactivates an app asynchronously when it becomes menu-bar only, which dropped
the window behind other applications ([#103](https://github.com/anomalyco/hex/issues/103)).
Turning the preference off therefore takes effect when Settings closes.

Closing the window only orders it out, and AppKit keeps an ordered-out window
on the Space it last showed on, so reopening it from the menu bar on another
Space switched back to that one. `macos_rune` gives the window
`NSWindowCollectionBehaviorMoveToActiveSpace`, so it opens on the active
Space instead. No automated check covers Spaces; it needs a native check with
two desktops.

Checks in [meeting/watcher.rs](../../src/meeting/watcher.rs):
`dockless_startup_stays_quiet_only_when_setup_and_menu_bar_are_ready` and
`dock_visible_startup_always_opens_the_app` cover the startup decision. The
Dock/Finder reopen Apple event and the status item's `OpenSettings` bypass it.
These checks do not establish signed-app login behavior or native Finder/Spotlight
reopening; those still need an installed-app smoke test. This addresses the
menu-bar-only case in [#64](https://github.com/anomalyco/hex/issues/64), not an
independent window preference for users keeping the Dock icon visible.

**Executed September 3, 2026:** 441 Rust tests passed (nine opt-in tests ignored),
along with all twelve keyboard-layout child scenarios, formatting, and Clippy.
The release Settings preview built and launched, but window screenshot capture
failed; no visual or installed-app startup verification is claimed.

```ts
Hex checks the release service            // maintain.updates
├── 10 s after launch, hourly (Dev channel: every 10 min and on window focus), and Check for updates -> anoma.ly/release/hex/desktop/<channel>.json
│   ├── User-Agent: hex/<build channel>/<version> (hex/dev/0.0.0-dev-171, hex/stable/2.3.1); nothing else about the install
│   ├── Failed check -> backs off, never past hourly
│   └── <channel> -> updates.channel (General › Application › Update channel: Stable | Dev,
│       or `hex settings set updates.channel`), else the channel the build was published on
│       (HEX_BUILD_CHANNEL; older builds: dev for a dev version, else the receipt's);
│       package managers always stable
│       └── Choosing a channel -> checks now; a staged update stays staged
├── Versions never compared: channel's current release != running build -> take it
│   ├── Running build -> Linux/Windows: executable SHA-256 = target's; macOS: same version
│   ├── Another channel than the build's -> sidebar, tray, menu bar: Switch to Stable 2.3.1
│   │   └── Only after the user confirms -> download -> stage (never switches by itself)
│   ├── required -> install without waiting for the rollout
│   └── Rollout -> device-local bucket in <support>/update-bucket, never sent
├── Direct install -> download (resumable) -> size + SHA-256 -> code signature -> stage
│   ├── macOS -> unpack beside Hex.app -> Developer ID 5NZ4Q7NXJ4, anomaly.hex, notarized
│   ├── Windows -> Authenticode valid, signer AUTHENTICODE_SIGNER -> new version directory
│   ├── Linux -> SHA-256 only -> new version directory, `current` switched
│   ├── Staged -> checks continue; Restart to update stays offered meanwhile
│   │   ├── Another release this install takes (update, required) -> download -> replaces the staged one
│   │   └── Same release, rollout deferred, unconfirmed switch, rollback, failure -> keep the staged one
│   └── Sidebar Update, menu bar / tray: Restart to update; a dev build lists its
│       first changes and What's new (https://anoma.ly/hex/dev) under it (updater/notes.rs)
│       ├── One more check first (manifest within 15 s) -> Checking / Downloading -> newest staged
│       │   └── A release published after the relaunch -> the next launch's check
│       ├── macOS -> renamex_np swap (also at quit) -> relaunch -> old bundle removed at launch
│       └── Labs › Release notes link -> a release build's footer carries this build's notes:
│           anoma.ly/hex/dev#<version> for a dev build, github.com/anomalyco/hex/releases/tag/v<version> for a stable one;
│           local and test builds show no link (by build kind, not version string)
├── macOS location not replaceable (read-only, translocated, root-owned) -> Download update (DMG)
├── Package manager (pacman, Nix) -> its command, once it serves another version
├── Source build, test build -> never checks
└── Failure -> keep the running version, back off, Check for updates retries

Check for updates pressed (footer, menu bar, app menu)   // maintain.update-feedback (#1120)
├── The menu bar's and app menu's open the window at General
└── Footer -> "Checking for updates…" -> "Hex is up to date" | "Couldn't check for updates"
    // the hourly check alone keeps "Check for updates"; check:
    // an_asked_update_check_says_how_it_went (fixture; not run on a Mac)

Capture                                  // hex app and hex dictate
├── Shortcut starts capture -> Queue start sound immediately, before audio arrives
├── Hold passes 300 ms -> While dictating mute/pause/lower begins, never at the press
├── Finish retained recording -> Stop sound at the release, while the 200 ms
│   tail is still captured; a muted output unmutes first   // dictate.stop-tone
│   └── Clip turns out near-silent or speechless -> Stop sound already played,
│       nothing pasted, no error sound or note; the log has its level
├── Brief discarded tap -> Start sound, but no stop sound or transcription
└── Dictation pasted -> Done sound; failures play the error sound
```

The shared [feedback player](../../src/feedback.rs) uses Hex's bundled sounds
([Feedback](dictation.md#feedback)) on every platform; only macOS wake/sleep
use `afplay`.
Decoding happens before the Linux capture loop; the output device opens for
the first tone and is released shortly after the last one (about ten seconds
on macOS, five minutes on Windows, one second on Linux), as described in
[macOS feedback](dictation.md#feedback). A fatal stream error (the output became
unavailable or invalid) drops the output, so the next tone reopens it. Playback
admission is bounded and nonblocking. Output preparation failures are logged
without blocking dictation. The Settings window saves the volume to
`settings.json` and the service reloads it without restarting the listener. The start sound does not wait
for the 300 ms capture-retention threshold; input delivery and audio-device
latency still apply. **While dictating** mute, pause, or lower does: the audio loop
requests it when the capture's audio reaches 300 ms past the press, and not
during a release tail. Until 2026-10-22 the service muted at the press. This
ordering is checked by reading the loop, not by a test or a muted device.

Checks: `bundled_sounds_decode_without_an_audio_device` and
`feedback_admission_never_waits_for_playback` in the player.
`start_sound_precedes_audio_and_does_not_change_short_tap_discard` in
[linux/dictation.rs](../../src/linux/dictation.rs) exercises the production start
path with a sound spy: immediate feedback without any audio, while a 100 ms tap
still discards. Existing timing checks in [dictation.rs](../../src/dictation.rs)
cover the unchanged intentional-hold boundary. These are not proof of audible
native playback or physical hotkey-to-sound timing.

The [September 4 sound-build evidence](release-verification.md#linux-recording-sounds-builds)
records the initial and immediate-start builds. It does not establish audible
native playback, measured physical key-to-sound latency, or Wayland UI behavior.

Both Linux CI triggers include `tests/**`, and the
[Nix source fileset](../../nix/package.nix) includes those sources. The
`keyboard_layout` harness skips on Linux; packaging it does not provide macOS
layout evidence.

```ts
TypeScript SDK + local service             // macOS, Linux, Windows
├── Caller records audio -> Explicit helper -> Raw transcription
│   └── Linux, Windows -> No packaged helper; command: hex service --embedded
├── Caller streams audio -> Running app's /streams -> Partials, segments, final
│   └── Dictation first; uses the loaded model only (docs/streams.md)
├── Running app's /transcriptions -> Listener's engine when it holds the model
│   └── Otherwise the service's own copy of the requested model
├── Running desktop capture API -> Separate microphone ownership protocol
│   ├── No silence gate                  // model output, unlike hotkey dictation;
│   │                                     // only recognition-hint echoes are dropped
│   ├── Linux, Windows -> The listener records it (dictation::service owner)
│   │   ├── With the microphone chosen in Settings, as the shortcut does
│   │   ├── Shortcut or `hex dictation` records -> 409 dictation-busy
│   │   ├── Client's dictation records -> shortcut starts and ends nothing
│   │   ├── Lapsed lease or owner's cancel -> only that dictation is dropped
│   │   ├── Microphone lost -> finish answers microphone-lost (no kept clip)
│   │   └── Start with "partials": true -> GET /dictations/{id}/partials
│   │       └── SSE {stable, tentative} while the model streams (GPU)
│   ├── macOS -> No partials: dictationPartials false; the SSE ends at once
│   └── hex service -> serviceCapture false, 404 service-capture-unavailable
└── Distribution
    ├── SDK 0.3.0 -> Published
    └── Turnkey signed helper -> Unfinished
```

Linux and Windows serve the same local API from the running app
([linux/app.rs](../../src/linux/app.rs), [windows/rune.rs](../../src/windows/rune.rs))
and from `hex service [--embedded]`; a failed start only logs, and Hex runs on
without it. On Linux, the embedded service answered `/health`,
`/capabilities`, `/models`, and transcribed a WAV with Parakeet Unified
English on the Vulkan iGPU (October 30). The running app's shared-engine path,
`/streams` on a live Linux or Windows listener, and anything on Windows beyond
`cargo xwin clippy` (x64) are unverified.

Linux `/dictations` (#981) shares the macOS HTTP contract and the owner,
lease, and level logic in [dictation/service.rs](../../src/dictation/service.rs);
the listener ([linux/dictation.rs](../../src/linux/dictation.rs)) records it
without a hold threshold or release tail. Unit tests cover ownership, lease
expiry, busy refusal beside a shortcut capture, and the HTTP round trip
(`local_api` tests now run on Linux). A live Linux microphone run through the
SDK is unverified.

Windows `/dictations` uses the same owner in the listener
([windows/dictation.rs](../../src/windows/dictation.rs)), so a client gets
Hex's microphone choice, pre-roll, and While dictating behavior, and the HUD
shows the recording. Live text is opt-in per dictation: `POST /dictations
{"source": "…", "partials": true}` holds a live-text demand for the
recording's stream, and `GET /dictations/{id}/partials` (owner token, once)
sends `data: {"stable": "…", "tentative": "…"}` after each change until the
dictation ends; it is provisional, and `/finish` returns the transcript.
`/capabilities` reports `dictationPartials`. Checked live on Windows x64
(start, partials subscription, finish, a silent room's empty transcript);
not run on Linux.

The [SDK guide](../../sdk/typescript/README.md),
[service contract](../specs/local-transcription-service.md), and
[SDK tests](../../sdk/typescript/test) distinguish those paths. Helper-only service
mode is not a physical hotkey/microphone test. Embedded/packaged service smokes
now expect API 2, matching service/SDK compatibility checks. Helper packaging
requires an explicit signing team and notarization profile, with the same
Developer ID identity validation as the app; identity fixtures do not establish
a signed helper distribution. Low-level blocking-peek and
FIN/half-close limitations in [local_api.rs](../../src/local_api.rs) must not be
generalized to every SDK cancellation path.

Live streams (`/streams`, [contract](../streams.md)) are exercised by fake
engines only: `transcription/stream` tests cover windows, partials, the
backlog during dictation, preemption and offline fallback, the lease, cancel,
and busy; `local_api::streams` tests cover routes, auth, and bounds; and
`transcription/engine` tests cover yielding streams closed by dictation. No
real-model or live-app stream has been run.

Model progress and dictation levels share bounded SSE framing in
[sse.ts](../../sdk/typescript/src/sse.ts). In-memory
[SDK regressions](../../sdk/typescript/test/sse.test.ts) cover transport-chunk
independence, line/event limits, callback cancellation, reader cleanup, and
bounded level observations. A CR-delimited terminal event is also checked with
an open response body, so completion does not depend on EOF or subsequent data.
These checks do not establish live service or
microphone behavior.

## Developer And Prototype Surfaces

```ts
macOS debug runtime                      // not release navigation
├── Meetings -> Always available; release builds need the Meetings opt-in (its pane's card or Meetings › Settings)
├── Activity / HUD Lab / hex dev -> Observation and developer controls
└── HUD Lab in release -> Only in the playground (Labs › Playground)

