# Annotate (macOS, Linux, and Windows)

Annotate is Scrawl's capture engine hosted inside Hex (code and settings keep
the name `capture`), steps 3 to 5 of
[the plan](../plans/capture-in-hex.md). It runs the real
`scrawl-capture` crate, imported into `crates/capture`: it records the active
display with ScreenCaptureKit, on Linux the screen-cast portal and
PipeWire (`capture.linux`), on Windows Windows.Graphics.Capture
(`capture.windows`), (or a cabled iPhone through `scrawl-helper`'s
mirror) and narration from Hex's microphone, takes ink and clicks (Shift on
macOS and Windows, or the Annotate key held or the pen), transcribes the narration with word timestamps on
Hex's loaded model, writes a Scrawl bundle into Scrawl's captures folder,
and copies it to the clipboard. Settings › Annotate (under
Labs in the sidebar) holds its settings. Scrawl's capsule (switch
chips, the on-screen picker) and its ink and laser overlay are
Rune overlays.

```ts
Turn it on                               // capture.gate
├── On by default in every macOS, Linux, and Windows build, stable included
│   (capture.enabled); the `annotate-on-by-default` migration turns it on
│   once for older settings
│   └── macOS bundles sign scrawl-helper beside `hex`; validate-app.sh requires it
├── Nothing is asked at launch: Screen Recording (macOS) at the first capture,
│   the portal's dialog (Linux) at the first Screen capture; Windows asks
│   nothing
├── A start without a permission it needs (macOS: Screen Recording unless
│   iPhone, then Microphone, then Accessibility; capture_host::permissions)
│   -> refused with code `permission`, no error tone, and the capsule's
│   permission helper opens (capture.permission)
└── Off -> no capture host, engine, sink, thread, or folder; `hex annotate` -> feature-off

Draw                                     // capture.draw (capture_host::drawing)
├── One key: tap (under DRAW_HOLD, 250 ms) to start and again to stop; held
│   while it records, the pointer draws with no click until it comes up
│   (macOS: CaptureShortcut tells the tap from the hold at release; Linux:
│   the portal's Activated/Deactivated or the X11 grab's press and release,
│   linux/input Capture/CaptureUp; Windows: the keyboard hook's
│   HotkeyAction::Capture/CaptureUp -> CaptureHost::annotate_down/up)
├── The capsule's pen -> on until clicked again, or the key is tapped, or
│   held and let go (Action::Pen -> capture_host::toggle_pen). Like a real
│   pen it inks only between mouse-down and mouse-up (capture_host::
│   pen_button; Linux: the overlay's drag, macOS: the tap's buttons); moving
│   with the button up draws nothing, and clicks still never reach the app
│   └── Releasing the button ends the stroke on the `capture-input` thread;
│       the pointer it reads (capture_host::ink_source) has its own lock, so
│       it never re-takes the input target's lock that thread holds (#572:
│       Windows hung on the first stroke). Check: capture_host
│       ending_a_stroke_while_the_input_target_is_locked_does_not_deadlock;
│       not observed on a Windows PC
├── Esc while drawing -> stops drawing, never the recording (recognition.rs
│   on macOS; the Escape grab on X11 (Wayland binds no Escape); the keyboard
│   hook on Windows, when no dictation records; and the overlay's own key
│   handler where it gets the keyboard)
├── macOS and Windows -> Shift held alone still draws, as in Scrawl
├── While drawing -> the ink overlay takes the pointer with a pen cursor, so
│   clicks never reach the app; the capsule stays clickable above it (macOS:
│   its level; Linux: the overlay leaves a hole over the capsule's pill).
│   Every move draws ink; there are no pins and no undo
├── `hex annotate draw` (Linux) -> the pen on or off, for desktops without
│   the portal (it starts a recording drawing)
├── Default key: ⌥⇧S on macOS; F9 on Linux and Windows, a single key one hand holds
│   while the other moves the mouse (F10 is GTK's menu key, F11 full screen,
│   F12 often a dropdown terminal). Saved bindings stay.
│   Check: capture_host the_shortcut_records_once_its_keys_come_up; none
│   physical for the hold on a portal desktop ("Annotate pen" is logged)
├── Ink holds 650 ms, then fades over 650 ms on screen; every stroke stays in
│   the bundle (keyframes, contact sheet, scrawl.json)
├── Linux: strokes are placed on the recorded screen in its logical points,
│   from the ScreenCast portal's stream size when it reports one smaller
│   than the frames, so a fractional scale (1.5, 1.33), which reaches the
│   overlay rounded up to 2, no longer pins strokes to a top-left part of
│   the screen; without that size it divides the pixels by the overlay's
│   scale. Check: scrawl-capture capture::platform::tests
│   a_fractionally_scaled_screen_keeps_its_logical_size; not observed on a
│   fractionally scaled monitor. Known gap: the overlay starts below a
│   desktop bar's reserved area, so saved ink sits that bar's height off
│   the live ink
└── Linux recordings show the overlay itself (Wayland can't leave it out)

On Linux                                 // capture.linux
├── X11 session (DISPLAY set, WAYLAND_DISPLAY empty; never XWayland) ->
│   Screen and Area are read from the X server itself (scrawl_capture::
│   linux::x11): MIT-SHM GetImage each frame (plain GetImage without SHM),
│   the pointer blended in from XFixes, 30 fps to x264 through ffmpeg. No
│   portal and no dialog, so i3 and other desktops without a ScreenCast
│   backend record (#690, @nefthy). Screen is the RandR monitor under the
│   pointer; Area is `hex annotate start --area X,Y,W,H` (pixels), clipped to
│   the screen. Hex's own overlays show in it. A Window still goes through
│   the portal's picker and, without one, fails saying to record the screen
│   or an area. Can't open the display, an unusual pixel depth, or no
│   ffmpeg/x264 -> the capture fails at once with why in the capsule, never
│   a capture without a picture. Check: x11 areas_are_clipped…; a headless
│   Xvnc + i3 session on apollo with no ScreenCast portal recorded an
│   800x500 Area (154 frames, 5.1 s) and the 1920x1080 screen (153 frames)
├── Wayland -> the ScreenCast portal (zbus): Screen asks the desktop once and
│   keeps its restore token (<support>/screencast-token), so later captures
│   start without a dialog; Window shows the desktop's picker every time (the
│   capsule's Window chip goes straight to it); the cursor is embedded
├── Frames -> the portal's PipeWire stream, libpipewire-0.3 loaded at run
│   time; 32-bit RGB as a linear DMA-BUF (niri, GNOME, KDE) or shared memory
├── Encoding -> VA-API (libva loaded at run time): each DMA-BUF imported, scaled
│   to a 2560 px long edge (1920 for a window) and converted on the video
│   engine, H.264 High at a constant QP with Hex-written headers, 30 fps;
│   `ffmpeg -c copy` muxes recording.mp4. Every /dev/dri render node is
│   tried in order, so a hybrid laptop whose first node is a GPU the VA
│   driver can't use encodes on the next (#853). No VA-API -> frames copied
│   and encoded with x264 through ffmpeg (slow on DMA-BUF-only compositors);
│   a first frame neither path can read (a DMA-BUF the CPU can't map) fails
│   the capture with why, never a capture without a picture. Check: vaapi
│   render_nodes_in_order; not observed on a hybrid laptop
├── Candidates -> a small copy from the video engine (at most 4× smaller per
│   pass: radeonsi crashes past it)
├── Reading movies back (keyframes, probe, audio) -> `ffmpeg`/`ffprobe`
├── Missing PipeWire, ffmpeg, or a picker answer -> that capture fails with why
│   └── The screen-share picker cancelled (or the stream never starting) after
│       the capture started -> its collector notices within 100 ms, cancels
│       it, and settles `failed`; the listener's next reap ends it, so the
│       shortcut and dictation work again (#681). Check: capture_host
│       a_screen_source_that_gives_up_ends_the_capture; not observed on GNOME
├── Narration -> the listener's microphone through ListenerTimeline (each
│   stream anchored to the monotonic clock); released-while-idle microphones
│   open for the capture
├── Words -> one foreground job on the listener's engine (its loaded model)
├── Copy -> the summary text through Hex's Linux clipboard (wl-copy or the
│   X11 clipboard), as the Mac copies it
├── Paste -> as the Mac: the contact sheet (image/png), then the summary,
│   each with Hex's own Ctrl+V (linux/paste). An app that takes no image (a
│   terminal) gets only the summary, which names the contact sheet; the Mac
│   pastes a terminal the sheet's path instead
├── Finished capsule -> "Copied" with Paste (accent) and Inspect
├── Shortcuts -> F9 (tap to start and stop, hold to draw) through the
│   GlobalShortcuts portal ("Annotate") or X11 grabs. There is no separate
│   draw shortcut on any OS
├── While it records, dictation starts nothing (the portal
│   or grab already took their keys, so nothing reaches the app)
├── Pane -> no Screen Recording row, no transcriber choice, no "Show in screen
│   recordings" (the portal asks; Wayland can't keep overlays out)
└── Gaps: no Area on Wayland (it gives Hex no pointer to aim it; X11 takes
    `--area`), no area picker on screen, no iPhone, no
    laser or click log outside draw mode, no window log, `hex capture status`
    unsupported, choosing another folder only through settings.json

On Windows                               // capture.windows
├── Source -> the engine's Windows.Graphics.Capture recorder (win/media.rs):
│   Screen, Window (the picker lists windows), and Area; no permission asked
├── Units -> the engine's targets, windows, and pointer are physical pixels;
│   the picker and ink overlays sit on the primary display and lay out in its
│   points, converted with capture_host::pixels_per_point (mixed-DPI displays:
│   the primary's scale applies everywhere)
├── Narration -> the listener's microphone through ListenerTimeline, fed by
│   windows/dictation; released-while-idle microphones open for the capture
├── Words -> one foreground job on the listener's engine (its loaded model)
├── Input -> windows/input's low-level hooks: the Annotate key (F9; tap to
│   start and stop, hold to draw), Escape while drawing, and while a capture
│   records every pointer move, click, wheel turn, and modifier change
│   (capture_host::report_input, as the macOS tap): Shift alone inks, the
│   laser follows the pointer, clicks are logged. Injected events are skipped
├── While it records, dictation starts nothing
├── While dictating -> General's setting through windows/audio_control, one
│   suppressor per capture, restored when it ends
├── Copy and paste -> the engine's own clipboard and Ctrl+V (handoff): the
│   contact sheet, then the summary
├── Pane -> no Screen Recording row, no transcriber choice
└── Gaps: no `hex annotate` (Windows has no control transport yet), no
    iPhone, keys pressed in the app are not logged. Checked only by
    cross-compiling (`cargo xwin clippy`, x86_64 and aarch64); never run on
    a Windows machine

Settings › Annotate                      // capture.pane
├── Capture with Hex (capture.enabled, default off) -> off: no host, shortcut,
│   input, retention, or folder
│   └── On while the shortcut collides -> refused, the shortcut row says why
├── Shortcut (capture.hotkey, default ⌥⇧S, F9 on Linux and Windows) -> same keycap and recorder as dictation;
│   a key is required; refused when it overlaps dictation or paste
├── How it works (on) -> start (tap), draw (hold, no click; Shift too on
│   macOS), the capsule's pen with Esc, and stop (tap again), each
│   with its keys. Check: rune_ui::scrawl tests
├── Screen Recording -> read once a second and again when the pane opens or
│   the window becomes key, never with a prompt (capture_host::screen_access):
│   the preflight, which keeps its first answer for the life of the process,
│   then, while it says no, a fresh `scrawl-helper permissions` at most every
│   2 s on its own thread and a one-frame probe once that says granted
│   ├── granted -> Allowed, no button
│   ├── granted in System Settings but unusable until Hex relaunches ->
│   │   "Turns on when Hex restarts" and Restart Hex, never Allow over a switch
│   │   already on
│   ├── Returned from System Settings still untrusted -> "Still grey? Fix it"
│   │   (`ScreenCapture` reset on running bundle + retired IDs, re-prompt + guide)
│   └── Allow -> clears retired Hex IDs, rechecks, then opens the permission
│       guide over System Settings' list (PermissionKind::ScreenRecording); flips
│       on the next read. Never part of onboarding (macOS also asks at the first
│       capture). Check: rune_ui::scrawl tests
├── Show in screen recordings (capture.show_overlays_in_recordings, default
│   off; also the menu bar's Show Hex in Screen Recordings checkmark) -> see
│   capture.own-windows; applies at once, to open windows and a recording capture
├── Transcriber (capture.transcriber) -> Parakeet (default) | Apple
├── Captures folder (capture.folder, default Scrawl's) -> Reveal | choose another
├── Keep (capture.media_days / bundle_days) -> Scrawl's presets, or imported days
├── Scrawl.app running -> "Scrawl is running" card + Use Hex for captures
│   └── Click -> asks Scrawl to quit (nothing else quits it); Hex takes the
│       shortcut within 2 s of Scrawl leaving
└── No iPhone pairing row: ScrawlKit pairing needs the ingest server, which Hex
    doesn't run. A cabled iPhone needs no row (capture.phone)

First launch with Annotate on            // capture.import
├── ~/.config/scrawl/config.json -> transcriber (apple; else Parakeet), capturesDir,
│   retention, read once (migration `scrawl-capture-settings`); token not used
├── Scrawl's Application Support config.json -> its shortcut, unless it collides
└── Scrawl's files are never written

⌥⇧S (capture.shortcut; captures on, Scrawl not running)
├── Press, held as long as you like -> armed; nothing records while ⌥⇧ is down
│   └── One of its modifiers comes up -> start on the display under the pointer,
│       as `hex capture start`                // a fresh press only, no repeats
├── Esc before the modifiers come up -> nothing records
├── Press while recording -> stop, as `hex capture stop` // ignored within 400 ms
├── Press while the capsule's picker is open -> the picker records what it is on
├── Esc while a capture records -> nothing: it reaches the app being recorded,
│   and Hex's tap lets it through; a dictation under it goes on too (#786)
├── The binding is swallowed, never chord evidence for dictation
└── Hold the dictation shortcut mid-capture -> dictation pastes as usual

While a capture records                  // capture.quiet
├── Start -> a recording-environment session with General's While dictating
│   (read at each start; captures have no setting of their own): mutes output
│   or pauses Music, Spotify, VLC; Do nothing changes nothing. No idle-sleep
│   assertion, and it never counts as an intentional dictation
├── Stop, cancel, discard, failed start, captures off, quit -> released; a
│   switch carries it to the next capture without restoring in between
├── Dictation shortcut -> starts nothing while it records
│   (macOS: recognition.rs swallows it in the tap; Linux: linux/dictation
│   ignores Start). Nothing leaks to the app in front
└── Dictation that began before the capture -> each effect is held from the first session that
    wants it to the last, so neither unmutes or resumes what the other still
    holds; a While dictating change mid-capture composes too (the capture keeps
    media paused, the dictation mutes). Only what Hex
    changed is restored. The dictation stop tone plays at once while the capture
    keeps output muted. Check: recording_environment tests (overlap both ways),
    capture_host a_capture_stays_quiet_until_it_stops_is_cancelled_or_hex_quits

Switching target                         // capture.switch; the capsule's chips
├── The recording capsule shows no target chips: a recording is the screen.
│   Switching (below) remains for the CLI, the picker, and the playground
├── CaptureCommand::Switch(target) -> discard and start in one step: no snapshot
│   shows nothing recording in between (Scrawl's switch_recording)
│   ├── No room to start (two writing, Hex stopping) -> refused `busy` before
│   │   the discard; the recording goes on
│   └── The start fails after the discard -> the discarded capture settles
│       `Failed` with the start's error
├── Window or Area -> CaptureCommand::Discard: bundle removed, no outcome, gone
│   from the snapshot at once (and from MAX_FINISHING), so the capsule becomes
│   the picker
├── Picker open (take_keys: Picker) -> Hex's tap hands it ⌥⇧S and Esc; it never
│   takes focus
├── A pick -> CaptureCommand::Start(Window | Area | Display); nothing while one
│   records
├── The picked window closed, or Screen Recording refused -> last outcome
│   `failed` with its reason (`error`)
└── iPhone -> CaptureCommand::Switch(Phone) or Start(Phone), while the snapshot
    names a cabled phone (capture.phone)

A cabled iPhone                          // capture.phone
├── Each capture start, and CaptureCommand::FindPhone (the picker opening) ->
│   a capture-phone thread runs `scrawl-helper devices` (the USB registry, no
│   permission) -> CaptureSnapshot.phone = its name, or None
├── Start(Phone) -> scrawl-helper's mirror window shows the phone and records
│   its native video into recording.mov; `--no-microphone`: the mirror never
│   asks for or opens a microphone
│   ├── Camera access -> macOS asks on Hex's behalf (every bundle carries
│   │   NSCameraUsageDescription; validate-app.sh requires it)
│   └── Pointer and pen input -> normalized to the mirror's picture
├── Narration -> Hex's timeline, as for the screen, moved onto the movie's clock
│   (its first frame, measured from the capture's start) and transcribed by Hex's
│   transcriber (capture.transcriber), not Scrawl's
├── Stop -> as any capture; keyframes decoded from the phone's own video
└── The mirror ends on its own (unplugged, no phone in 12 s, camera denied) ->
    the listener finishes the capture: written if it recorded, else `failed`
    with the helper's reason

The capsule                              // capture.capsule (rune_ui/capture_capsule)
├── A Rune overlay at the top of the pointer's display that never takes focus
│   and lets clicks through except on its live chips, buttons, and list; a
│   new recording brings it to the pointer's display even while it's up on
│   another;
│   its window id goes to the engine (`Host::own_windows`, one id per role, so a
│   recreated window replaces the old one), so recordings leave it out
├── Follows `capture_host::watch()`: Recording from the instant `recording` is
│   Some (the bundle's t0; timer, meter, ✓ finish, ✕ cancel) -> Processing ->
│   done (the lead alone, no word), Failed, or Cancelled, then leaves on its
│   own; a written capture whose transcript has no words reads "no speech"
├── Tones (`feedback::Tone::Capture*`, at the dictation volume; zero is
│   silent): start, stop, a failure
├── Failed -> "Couldn't start" until the engine reports it
│   recording (`screen`), then "Couldn't save". Each keeps its `error`; a start or save that fails
│   while Screen Recording is off (a fresh read) opens the permission helper
│   instead; a start that fails before any snapshot showed it
│   recording still shows, but never over a newer recording, and a capture
│   already under way that fails in the background never takes it over
├── Permission helper (capture.permission) -> "Hex needs Screen Recording ·
│   to see your screen · Allow ✕" (Microphone, Accessibility alike)
│   ├── Allow -> macOS's microphone prompt, else System Settings with the
│   │   floating guide beside it (permission_guide), never at launch
│   ├── Followed once a second (feed::watch_grant, screen_access) ->
│   │   "… is on · Restart Hex" when macOS needs one, then "… is on ·
│   │   Capture ⌥⇧S"; Capture or the shortcut records the pointer's display
│   ├── ✕ closes it; unanswered it folds after 2 min; the shortcut brings it
│   │   back as it stood
│   ├── A capture written with no frames (Screen Recording turned off mid-way)
│   │   -> written with only the voice and marks, its summary noting "No screen:
│   │   Screen Recording is off for Hex", no contact sheet; the helper opens
│   │   once its capsule folds
│   └── Story: hex://story/prototypes/capture-capsule/permission; goldens
│       capture-capsule-permission{,-restart,-ready}
├── A shown recording that leaves with no outcome (a discard, from a chip or
│   `hex capture discard`) -> the capsule leaves; the picker a chip opened
│   stays. A switch that can't start settles the recording it discarded as
│   failed, which the capsule shows as "Couldn't start"
├── Recording -> `● 0:07 │ ✎ │ ■ ✕`: the timer, the toolbar (the pen, its
│   only tool, lit with the accent fill while drawing), a divider, stop
│   (Finish) and ✕ (asks "Discard?" first), all equal square icon buttons
│   with tooltips "Draw", "Stop", "Discard". Goldens: recording,
│   recording-drawing (rendered on apollo)
├── Target chips (playground stories only; feed sends no Input::Chips) ->
│   Area · Window · Screen, plus iPhone while a phone is cabled, for 3.5 s
│   ├── `worked` -> they fold once seen for 0.9 s; never while the pointer is on
│   │   the capsule, and 1.2 s after it leaves
│   ├── A chip restarts rather than retargets (Scrawl's `switch_recording`)
│   ├── Screen -> it's what's recording: the chips fold, no command
│   ├── iPhone -> CaptureCommand::Switch(Phone): the recording restarts on the
│   │   phone
│   └── Window or Area -> Discard; the capsule becomes the picker, and
│       FindPhone looks for a phone cabled since the recording began
├── Picker: ⌥⇧S records what the on-screen picker shows; Esc closes it
├── ✓ -> `hex capture stop`
├── ✕ while recording -> asks first: a destructive "Discard" and "Keep
│   recording", the recording running on; Discard -> `hex capture cancel`.
│   Unanswered, it goes back to recording after 4 s (never while hovered, and
│   1.5 s after the pointer leaves). Esc never discards
├── Keys (capture_host::take_keys), never taking focus: Picker -> ⌥⇧S and
│   Esc through the listener
├── The top line is calm: the label, its note, one action, and ✕. Paste and
│   Inspect are two quiet icons (tooltips "Paste the capture", "Open the
│   capture") for a written capture. Paste and Inspect have no key on either OS.
│   The capsule is a rounded rect with 14 pt
│   corners (`SHELL_RADIUS`), not a pill, and its chips and icon buttons
│   round concentrically inside it. A capture that only copied has no
│   label, the lead alone
├── The lead is the capture's first frame (`frames/frame-01.png`, else the
│   contact sheet), cropped to a small rounded card once the bundle is
│   written and decoded off the UI thread; until then, or without one, a
│   check. A warning keeps its glyph
│   ├── Paste -> CaptureCommand::Paste: the contact sheet, then the summary,
│   │   into the app in front
│   └── Inspect -> CaptureCommand::Inspect: opens the contact sheet (Hex
│       has no review window, and opening the bundle could launch
│       Scrawl.app)
├── Every change moves at once, as Dynamic Island's: the walls and what stays
│   travel on `island` (0.30 s, bounce 0.12), while everything leaving and
│   everything arriving blurs (`content_blur` 3 pt), scales toward the
│   capsule's middle (`content_scale` 0.92), and fades over that same move
│   (`content_in` 0.30 s, `content_out` 0.24 s). Nothing staggers, and nothing
│   waits for a wall or for other content; only an entry takes two frames
│   (the origin pose, then rest). The playground's Transition card tunes all of it
├── Reduce Motion -> travel jumps, nothing blurs or scales; opacity still fades
└── The playground's "Prototypes/Capture capsule/family" story (playground/family.rs)
    plays a dictation and then a capture in one shape on the same springs:
    from the notch on a 14-inch MacBook Pro, and on a flat 27-inch display
    inside the menu bar (hud_placement's 3 pt inset) or floating 6 pt under
    it. Its variants draw the voice as Dot scope's braid (its real
    simulation), bars, or a line; only the capture adds a red dot or a small
    screen, the cue alternating each loop

The on-screen picker                     // capture.picker (capture_capsule/picker.rs)
├── A full-screen Rune overlay on the pointer's display, under the capsule,
│   left out of recordings (`OwnWindow::Picker`); it takes the pointer only while
│   picking and never takes focus; it follows the pointer to another display
├── Everything but the target dims; the dim moves with the target as one piece
├── Window -> the window under the pointer: tinted, ringed, its title in a badge;
│   the cursor becomes Scrawl's camera; no window under it -> the display
├── Area -> drag at least 36 pt; the badge shows its size; a click alone -> the
│   display
├── Screen -> the display, ringed
│   └── An area's ring is white with a dark hairline outside it, so it reads
│       over white content too (Scrawl's vanishes there)
├── Click or ⌥⇧S -> Action::Start(target) -> CaptureCommand::Start; the picker
│   fades; a pick that starts no capture within 2 s brings it back
└── Windows load off the UI thread (`list_selectable_windows`); nothing picks
    until they arrive

Hex's own windows                        // capture.own-windows
├── Chrome (the capsule, ink, picker, HUDs, meeting capsules: Hex's named
│   windows and any of its windows above the normal level) -> never recorded,
│   never in the window log, never the target, clicks on it not logged
│   ├── Other recorders -> the capsule, picker, and ink are left out
│   │   (sharingType none) while shown; the dictation HUD and meeting
│   │   capsules show
│   │   └── Hidden -> ordered out of every Space once the hide finishes,
│   │       before sharingType returns to readOnly; prewarming keeps the
│   │       surface ready but leaves no invisible panel ordered in. This
│   │       avoids relying on macOS restoring sharing after exclusion
│   ├── Show in screen recordings on -> every recorder, Hex's captures
│   │   included, shows all of it (sharingType readOnly, no SCK exclusions);
│   │   still never in the window log or the target. Rune's
│   │   OverlayHandle::set_exclude_from_capture flips open windows, and the
│   │   sampler rebuilds its filter mid-recording (OwnWindows::recorded) and
│   │   the recording's (capture-core's Recording::set_exclusions)
│   └── Rune's presence tests check ordering and sharing together, including
│       stale fade completions and setting changes. Its native `spaces_probe`
│       checks this process's WindowServer ordering. A native Hex smoke on
│       2026-11-02 failed before the pin bump and passed after it: capsule,
│       Screen/Area picker, ink, dictation HUD and meeting capsule show and
│       leave no onscreen window after hiding. This used isolated test state,
│       without recording. Mission Control's actual thumbnail report and
│       third-party recording exclusion remain unverified
│   └── Click-through chrome (the full-screen ink overlay, the capsule's
│       640×424 panel outside its pill) -> clicks there count for the app
│       beneath; only the pill's (capture_host::set_capsule_pill) are Hex's
└── Normal windows (Settings) -> like any app's: recorded, logged, pickable,
    and Hex is the front app when its window is the topmost normal one. A
    capsule Paste still goes to the last other app, never into Hex

Ink and laser                            // capture.ink
├── A click-through overlay over the recorded display, excluded from the recording
├── Shift strokes draw as ink, hold 650 ms, then fade over 650 ms
└── The laser's tail lasts 420 ms; nothing lags the pointer

While a capture records                  // capture.input
├── The existing session tap adds mouse down/up/drag/scroll and Shift (no second tap)
├── -> Capture::input() through a bounded queue; full -> events dropped and counted
├── Keys pressed in the app -> logged; keys Hex consumes (its shortcuts, Esc) and
│   anything while a shortcut is being recorded -> not logged
├── Scrawl's rules in the engine (hosted_input): Shift drag inks, a
│   Shift-click is part of the drawing (no pins, no ⌘Z undo), clicks on
│   Hex's capture chrome ignored; while drawing, capture_host::as_pen makes
│   every pointer event Shift ink
└── Shift still held from ⌥⇧S -> must come up once before it can ink; Shift up
    at the start -> the first Shift drag inks

hex capture start                        // capture.start; needs a listening Hex
├── Creates <captures folder>/<unix ms>.scrawl (owner-only) -> reply: recording + bundle path
├── Two stopped captures still writing -> refused `busy`
├── Copies the dictation timeline from now into a bounded sink (no second stream)
├── In the background: Scrawl's engine starts on the active display
│   ├── Needs Screen Recording for Hex (or the terminal running it)
│   └── Fails to start -> sink closed, bundle removed, last outcome `failed`;
│       Esc and the next ⌥⇧S see nothing recording
├── Release microphone while idle -> the capture keeps the microphone open
└── Already recording -> unchanged, same bundle

While it records                         // capture.audio
├── Dictation and meetings work as usual on the same audio
├── Sink full (collector stalled) -> capture audio dropped as a gap, dictation untouched
└── Microphone gap or recovery -> the gap is recorded, positions stay on the clock

Never on Hex's path                      // capture.isolation
├── No microphone, event tap, global shortcut, or model of its own
├── No ingest server (47777) or recovery workers
├── No ingest token used; Scrawl's two config files read once by the import, never
│   written; nothing else of Scrawl's touched (its log goes to Hex's tracing)
└── Settings from Hex (capture.*), not Scrawl's config: auto-crop on (display
    captures), the agentContext summary format off

Retention                                // capture.retention
└── Hex's own thread: Scrawl's retention pass with capture.*'s policy, 90 s after
    captures turn on, then hourly; skipped while a capture records or writes,
    or while Scrawl.app runs (it cleans the shared folder then)

hex capture stop                         // capture.stop
├── Closes the sink (≤ 100 ms wait) -> reply: idle + the bundle path, at once
├── The screen recording, window log, and input stop at once (Capture::stop)
├── recording.mp4 finishes after that on capture-core's own thread: detached
│   before its stream stops, its final frame held to the stop, capture.json
│   written beside it; only the steps that read the movie wait for it, and it
│   starts where its first frame was shown (video_offset_ms)
└── In the background, on one capture-finish thread, one capture at a time:
    ├── 16 kHz audio with its start offset and gaps -> SealedCapture::finish
    ├── Words: 30 s chunks, each a foreground job on the dictation engine's model,
    │   after queued dictation, before meeting chunks; waits while dictation records
    │   or transcribes; never interrupts a live stream
    ├── Word times moved earlier by the model's measured lag (Parakeet Unified
    │   English: starts −280 ms, −520 ms after a ≥ 250 ms pause; ends −195 ms);
    │   other models and Apple keep their own times
    ├── Past 20 min -> audio kept to 20 min, the rest one gap
    ├── recording.mp4, capture.json, audio.wav, frames/, scrawl.json,
    │   summary.md; then every file 0600 and folder 0700
    ├── Keyframes decoded from recording.mp4 by scrawl-helper (beside `hex`: in
    │   Hex.app's Contents/MacOS or target/<profile>; or $SCRAWL_HELPER); helper
    │   missing -> the 480 px sampled frames
    ├── Spoken cues (look, now, here, this…) pick the keyframe the screen
    │   changes to; they also count in Spanish (mira, ahora, aquí, esto…),
    │   French, German, Portuguese, and Italian, whatever the dictation
    │   language. Check: scrawl-capture keyframes
    │   spoken_cues_count_in_other_languages
    ├── Transcription fails -> logged; the bundle has no words, and the outcome says why
    └── Written -> image and text copied to the clipboard (Scrawl's handoff)

hex capture list|show                    // capture.cli; docs/cli.md
├── list [--limit N] -> the newest bundles in the captures folder: mode, words
├── show <capture> -> one capture's words and files
└── <capture> = bundle name, path, or `latest`; unknown -> `not-found`

hex capture cancel -> engine cancelled, sink closed, bundle removed
hex capture status -> recording (bundle, elapsed), bundles still writing, last outcome
                      (words, audio gaps, or why it has no transcript)
Hex quits mid-capture -> cancelled, bundle removed; captures still writing stop
transcribing between chunks and keep their audio (shutdown waits ≤ 2 s)
```

