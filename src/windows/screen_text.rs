//! Hexium: the foreground window's text and the focused field's text on
//! Windows, read through UI Automation for `dictation::screen_text`. The
//! window's visible elements come back in one cached call (their names);
//! the field is read through its Text pattern, else its Value pattern.

use std::time::Instant;

use windows::Win32::Foundation::{HWND, VARIANT_FALSE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, IUIAutomationValuePattern,
    TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, TreeScope_Descendants,
    UIA_IsOffscreenPropertyId, UIA_IsPasswordPropertyId, UIA_NamePropertyId, UIA_TextPatternId,
    UIA_ValuePatternId,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use crate::dictation::screen_text::{
    Collector, FieldText, MAX_FIELD_CHARS, MAX_NODES, ScreenText, WINDOW_BUDGET,
};

/// The visible text of the foreground window.
pub(crate) fn window_text() -> Option<ScreenText> {
    let started = Instant::now();
    with_automation(|automation| unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return Ok(None);
        }
        let root = automation.ElementFromHandle(HWND(window.cast()))?;
        let cache = automation.CreateCacheRequest()?;
        cache.AddProperty(UIA_NamePropertyId)?;
        cache.AddProperty(UIA_IsPasswordPropertyId)?;
        let visible =
            automation.CreatePropertyCondition(UIA_IsOffscreenPropertyId, &variant_false())?;
        let elements = root.FindAllBuildCache(TreeScope_Descendants, &visible, &cache)?;
        let mut collector = Collector::default();
        if let Ok(title) = root.CurrentName() {
            collector.push(&title.to_string());
        }
        let count = elements.Length()?.min(MAX_NODES as i32);
        for index in 0..count {
            if collector.full() || started.elapsed() > WINDOW_BUDGET {
                break;
            }
            let Ok(element) = elements.GetElement(index) else {
                continue;
            };
            if element
                .CachedIsPassword()
                .is_ok_and(|secret| secret.as_bool())
            {
                continue;
            }
            if let Ok(name) = element.CachedName() {
                collector.push(&name.to_string());
            }
        }
        Ok(Some(collector.finish()))
    })
}

/// The focused element's text and insertion point; `None` for a password
/// field or an element without text.
pub(crate) fn focused_field() -> Option<FieldText> {
    with_automation(|automation| unsafe {
        let element = automation.GetFocusedElement()?;
        if element.CurrentIsPassword()?.as_bool() {
            return Ok(None);
        }
        let identity = format!(
            "{}\u{1f}{:?}\u{1f}{}\u{1f}{}\u{1f}{}",
            element.CurrentProcessId().unwrap_or_default(),
            GetForegroundWindow(),
            element
                .CurrentControlType()
                .map(|kind| kind.0)
                .unwrap_or_default(),
            element
                .CurrentAutomationId()
                .map(|id| id.to_string())
                .unwrap_or_default(),
            element
                .CurrentName()
                .map(|name| name.to_string())
                .unwrap_or_default(),
        );
        if let Ok(pattern) =
            element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
        {
            let document = pattern.DocumentRange()?;
            let text = unix_newlines(&document.GetText(MAX_FIELD_CHARS as i32)?.to_string());
            // The text from the document's start to the selection's start.
            let caret = (|| -> windows::core::Result<Option<usize>> {
                let selection = pattern.GetSelection()?;
                if selection.Length()? < 1 {
                    return Ok(None);
                }
                let before = document.Clone()?;
                before.MoveEndpointByRange(
                    TextPatternRangeEndpoint_End,
                    &selection.GetElement(0)?,
                    TextPatternRangeEndpoint_Start,
                )?;
                Ok(Some(
                    unix_newlines(&before.GetText(-1)?.to_string())
                        .chars()
                        .count(),
                ))
            })()
            .ok()
            .flatten();
            return Ok(Some(FieldText {
                text,
                caret,
                identity,
            }));
        }
        if let Ok(pattern) =
            element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
        {
            let text = unix_newlines(&pattern.CurrentValue()?.to_string());
            return Ok(
                (text.chars().count() <= MAX_FIELD_CHARS).then_some(FieldText {
                    text,
                    caret: None,
                    identity,
                }),
            );
        }
        Ok(None)
    })
}

