//! Hexium: the focused window's text and the focused field's text on macOS,
//! read through Accessibility for `dictation::screen_text`. Every element
//! carries the parent module's messaging timeout, so a hung app costs at
//! most that per call, and the walk stops at its budget.

use std::time::Instant;

use super::{
    AXUIElementCreateApplication, CfObject, bounded_element, copy_element, copy_elements,
    copy_range, copy_string, request_web_accessibility,
};
use crate::dictation::screen_text::{
    FieldText, MAX_FIELD_CHARS, ScreenText, TextNode, char_index_of_utf16, walk,
};

/// The text of `pid`'s focused (else main) window.
pub(crate) fn window_text(pid: i32) -> Option<ScreenText> {
    let started = Instant::now();
    // SAFETY: AXUIElementCreateApplication returns a retained Core Foundation object.
    let application = bounded_element(unsafe { AXUIElementCreateApplication(pid) })?;
    // Chromium builds its web tree only for an assistive app that asks.
    request_web_accessibility(pid, &application);
    let window = copy_element(&application, c"AXFocusedWindow")
        .or_else(|| copy_element(&application, c"AXMainWindow"))?;
    Some(walk(Node(window), started))
}

/// The text of `pid`'s focused text element and its insertion point; `None`
/// for a password field, an element without text, or a very long one.
pub(crate) fn focused_field(pid: i32) -> Option<FieldText> {
    // SAFETY: AXUIElementCreateApplication returns a retained Core Foundation object.
    let application = bounded_element(unsafe { AXUIElementCreateApplication(pid) })?;
    let focused = copy_element(&application, c"AXFocusedUIElement")?;
    let secure = |name| copy_string(&focused, name).as_deref() == Some("AXSecureTextField");
    if secure(c"AXRole") || secure(c"AXSubrole") {
        return None;
    }
    let text = copy_string(&focused, c"AXValue")?;
    if text.len() > MAX_FIELD_CHARS * 4 || text.chars().count() > MAX_FIELD_CHARS {
        return None;
    }
    let caret = copy_range(&focused, c"AXSelectedTextRange")
        .map(|range| char_index_of_utf16(&text, range.location.max(0) as usize));
    let window = copy_element(&focused, c"AXWindow")
        .and_then(|window| copy_string(&window, c"AXTitle"))
        .unwrap_or_default();
    let names = [
        c"AXRole",
        c"AXIdentifier",
        c"AXTitle",
        c"AXDescription",
        c"AXPlaceholderValue",
    ]
    .map(|name| copy_string(&focused, name).unwrap_or_default());
    let identity = format!("{pid}\u{1f}{window}\u{1f}{}", names.join("\u{1f}"));
    Some(FieldText {
        text,
        caret,
        identity,
    })
}

struct Node(CfObject);

impl TextNode for Node {
    fn text(&self) -> Option<String> {
        let role = copy_string(&self.0, c"AXRole")?;
        match role.as_str() {
            "AXStaticText" | "AXTextField" | "AXTextArea" | "AXComboBox" => {
                copy_string(&self.0, c"AXValue")
            }
            "AXHeading" | "AXButton" | "AXLink" | "AXMenuItem" | "AXMenuButton"
            | "AXRadioButton" | "AXCheckBox" | "AXPopUpButton" | "AXWindow" | "AXCell"
            | "AXRow" => copy_string(&self.0, c"AXTitle")
                .filter(|title| !title.trim().is_empty())
                .or_else(|| copy_string(&self.0, c"AXDescription")),
            _ => None,
        }
    }

    fn children(&self) -> Vec<Self> {
        copy_elements(&self.0, c"AXChildren")
            .into_iter()
            .map(Node)
            .collect()
    }
}