`scrawl.json` is Scrawl's bundle manifest. Its `transcript.words` are
`{t0, t1, text}` in milliseconds from the capture's start (`t0_boot_ns`, the
boot-time clock audio and shortcuts share), and `audio.wav` starts at that
start too, padded with silence when the microphone began late or dropped
audio.

`hex dev capture-replay <wav>…` (debug, with the feature) runs the same
capture host on a fresh transcription engine with the selected model, feeding
WAV files instead of the microphone, and prints the bundle path. It needs
Annotate on too, and records the screen meanwhile.

Sources: [capture_host.rs](../../src/capture_host.rs), the tap in
[suppression.rs](../../src/suppression.rs), the shortcut and Escape in
[recognition.rs](../../src/recognition.rs), `CaptureSettings` and the import in
[app_settings.rs](../../src/app_settings.rs), the pane in
[rune_ui/scrawl.rs](../../src/rune_ui/scrawl.rs), the input rules in
[hosted_input.rs](../../crates/capture/src/hosted_input.rs), the sink in
[dictation/audio.rs](../../src/dictation/audio.rs),
`transcribe_with_words` in [parakeet.rs](../../src/parakeet.rs), the CLI's reads in
[capture_host/library.rs](../../src/capture_host/library.rs), the phone finish
in [phone.rs](../../crates/capture/src/phone.rs), and the `scrawl-capture`
control commands in [control.rs](../../src/control.rs).

