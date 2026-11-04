# Scrawl App UI Handover Spec (for Hex)

This spec describes the user-facing flows and edge cases that live in Scrawl's macOS app shell (`src/`) above `scrawl-capture` and `scrawl-router`. Pure geometry, state machines, and protocol logic (`capture::pure_shift_state`, `capture::InputSink`, `sketch::freehand_outline`, `routing::*`) already live in the crates; Hex implements the native Swift/Rust UI flows below.

---

## 1. Capture Hotkey, Capsule & Target Switcher

### 1.1 Record Shortcut (`⌥⇧S` by default)
- **Trigger on modifier release, not key-down:** When the record shortcut is pressed while idle (`Activity::Free`), Scrawl enters a pending state and waits until all shortcut modifiers (`⌥`, `⇧`, etc.) are released before starting the recording (`RecordScreen`).
  - *Why:* Starting while `⇧` is still held would immediately treat cursor movement as `⇧` ink, and releasing `⌥⇧S` over the target app would leak modifier-up events into the recording.
  - *Cancel while held:* Pressing `Esc` before the shortcut modifiers are released cancels the pending start (`Action::Cancel`) without flashing the screen or creating a bundle.
- **Double-press guard (`DOUBLE_PRESS_MS = 400 ms`):** A second press of the record shortcut within `400 ms` of recording start is ignored (`Action::None`) so accidental key bounce or double-taps do not immediately end a 0-second capture.
- **Press while recording (`≥ 400 ms`):** Finishes the active recording (`Action::Finish`).
- **Press while target picker is open (`Activity::Picker`):** Starts recording the currently highlighted target in the picker (`Action::RecordSelected`).
- **Press while processing (`Activity::Processing`):** Ignored (`Action::None`).
- **Default shortcut aliases:** When the shortcut is left at its default (`⌥⇧S` / `⌥⇧V`), `capture::shortcut_matches` also accepts `⌃⌥⇧`, `⌘⌥⇧`, and `⌃⌥⌘` on both the active keyboard layout's `S`/`V` key and the physical ANSI key code. Custom shortcuts match their exact Carbon modifiers and key code only.

### 1.2 Target Switcher & Reveal Chips (`Area · Window · Screen · iPhone`)
- **Default start is full display under cursor (`Screen`, auto-cropped):** Pressing and releasing `⌥⇧S` starts recording the display under the pointer immediately (`Target::ActiveDisplay`), positioned below that display's menu bar and notch (`screens::capsule_origin_for_display`).
- **Reveal chips on tap-start:** For the first `3,500 ms` (`REVEAL_MS`) of a shortcut-started screen recording, the floating capsule expands to show target-switch chips (`Area · Window · Screen · iPhone`):
  - Minimum visibility: `900 ms` (`REVEAL_MIN_MS`) even if the user immediately clicks or draws.
  - Auto-fold on work: Once `900 ms` has elapsed, the first recorded user action (click, scroll, or stroke) folds the switcher chips back into the compact recording pill.
  - Hover linger: Hovering the capsule keeps the chips open; after the pointer leaves the capsule, they stay open for `1,200 ms` (`REVEAL_LINGER_MS`) before folding.
- **Mid-capture target switching:** Clicking `Area`, `Window`, or `iPhone` while the reveal chips are open switches the capture target:
  - **Window:** Highlights hovered windows (front-to-back hit-testing via `capture::window_at_point`, excluding Scrawl/host windows); clicking a window switches the recording to `CaptureMode::Window` (or restarts the stream on that window).
  - **Area:** Lets the user drag a rectangle on a single display (`screens::clamp_rect_to_display`; minimum size expanded via `screens::ensure_min_area_rect`). Cross-display drags clamp to the display where the drag started.
  - **iPhone:** Connects to a cabled iPhone (`helper::devices`), opens the mirror window, and records native device video + touch telemetry. If the phone disconnects or is locked (`Event::PhoneEnded`), the capture ends cleanly or prompts to unlock.
- **Explicit picker (`Choose What to Record…`):** Opening the picker from the menu bar enters `Activity::Picker` before starting any recording; `Esc` dismisses the picker.

---

## 2. Ink Overlay (`⇧` Ink, Pins, Laser, Flags & Undo)

The ink overlay is a transparent, click-through (`ignoresMouseEvents = true`), non-activating (`NSWindowSharingNone`) window covering the target display, excluded from `ScreenCaptureKit` via `OwnWindows::excluded` and ignored for self-click filtering via `OwnWindows::passthrough`.

