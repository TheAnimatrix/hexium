//! Hexium: every translated UI string (tray tooltip and menu, sidebar,
//! settings, notices) names the app "Hexium" instead of "Hex". Applied once in
//! `desktop::i18n::tr`, so upstream's strings and translation tables stay
//! untouched. "Hex Microphone" keeps its name: it is the macOS audio driver's
//! device name.

use std::collections::HashMap;
use std::sync::Mutex;

const KEEP: &[&str] = &["Hex Microphone"];
/// Distinct branded strings kept; past this, text is returned as it came.
const MAX_CACHED: usize = 4096;

/// `text` with the app's name as Hexium.
pub(crate) fn brand(text: &str) -> &str {
    if !crate::app_paths::HEXIUM || !names_hex(text) {
        return text;
    }
    static CACHE: Mutex<Option<HashMap<String, &'static str>>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(branded) = cache.get(text) {
        return branded;
    }
    if cache.len() >= MAX_CACHED {
        return text;
    }
    let branded: &'static str = Box::leak(rename(text).into_boxed_str());
    cache.insert(text.to_owned(), branded);
    branded
}

fn names_hex(text: &str) -> bool {
    hex_words(text).next().is_some()
}

/// Byte offsets of each standalone "Hex" that is not part of a kept name.
fn hex_words(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.match_indices("Hex")
        .map(|(at, _)| at)
        .filter(move |&at| {
            let before = text[..at].chars().next_back();
            let after = text[at + 3..].chars().next();
            !before.is_some_and(char::is_alphanumeric)
                && !after.is_some_and(char::is_alphanumeric)
                && !KEEP.iter().any(|keep| text[at..].starts_with(keep))
        })
}

fn rename(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut last = 0;
    for at in hex_words(text) {
        out.push_str(&text[last..at]);
        out.push_str("Hexium");
        last = at + 3;
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_is_called_hexium() {
        assert_eq!(brand("Quit Hex"), "Quit Hexium");
        assert_eq!(
            brand("Hex Dictation: recording"),
            "Hexium Dictation: recording"
        );
        assert_eq!(brand("Hex's settings.json"), "Hexium's settings.json");
        assert_eq!(brand("Quitter Hex"), "Quitter Hexium");
    }

    #[test]
    fn other_words_and_device_names_are_left_alone() {
        assert_eq!(brand("Download Hexium update"), "Download Hexium update");
        assert_eq!(brand("hex app"), "hex app");
        assert_eq!(brand("Hexagon"), "Hexagon");
        assert_eq!(
            brand("Hex Microphone is the system default, so Hex listens to {}"),
            "Hex Microphone is the system default, so Hexium listens to {}"
        );
    }
}