Checks:

- Gate: `the_host_is_absent_while_the_experiment_is_off` in capture_host.rs;
  [test-app-identity.sh](../../scripts/test-app-identity.sh) for the
  release-path refusals.
- Sink, in dictation/audio.rs: `capture_sink_shares_the_timeline_with_dictation`,
  `a_full_capture_sink_drops_audio_as_a_gap_and_never_stalls_dictation`,
  `capture_sink_records_microphone_gaps`,
  `capture_sink_holds_a_released_microphone_open_until_it_closes`,
  `capture_sink_keeps_every_frame_through_timestamp_jitter`, and
  `a_dropped_capture_receiver_closes_the_sink`.
- Words, in parakeet.rs: `chunked_word_transcription_keeps_word_times_across_chunks`
  and `word_transcription_is_a_foreground_request_on_the_shared_engine`;
  `foreground_work_waits_for_an_open_stream_instead_of_interrupting_it` in
  [transcription/engine.rs](../../src/transcription/engine.rs).
- Host, in capture_host.rs (a fake engine that writes through Scrawl's real
  `finalize_capture_bundle_with_flags`, without a screen):
  `controls_are_idempotent_and_stop_writes_a_scrawl_bundle`,
  `an_engine_that_cannot_start_leaves_nothing_behind`,
  `a_screen_source_that_gives_up_ends_the_capture`,
  `scrawls_engine_claims_nothing_machine_wide`,
  `the_timeline_resamples_pads_gaps_and_ends_at_the_stop`,
  `a_pending_gap_and_a_stalled_tail_come_back_from_the_closed_sink`,
  `padding_past_the_length_limit_stays_bounded`,
  `captures_finish_one_at_a_time_and_starts_wait_for_room`,
  `a_capture_queued_behind_a_slow_finish_has_already_stopped`,
  `a_failed_start_raced_by_stop_never_wedges_later_starts`,
  `the_engine_config_comes_from_hex`, and
  `shutdown_cancels_the_recording_capture_and_waits_for_its_cleanup`.