### 2.1 Pure Shift State (`capture::pure_shift_state`)
- `⇧` only activates ink/pins when `⇧` is held **alone** (no `⌘`, `⌥`, `⌃`, or `fn`) **and** is not left over from a multi-modifier chord (such as `⌥⇧S` at capture start or `⇧⌘4` mid-capture).
- If another modifier is pressed while `⇧` is down, `await_shift_release` latches `true` until `⇧` is completely released and pressed again on its own.

### 2.2 Drawing `⇧` Ink (`InputSink::ink`)
- **Draw without clicking:** Holding pure `⇧` and moving the pointer inside the capture target draws live red ink (`#FF3B30`) along the cursor path (`sketch::freehand_outline`), without requiring a mouse button press.
- **Minimum span (`MIN_INK_SPAN_PT = 6.0 pt`):** Releasing `⇧` commits the stroke only if the pointer moved at least `6 pt` from its start (`≥ 2` points). Stationary `⇧` taps (e.g. typing a capital letter while the cursor rests on screen) are discarded so they never produce stray dots.
- **Point decimation & cap:** Points closer than `1.5 pt` to the previous point are skipped, capped at `2,400` points per stroke.
- **Accessibility / DOM probing:** When `agentContext` is on, releasing a stroke probes the Accessibility tree (and `/web/context` for signed browser tabs) at the stroke's target point (`ax::stroke_probe_point`: just above underlines, center of circles, tip of arrows).

### 2.3 Numbered Pins (`⇧` + Left Click)
- **Drop a pin:** Holding pure `⇧` and left-clicking (`button_down` → `button_up` within `PIN_MAX_PRESS_MS = 1,000 ms` and `< CLICK_SLOP_PT = 12.0 pt` movement) drops the next sequential numbered pin (`1`, `2`, `3`, …) at the press point.
- **Mid-drawing suppression:** If the user has already drawn `≥ 6 pt` of ink during the current `⇧` hold before clicking, the click is treated as part of the drawing and does **not** drop a pin or record a click event.
- **Long still press (`> 1,000 ms`):** Holding `⇧` + mouse button stationary for over `1 s` before releasing drops no pin.
- **Visual:** Rendered on the overlay with a brief outward pulse (`sketch::pin_pulse`) and stamped on keyframes as a red numbered badge with a white border.

### 2.4 Laser Pointer
- Triggered when the pointer moves with the laser modifier/button active (`InputSink::laser`).
- Renders a trailing, fading red beam (`sketch::slice_laser_window` + `sketch::smooth_laser_points`) on the live overlay without leaving permanent ink on later keyframes; when the laser ends, its final resting coordinate is recorded as an `InteractionEvent::Point`.

### 2.5 Flags & Undo
- **Flag (`InputSink::flag`):** Pressing the flag button in the recording pill (or shortcut) stamps a timestamp-only `Shape::Flag` mark (`t_ms`), which forces a priority keyframe at that moment even if the screen is static.
- **Undo (`⌘Z` while `⇧` is held, or `InputSink::undo_stroke`):** If live ink is currently being drawn, clears the in-progress stroke; otherwise pops the most recently committed stroke or pin. Normal `⌘Z` without `⇧` held passes through to the frontmost app as a normal key chord.
- **Self-window filtering:** Clicks and scrolls landing on the host's own capsule or settings windows (`OwnWindows`, excluding the click-through ink overlay) are never recorded as capture interactions.

---

## 3. Paste of a Finished Capture

When a capture finishes (`SealedCapture::finish` → `Finished`), the app decides how to hand it off:

### 3.1 Clipboard & Two-Step Paste (`⌥⇧V` or Paste button)
- **Clipboard default:** As soon as a capture finishes, `summary.md` is copied to the system clipboard (`SystemClipboard::copy_bundle`), so a manual `⌘V` anywhere pastes the plain-text markdown summary with absolute paths to `contact-sheet.png` and `scrawl.json`.
- **Two-step synthetic paste (`paste_bundle_image_and_text` / `⌥⇧V`):**
  1. Wait up to `350 ms` for all physical modifier keys (`⌥`, `⇧`, `⌘`, `⌃`) to be released so synthetic `⌘V` does not turn into `⌥⇧⌘V`.
  2. Activate the target external app (or, if the user dragged the Paste chip onto a specific window/input, post a synthetic click at that coordinate and wait `65 ms` for focus).
  3. **Step 1 (Image):**
     - If the frontmost app is a terminal (`Ghostty`, `iTerm2`, `Terminal`, `kitty`, `WezTerm`, `Alacritty`, `Warp`), copy the absolute path of `contact-sheet.png` as text and post `⌘V` (terminals attach `[Image 1]` from file paths).
     - If the frontmost app is a GUI app (Slack, Linear, browsers), copy `contact-sheet.png` as `NSPasteboardTypePNG` bytes and post `⌘V`.
  4. Wait `160 ms` for the target composer to ingest the image attachment.
  5. **Step 2 (Text):** Copy `summary.md` as `NSPasteboardTypeString`, wait `20 ms`, and post a second `⌘V`.