/// `text` with `\n` line breaks: rich-edit controls (Notepad, WordPad, many
/// native apps) report a break as `\r`, others as `\r\n`, but a pasted
/// dictation is matched with `\n`.
fn unix_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Runs `read` with a UI Automation client on this thread's COM apartment.
fn with_automation<T>(
    read: impl FnOnce(&IUIAutomation) -> windows::core::Result<Option<T>>,
) -> Option<T> {
    // SAFETY: COM is initialized for this thread and uninitialized after the
    // client and every element it made are dropped inside the closure.
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED);
        let value = (|| {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            read(&automation)
        })();
        if com.is_ok() {
            CoUninitialize();
        }
        match value {
            Ok(value) => value,
            Err(error) => {
                tracing::info!(%error, "UI Automation could not read the screen text");
                None
            }
        }
    }
}

/// A VT_BOOL VARIANT holding false; it owns nothing, so the ManuallyDrop
/// payload never needs dropping.
fn variant_false() -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BOOL,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 {
                    boolVal: VARIANT_FALSE,
                },
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_breaks_read_as_newlines() {
        assert_eq!(unix_newlines("a\r\nb\rc\nd"), "a\nb\nc\nd");
        assert_eq!(unix_newlines("\r\n\r\n"), "\n\n");
        assert_eq!(unix_newlines("plain"), "plain");
    }

    #[test]
    fn a_multi_line_dictation_is_found_in_a_rich_edit_field() {
        // Notepad reports the pasted "\n" as "\r".
        let dictated = "Notes for today:\nthe hexy um build is out.";
        let first = FieldText {
            text: unix_newlines("Notes for today:\rthe hexy um build is out."),
            caret: None,
            identity: String::new(),
        };
        let now = unix_newlines("Notes for today:\rthe Hexium build is out.");
        assert_eq!(
            crate::dictation::fix_learning::fixes_between(&first, dictated, &now),
            [("hexy um".to_string(), "Hexium".to_string())]
        );
    }
}