- Shortcut and input, in capture_host.rs: `the_host_is_absent_while_captures_are_off`,
  `the_shortcut_starts_and_a_later_press_stops`,
  `input_reaches_only_the_capture_that_is_recording`, `tap_flags_become_modifiers`,
  `the_shortcut_records_once_its_keys_come_up`,
  `a_failed_start_ends_before_escape_or_the_shortcut_sees_it`;
  `the_capture_shortcut_is_suppressed_and_never_chord_evidence` and
  `a_capture_logs_only_keys_that_reach_the_app` in suppression/tests.rs;
  `only_the_capsules_pill_and_not_its_panel_takes_clicks` in capture_host.rs;
  `a_paste_goes_to_an_external_app_never_the_host` in capture/macos.rs;
  `the_hosts_normal_windows_count_and_its_chrome_does_not` in
  [window_log.rs](../../crates/capture/src/window_log.rs) and
  `helper_windows_are_scrawl_s_except_the_capture_source` in
  [capture.rs](../../crates/capture/src/capture.rs) for Hex's own windows;
  the ink and click rules in hosted_input.rs, including
  `a_capture_started_by_its_shortcut_waits_for_shift_to_come_up`,
  `a_capture_started_without_shift_held_inks_from_the_first_stroke`, and
  `a_click_with_the_chords_shift_still_held_is_no_pin`, and in
  capture.rs `a_shift_click_is_part_of_the_drawing`.