---

## 4. Onboarding & Permissions

Scrawl requires two permissions to record and two optional permissions for richer context (`crates/capture/src/permissions.rs`):

| Permission | Role | Detection & Grant Flow |
|---|---|---|
| **Screen Recording** | Required for Mac screen/window/area capture | `CGPreflightScreenCaptureAccess()` / `CGRequestScreenCaptureAccess()`. Granting in System Settings may require an app relaunch on macOS if the process was already running. |
| **Microphone** | Required for voice narration | `AVCaptureDevice.authorizationStatus(for: .audio)`. Prompts inline on first request; if denied, deep-links to `x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone`. |
| **Accessibility** | Optional — enables synthetic `⌘V` paste, global mouse/key monitors, and UI element labels (`agentContext`) | `AXIsProcessTrusted()`. Prompts via `AXIsProcessTrustedWithOptions`; deep-links to `Privacy_Accessibility`. Without it, Scrawl falls back to polled mouse buttons (`NSEvent.pressedMouseButtons`) and clipboard copy instead of synthetic paste. |
| **Input Monitoring** | Optional — global key chord logging & shortcut tap when Accessibility is off | `CGPreflightListenEventAccess()`. Covered automatically when Accessibility is granted. |

- **Non-blocking permission panel:** If the user presses `⌥⇧S` when Screen Recording is missing, show the permission checklist window explaining what is missing with one-click buttons to open the exact System Settings pane, and poll grant status live (`permissions::Grants::current()`) so the row turns green as soon as the toggle is flipped.
- **Signed binary identity:** Keep the app signed with a stable Developer ID bundle identifier so macOS `tccd` preserves Screen Recording, Microphone, and Accessibility grants across updates.

---

## 5. Settings Surface

In Hex, these settings live in Hex's own config store and construct `scrawl_capture::EngineConfig` and `scrawl_router::RouterConfig` (with one-time migration from `~/.config/scrawl/config.json` via `scrawl_router::parse_scrawl_config(&str)` and `handoff::import_or_ensure_ingest_token_file`):

1. **Shortcuts:**
   - **Record shortcut:** Default `⌥⇧S`. Cannot be unset (`ShortcutKind::Record.can_unset() == false`).
   - **Paste last capture shortcut:** Default `⌥⇧V`. Can be cleared/disabled (`"paste": null`).
   - *Recorder edge case:* Pause global shortcut listeners (`capture::pause_shortcuts(true)`) while the user is recording a new keybinding so pressing the existing shortcut inside the recorder field captures the keys instead of starting a recording.
2. **Recording & Transcription (`EngineConfig`):**
   - **Microphone (`microphone`):** Follow macOS system default (`None`) or pin a specific CoreAudio device UID (`MicChoice { id, name }`). If a pinned mic is unplugged, fall back to the system default for that recording and surface a note (`mic::Pick::note`).
   - **Transcriber (`transcriber`):** `hex` (Hex's local model server), `apple` (`SpeechAnalyzer` on macOS 26+ with locale picker `appleLocale`), or `command` (custom CLI program).
   - **Auto-crop (`autocrop`):** On by default for full-display captures; crops keyframes to the active window/region while preserving full frames in `full/`.
3. **Storage & Retention (`retention.rs`):**
   - **Captures folder (`capturesDir`):** Must be an absolute or `~/…` writable directory; if a configured external drive is unplugged or read-only, log once and fall back to the built-in Application Support directory so a capture is never lost.
   - **Cleanup policy (`retention`):** Presets (`Keep all`, `Balanced` — shrink/remove heavy video after 7 days and trash bundles after 30 days, `Compact` — 2 days / 14 days). Never touch `.pinned` bundles, `.media-sending` bundles, or active recordings.
4. **Agents & iPhone Pairing:**
   - **CLI & Skill:** Install `scrawl` on `PATH` (`~/.local/bin/scrawl`) and `skills/scrawl/SKILL.md` so coding agents can run `scrawl zoom` and `scrawl inspect`.
   - **ScrawlKit pairing (`ingestToken`):** Owner-only (`0600`) bearer token and `scrawl://setup?...` deep link / QR code for pairing iOS apps built with `ScrawlKit`.