/// Live checks against real Windows apps, run on a Windows desktop with
/// `cargo test --bin hex -- --ignored --nocapture --test-threads=1 windows::screen_text::live`.
/// Each one takes the foreground for up to a minute, so keep your hands off
/// the PC while it runs; a check whose window loses the front fails with
/// "focus moved away" rather than reading or typing into another window.
#[cfg(test)]
mod live {
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
        IUIAutomationSelectionItemPattern, IUIAutomationTextPattern, IUIAutomationValuePattern,
        TreeScope_Descendants, UIA_ButtonControlTypeId, UIA_CONTROLTYPE_ID, UIA_InvokePatternId,
        UIA_SelectionItemPatternId, UIA_TabItemControlTypeId, UIA_TextPatternId,
        UIA_ValuePatternId,
    };
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        KEYEVENTF_KEYUP, VK_ESCAPE, VK_MENU, keybd_event,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowTextW, IsWindowVisible,
        PostMessageW, SetForegroundWindow, WM_CLOSE,
    };

    const DICTATED: &str = "I pushed the hexy um release today.";
    const FIXED: &str = "I pushed the Hexium release today.";
    /// The Notepad check's file, in the temp folder.
    const NOTEPAD_FILE: &str = "hexium-live.txt";

    fn automation() -> IUIAutomation {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).unwrap()
        }
    }

    fn title_of(window: HWND) -> String {
        let mut buffer = [0_u16; 512];
        let length = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), 512) };
        String::from_utf16_lossy(&buffer[..length.max(0) as usize])
    }

    /// The visible top-level window whose title holds `title`.
    fn window_titled(title: &str) -> Option<HWND> {
        struct Search<'a>(&'a str, HWND);
        unsafe extern "system" fn visit(window: HWND, search: LPARAM) -> i32 {
            let search = unsafe { &mut *(search as *mut Search) };
            if unsafe { IsWindowVisible(window) } != 0 && title_of(window).contains(search.0) {
                search.1 = window;
                return 0;
            }
            1
        }
        let mut search = Search(title, std::ptr::null_mut());
        unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM) };
        (!search.1.is_null()).then_some(search.1)
    }

    /// Brings `title`'s window to the front, waiting up to `wait` for it to
    /// appear and take the foreground.
    fn focus(title: &str, wait: Duration) -> HWND {
        let started = Instant::now();
        let mut tries = 0;
        loop {
            if let Some(window) = window_titled(title) {
                tries += 1;
                unsafe {
                    if tries > 1 && GetForegroundWindow() != window {
                        // An Alt tap lets a background process take the
                        // foreground, but it also moves focus to the app's
                        // menu (Chromium's app-menu button, a menu bar), so
                        // Esc hands focus back to the page or document.
                        keybd_event(VK_MENU as u8, 0, 0, 0);
                        keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
                        SetForegroundWindow(window);
                        std::thread::sleep(Duration::from_millis(300));
                        if GetForegroundWindow() == window {
                            keybd_event(VK_ESCAPE as u8, 0, 0, 0);
                            keybd_event(VK_ESCAPE as u8, 0, KEYEVENTF_KEYUP, 0);
                        }
                    } else {
                        SetForegroundWindow(window);
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
                if unsafe { GetForegroundWindow() } == window {
                    // Let the app settle on its focused element.
                    std::thread::sleep(Duration::from_millis(1_000));
                    return window;
                }
            }
            assert!(
                started.elapsed() < wait,
                "no window titled {title} came to the front: {}",
                describe_focus()
            );
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Fails unless the test's own window is still in front, so nothing is
    /// read from or typed into a window the check didn't open.
    fn assert_in_front(window: HWND) {
        assert!(
            unsafe { GetForegroundWindow() } == window,
            "focus moved away from the test window: {}",
            describe_focus()
        );
    }

    /// What has focus, for a failed read: the foreground window's title and
    /// class, and the focused element's process, control type, class,
    /// framework, and whether it has the Text and Value patterns.
    fn describe_focus() -> String {
        let foreground = unsafe { GetForegroundWindow() };
        let mut buffer = [0_u16; 256];
        let length = unsafe { GetClassNameW(foreground, buffer.as_mut_ptr(), 256) };
        let title = format!(
            "{:?} ({})",
            title_of(foreground),
            String::from_utf16_lossy(&buffer[..length.max(0) as usize])
        );
        let element = match unsafe { automation().GetFocusedElement() } {
            Ok(element) => element,
            Err(error) => return format!("window {title}; no focused element: {error}"),
        };
        unsafe {
            format!(
                "window {title}; focused pid {} type {} class {:?} framework {:?} text {} value {}",
                element.CurrentProcessId().unwrap_or_default(),
                element
                    .CurrentControlType()
                    .map(|kind| kind.0)
                    .unwrap_or_default(),
                element
                    .CurrentClassName()
                    .map(|name| name.to_string())
                    .unwrap_or_default(),
                element
                    .CurrentFrameworkId()
                    .map(|name| name.to_string())
                    .unwrap_or_default(),
                element
                    .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                    .is_ok(),
                element
                    .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                    .is_ok(),
            )
        }
    }

    /// One read the way a watch does it (on a helper thread, with the
    /// watch's deadline), timed.
    fn timed_read() -> Option<crate::dictation::screen_text::FieldText> {
        let started = Instant::now();
        let field = crate::dictation::screen_text::focused_field(Duration::from_millis(1_500));
        println!("read took {:?}", started.elapsed());
        field
    }

    /// Replaces the focused element's text, as a person's edit would.
    fn set_focused_text(text: &str) {
        unsafe {
            let element = automation().GetFocusedElement().unwrap();
            let pattern = element
                .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                .unwrap();
            pattern.SetValue(&windows::core::BSTR::from(text)).unwrap();
        }
    }

    /// Reads the field in `window`, edits it, reads it again, and judges
    /// the fix.
    fn learns_in_focused_field(window: HWND, edit: impl FnOnce(&str)) {
        let mut first = None;
        for _ in 0..10 {
            assert_in_front(window);
            let read = timed_read();
            first = read.clone().filter(|field| field.text.contains(DICTATED));
            if first.is_some() {
                break;
            }
            println!("focused field so far: {read:?}; {}", describe_focus());
            std::thread::sleep(Duration::from_secs(1));
        }
        let first = first.expect("the focused field holds the dictation");
        println!("first read: {first:?}");
        assert_in_front(window);
        edit(FIXED);
        std::thread::sleep(Duration::from_millis(1_000));
        assert_in_front(window);
        let now = timed_read().expect("the field reads after the edit");
        println!("after the edit: {now:?}");
        assert_eq!(now.identity, first.identity, "the same element");
        assert_eq!(
            crate::dictation::fix_learning::fixes_between(&first, DICTATED, &now.text),
            [("hexy um".to_string(), "Hexium".to_string())]
        );
    }

    /// A process a check started, with its output detached (an inherited
    /// pipe would keep `cargo test` waiting on it); its whole process tree
    /// ends when the check does, pass or fail.
    struct Launched(Child);

    impl Launched {
        fn new(command: &mut Command) -> Self {
            Self(
                command
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            )
        }
    }

    impl Drop for Launched {
        /// Asks the app to close first, so a browser doesn't offer to
        /// restore a crashed session next time, then ends what's left.
        fn drop(&mut self) {
            let pid = self.0.id().to_string();
            let taskkill = |force: bool| {
                let mut args = vec!["/T", "/PID", pid.as_str()];
                if force {
                    args.insert(0, "/F");
                }
                let _ = Command::new("taskkill")
                    .args(args)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            };
            taskkill(false);
            let asked = Instant::now();
            while asked.elapsed() < Duration::from_secs(5) {
                if self.0.try_wait().is_ok_and(|status| status.is_some()) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            taskkill(true);
            let _ = self.0.wait();
        }
    }

    /// The first element under `root` of `kind` whose name passes `named`.
    fn find(
        automation: &IUIAutomation,
        root: &IUIAutomationElement,
        kind: UIA_CONTROLTYPE_ID,
        named: impl Fn(&str) -> bool,
    ) -> windows::core::Result<Option<IUIAutomationElement>> {
        unsafe {
            let all = root.FindAll(TreeScope_Descendants, &automation.CreateTrueCondition()?)?;
            for index in 0..all.Length()? {
                let element = all.GetElement(index)?;
                if element.CurrentControlType()? == kind
                    && named(&element.CurrentName()?.to_string())
                {
                    return Ok(Some(element));
                }
            }
            Ok(None)
        }
    }

    /// Whether Notepad is running, before the check starts it. Unknown
    /// counts as running, so a window that may be someone's stays open.
    fn notepad_running() -> bool {
        Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Notepad.exe", "/NH"])
            .output()
            .map_or(true, |output| {
                String::from_utf8_lossy(&output.stdout)
                    .to_lowercase()
                    .contains("notepad.exe")
            })
    }

    /// The Notepad check's cleanup. Windows 11's `notepad.exe` hands the
    /// file to the packaged Notepad and exits, so there's no child to end:
    /// the check's own tab is closed without saving, and the window only
    /// when Notepad wasn't running before (it then holds just the tabs
    /// Notepad restores, which it keeps for next time).
    struct Notepad {
        window: Option<HWND>,
        path: PathBuf,
        was_running: bool,
    }

    impl Drop for Notepad {
        fn drop(&mut self) {
            if let Some(window) = self.window {
                match close_tab(window, NOTEPAD_FILE) {
                    Ok(true) => println!("closed the check's Notepad tab"),
                    Ok(false) => println!("the check's Notepad tab wasn't found to close"),
                    Err(error) => println!("closing the check's Notepad tab failed: {error}"),
                }
                if !self.was_running {
                    unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
                }
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Closes `window`'s tab named `name` through its own Close button,
    /// answering "Don't save" if Notepad asks.
    fn close_tab(window: HWND, name: &str) -> windows::core::Result<bool> {
        let automation = automation();
        unsafe {
            let root = automation.ElementFromHandle(windows::Win32::Foundation::HWND(window))?;
            let Some(tab) = find(&automation, &root, UIA_TabItemControlTypeId, |tab| {
                tab.starts_with(name)
            })?
            else {
                return Ok(false);
            };
            if let Ok(selection) = tab.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(
                UIA_SelectionItemPatternId,
            ) {
                selection.Select()?;
                std::thread::sleep(Duration::from_millis(500));
            }
            let Some(close) = find(&automation, &tab, UIA_ButtonControlTypeId, |button| {
                button.contains("Close")
            })?
            else {
                return Ok(false);
            };
            close
                .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)?
                .Invoke()?;
            std::thread::sleep(Duration::from_millis(1_000));
            let dont_save = find(&automation, &root, UIA_ButtonControlTypeId, |button| {
                button
                    .replace('\u{2019}', "'")
                    .eq_ignore_ascii_case("don't save")
            })?;
            if let Some(dont_save) = dont_save {
                dont_save
                    .GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)?
                    .Invoke()?;
            }
            Ok(true)
        }
    }

    #[test]
    #[ignore]
    fn notepad() {
        let path = std::env::temp_dir().join(NOTEPAD_FILE);
        std::fs::write(&path, DICTATED).unwrap();
        let mut cleanup = Notepad {
            window: None,
            path: path.clone(),
            was_running: notepad_running(),
        };
        let mut launcher = Command::new("notepad.exe")
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // Waited for off the test's thread: the alias process can live as
        // long as the window.
        std::thread::spawn(move || launcher.wait());
        let window = focus("hexium-live", Duration::from_secs(20));
        cleanup.window = Some(window);
        // Not printed: Notepad's other tabs may be someone's files.
        let pieces = super::window_text().map_or(0, |text| text.pieces.len());
        println!("window text: {pieces} pieces");
        learns_in_focused_field(window, set_focused_text);
    }

    #[test]
    #[ignore]
    fn edge() {
        let edge = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";
        let profile = std::env::temp_dir().join("hexium-live-edge");
        let page = format!(
            "data:text/html,<title>Hexium live</title><h1>Indian-language features nobody else has</h1><textarea autofocus>{DICTATED}</textarea>"
        );
        // A guest window: no sign-in or personalization prompt, which a new
        // profile opens in a window of its own over the page.
        let _browser = Launched::new(
            Command::new(edge)
                .arg("--no-first-run")
                .arg("--no-default-browser-check")
                .arg("--guest")
                .arg(format!("--user-data-dir={}", profile.display()))
                .arg("--new-window")
                .arg(page),
        );
        let window = focus("Hexium live", Duration::from_secs(60));
        let mut text = None;
        for _ in 0..10 {
            assert_in_front(window);
            text = super::window_text().filter(|text| {
                text.pieces
                    .iter()
                    .any(|piece| piece.contains("Indian-language features nobody else has"))
            });
            if text.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        println!("window: {text:?}");
        assert!(text.is_some(), "the page's heading is read");
        learns_in_focused_field(window, set_focused_text);
    }
}