- iPhone: `an_iphone_capture_narrates_with_hex_audio_and_finishes_when_its_mirror_ends`
  and `a_mirror_ending_never_stops_a_screen_capture` in capture_host.rs;
  `since_moves_audio_and_gaps_onto_a_later_clock` in
  [audio.rs](../../crates/capture/src/audio.rs). The mirror, its
  `--no-microphone`, and the camera prompt have not run in Hex.
- CLI: `a_capture_on_disk_summarizes_its_mode_words_and_send` in
  [library.rs](../../src/capture_host/library.rs), the protocol round trips in
  control.rs, and `control_commands_parse` in main.rs.
- Switching, in capture_host.rs:
  `a_discarded_capture_leaves_no_trace_and_a_pick_takes_its_place`,
  `a_switch_never_shows_nothing_recording`,
  `a_switch_with_no_room_to_start_keeps_the_recording`,
  `a_switch_that_cannot_start_after_its_discard_settles_failed`,
  `discarded_captures_never_count_against_the_writers`,
  `a_pick_that_cannot_start_says_why`,
  `the_first_click_or_scroll_means_work_started`, and
  `the_newest_grab_takes_only_its_keys_until_it_drops`. Paste and Inspect
  need a written capture
  (`controls_are_idempotent_and_stop_writes_a_scrawl_bundle`); neither has
  pasted into or opened anything in a test.
