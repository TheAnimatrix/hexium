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
            let text = document.GetText(MAX_FIELD_CHARS as i32)?.to_string();
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
                Ok(Some(before.GetText(-1)?.to_string().chars().count()))
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
            let text = pattern.CurrentValue()?.to_string();
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

/// Live checks against real Windows apps, run on a Windows desktop with
/// `cargo test --bin hex -- --ignored --nocapture windows::screen_text::live`.
#[cfg(test)]
mod live {
    use std::time::Duration;

    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationValuePattern, UIA_ValuePatternId,
    };
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, VK_MENU, keybd_event};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, IsWindowVisible, SetForegroundWindow,
    };

    const DICTATED: &str = "I pushed the hexy um release today.";
    const FIXED: &str = "I pushed the Hexium release today.";

    /// The visible top-level window whose title holds `title`.
    fn window_titled(title: &str) -> Option<HWND> {
        struct Search<'a>(&'a str, HWND);
        unsafe extern "system" fn visit(window: HWND, search: LPARAM) -> i32 {
            let search = unsafe { &mut *(search as *mut Search) };
            let mut buffer = [0_u16; 512];
            let length = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), 512) };
            let text = String::from_utf16_lossy(&buffer[..length.max(0) as usize]);
            if unsafe { IsWindowVisible(window) } != 0 && text.contains(search.0) {
                search.1 = window;
                return 0;
            }
            1
        }
        let mut search = Search(title, std::ptr::null_mut());
        unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM) };
        (!search.1.is_null()).then_some(search.1)
    }

    /// Brings `title`'s window to the front, waiting up to `wait` for it.
    fn focus(title: &str, wait: Duration) -> HWND {
        let started = std::time::Instant::now();
        loop {
            if let Some(window) = window_titled(title) {
                // An Alt tap lets a background process take the foreground.
                unsafe {
                    keybd_event(VK_MENU as u8, 0, 0, 0);
                    keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
                    SetForegroundWindow(window);
                }
                std::thread::sleep(Duration::from_millis(1_500));
                return window;
            }
            assert!(started.elapsed() < wait, "no window titled {title}");
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Replaces the focused element's text, as a person's edit would.
    fn set_focused_text(text: &str) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).unwrap();
            let element = automation.GetFocusedElement().unwrap();
            let pattern = element
                .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                .unwrap();
            pattern.SetValue(&windows::core::BSTR::from(text)).unwrap();
        }
    }

    /// Reads the field, edits it, reads it again, and judges the fix.
    fn learns_in_focused_field() {
        let mut first = None;
        for _ in 0..10 {
            first = super::focused_field().filter(|field| field.text.contains(DICTATED));
            if first.is_some() {
                break;
            }
            println!("focused field so far: {:?}", super::focused_field());
            std::thread::sleep(Duration::from_secs(1));
        }
        let first = first.expect("the focused field holds the dictation");
        println!("first read: {first:?}");
        set_focused_text(FIXED);
        std::thread::sleep(Duration::from_millis(500));
        let now = super::focused_field().expect("the field reads after the edit");
        println!("after the edit: {now:?}");
        assert_eq!(now.identity, first.identity, "the same element");
        assert_eq!(
            crate::dictation::fix_learning::fixes_between(&first, DICTATED, &now.text),
            [("hexy um".to_string(), "Hexium".to_string())]
        );
    }

    #[test]
    #[ignore]
    fn notepad() {
        let path = std::env::temp_dir().join("hexium-live.txt");
        std::fs::write(&path, DICTATED).unwrap();
        let mut notepad = std::process::Command::new("notepad.exe")
            .arg(&path)
            .spawn()
            .unwrap();
        focus("hexium-live", Duration::from_secs(20));
        println!("window: {:?}", super::window_text());
        learns_in_focused_field();
        let _ = notepad.kill();
    }

    #[test]
    #[ignore]
    fn edge() {
        let edge = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";
        let profile = std::env::temp_dir().join("hexium-live-edge");
        let page = format!(
            "data:text/html,<title>Hexium live</title><h1>Indian-language features nobody else has</h1><textarea autofocus>{DICTATED}</textarea>"
        );
        let mut browser = std::process::Command::new(edge)
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--new-window")
            .arg(page)
            .spawn()
            .unwrap();
        focus("Hexium live", Duration::from_secs(60));
        let mut window = None;
        for _ in 0..10 {
            window = super::window_text().filter(|text| {
                text.pieces
                    .iter()
                    .any(|piece| piece.contains("Indian-language features nobody else has"))
            });
            if window.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        println!("window: {window:?}");
        assert!(window.is_some(), "the page's heading is read");
        learns_in_focused_field();
        let _ = browser.kill();
    }
}
