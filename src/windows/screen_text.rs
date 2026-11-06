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
                tracing::debug!(%error, "UI Automation could not read the screen text");
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