- Capsule, in rune_ui/capture_capsule (feature builds only): the flow tests
  (`chips_fold_once_seen_when_work_starts_but_never_under_the_pointer`,
  `the_shortcut_records_the_pickers_screen_and_a_chip_discards_the_recording`,
  …), feed tests
  (`the_shortcuts_recording_shows_no_target_chips`,
  `the_pen_lights_while_drawing`,
  `a_cabled_phone_shows_its_chip`, …),
  `the_capsule_takes_only_the_keys_its_state_needs`,
  `a_written_copy_offers_paste_and_inspect_as_icons`,
  `every_change_moves_at_once`,
  `entry_lands_the_pose_instantly_then_springs_to_rest_with_its_content`,
  goldens `tests/golden/capture-capsule-*.png`, and
  `the_capsule_moves_inside_its_panel_and_comes_to_rest`, which traces every
  scripted state change per frame, Calm and Reduce Motion, for no pops,
  jumps, or jiggle. `HEX_CAPSULE_MOTION` also writes rune-motion traces for
  motion-lint. `cues_mark_start_stop_sent_and_taking_back`,
  `nothing_heard_says_so`, and `a_capture_with_no_words_says_nothing_was_heard`
  cover the tones and the empty transcript. Observed (Oct 27, worker, r19 motion): all 28
  capsule runs, Calm and Reduce Motion, pass lint.
- Playground: `hex capsule-playground` (feature builds; hidden), or
  `scripts/build-capsule-playground.sh` for an ad-hoc signed
  `Hex Playground.app`, runs the real capsule in a window of its own against a
  fake host, with state buttons, the whole flow, slow motion, Reduce Motion,
  the tones, and live tuning ([tuning.rs](../../src/rune_ui/capture_capsule/tuning.rs);
  "Copy values" copies its `DEFAULT`). It reads and writes no Hex data and
  records or sends nothing.
  `--snap --scene recording` renders it. The tones were checked by waveform and level only, never on
  speakers.
- Picker, in capture_capsule/picker.rs and tests.rs: geometry, label, and
  target tests, `the_picker_records_what_it_shows_and_comes_to_rest`, and
  `picker_stills_and_motion`, which scripts window hover, an area drag, and
  mode switches per frame (Calm and Reduce Motion). Observed (Oct 26, pro):
  all 6 runs pass motion-lint with two excuses: the target growing from a
  zero-size point, and the camera stopping where the scripted pointer stops.
  Not proven natively end to end: the overlays have been driven only by the
  scripted scenes, not a live capture.
- Pane: `captures_default_off_and_refuse_a_shortcut_in_use` and
  `retention_names_presets_and_imported_days` in rune_ui/scrawl.rs; scenes
  `mac-scrawl-off`, `mac-scrawl`, `mac-scrawl-running`, `mac-scrawl-permission`
  (feature builds only, so no goldens: those run on Linux).
- Settings: every `capture.*` key in `hex settings`
  (`every_persisted_setting_has_a_cli_key`).

Observed 2026-10-25 on pro (M2 Max), before the real engine, with the
stand-in: `HEX_EXPERIMENTS=capture hex dev capture-replay` over three
vocabulary fixtures (6.5 s) loaded Parakeet unified English on Metal and
wrote 18 words with plausible times. The real engine has not run end to end:
the unit tests use a fake engine over Scrawl's real bundle writer, and the
mini (the test Mac) has no Screen Recording grant for a dev build, so neither
`hex capture` against a live app nor `capture-replay` has recorded a screen.
The shortcut, tap input, Escape, switching, the iPhone path, Scrawl
coexistence, and import are covered by unit tests only; none has run against a
live Hex.
