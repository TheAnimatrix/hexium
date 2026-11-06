//! Hexium: what the focused window shows and what the focused text field
//! holds, read through each OS's accessibility interface (macOS
//! Accessibility, Windows UI Automation, Linux AT-SPI). Labs › Quote what's
//! on screen matches a dictation against the window's text; Labs › Learn
//! from your fixes watches the field a dictation was pasted into.
//!
//! Every read is bounded in elements, characters, and time, and runs off the
//! thread that asked: a slow or hung app costs at most the deadline. Nothing
//! read here is stored or sent anywhere; it lives for one dictation.

use std::sync::Mutex;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Most elements one window read visits.
pub(crate) const MAX_NODES: usize = 2_500;
/// Most characters one window read keeps.
pub(crate) const MAX_CHARS: usize = 60_000;
/// Most characters one piece keeps; past this it's prose, not a label.
const MAX_PIECE_CHARS: usize = 2_000;
/// Deepest element one window read enters.
pub(crate) const MAX_DEPTH: usize = 64;
/// How long one window read may take before it stops with what it has.
pub(crate) const WINDOW_BUDGET: Duration = Duration::from_millis(450);
/// The longest field text a fix is looked for in.
pub(crate) const MAX_FIELD_CHARS: usize = 200_000;
/// How long a dictation's processing waits for the window read it started.
const TAKE_WAIT: Duration = Duration::from_millis(500);
/// A prefetched read older than this belongs to another dictation.
const FRESH: Duration = Duration::from_secs(20);

/// The text of the focused window: each element's own text, in tree order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ScreenText {
    pub(crate) pieces: Vec<String>,
}

/// The focused text field: its whole text and, when the app says, where the
/// insertion point is (a char index).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FieldText {
    pub(crate) text: String,
    pub(crate) caret: Option<usize>,
    /// Which element this is (app, window, role, and the element's own
    /// names), so a watcher notices focus moving to another field.
    pub(crate) identity: String,
}

/// One accessibility element, as [`walk`] reads it.
pub(crate) trait TextNode: Sized {
    /// The text this element shows itself (not its children's), if any.
    fn text(&self) -> Option<String>;
    fn children(&self) -> Vec<Self>;
}

/// Collects the text of `root`'s tree, depth first, within the bounds.
pub(crate) fn walk<N: TextNode>(root: N, started: Instant) -> ScreenText {
    let mut collector = Collector::default();
    let mut stack = vec![(root, 0_usize)];
    let mut visited = 0;
    while let Some((node, depth)) = stack.pop() {
        visited += 1;
        if visited > MAX_NODES || collector.full() || started.elapsed() > WINDOW_BUDGET {
            break;
        }
        if let Some(text) = node.text() {
            collector.push(&text);
        }
        if depth < MAX_DEPTH {
            let children = node.children();
            stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
        }
    }
    collector.finish()
}

/// Gathers pieces: one per line of an element's text, trimmed, without
/// repeats, up to [`MAX_CHARS`].
#[derive(Default)]
pub(crate) struct Collector {
    pieces: Vec<String>,
    chars: usize,
    seen: std::collections::HashSet<String>,
}

impl Collector {
    pub(crate) fn push(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || !line.chars().any(char::is_alphanumeric) || self.full() {
                continue;
            }
            // Cut to what the budget has left before anything is copied.
            let keep = MAX_PIECE_CHARS.min(MAX_CHARS - self.chars);
            let line = match line.char_indices().nth(keep) {
                Some((end, _)) => &line[..end],
                None => line,
            };
            if !self.seen.contains(line) {
                self.seen.insert(line.to_string());
                self.chars += line.chars().count();
                self.pieces.push(line.to_string());
            }
        }
    }

    pub(crate) fn full(&self) -> bool {
        self.chars >= MAX_CHARS
    }

    pub(crate) fn finish(self) -> ScreenText {
        ScreenText {
            pieces: self.pieces,
        }
    }
}

/// The focused window's text, read now on a helper thread and waited for up
/// to `wait`.
pub(crate) fn window_text(wait: Duration) -> Option<ScreenText> {
    let receiver = spawn_read("hexium-screen-text", platform::window_text)?;
    receiver.recv_timeout(wait).ok().flatten()
}

/// The focused text field, read now on a helper thread and waited for up
/// to `wait`.
pub(crate) fn focused_field(wait: Duration) -> Option<FieldText> {
    let receiver = spawn_read("hexium-focused-field", platform::focused_field)?;
    receiver.recv_timeout(wait).ok().flatten()
}

/// One read of each kind at a time: a hung app blocks its helper thread
/// until it answers, and starting more would pile them up.
fn spawn_read<T: Send + 'static>(
    name: &'static str,
    read: fn() -> Option<T>,
) -> Option<mpsc::Receiver<Option<T>>> {
    static IN_FLIGHT: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    {
        let mut in_flight = IN_FLIGHT.lock().unwrap_or_else(|error| error.into_inner());
        if in_flight.contains(&name) {
            tracing::debug!(
                name,
                "skipping a screen read; the previous one is still blocked"
            );
            return None;
        }
        in_flight.push(name);
    }
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let value = read();
            IN_FLIGHT
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .retain(|known| *known != name);
            let _ = sender.send(value);
        });
    match spawned {
        Ok(_) => Some(receiver),
        Err(_) => {
            IN_FLIGHT
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .retain(|known| *known != name);
            None
        }
    }
}

struct Prefetch {
    started: Instant,
    receiver: mpsc::Receiver<Option<ScreenText>>,
}

static PREFETCH: Mutex<Option<Prefetch>> = Mutex::new(None);

/// Starts reading the focused window as a dictation ends, so the read
/// overlaps transcription; [`take`] collects it. Only while Labs › Quote
/// what's on screen is on.
pub(crate) fn prefetch() {
    if !crate::labs::quote_screen() {
        return;
    }
    let Some(receiver) = spawn_read("hexium-screen-text", platform::window_text) else {
        return;
    };
    *PREFETCH.lock().unwrap_or_else(|error| error.into_inner()) = Some(Prefetch {
        started: Instant::now(),
        receiver,
    });
}

/// The window text for the dictation being processed: the prefetched read
/// when one is fresh, else a read now.
pub(crate) fn take() -> Option<ScreenText> {
    let prefetched = PREFETCH
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take()
        .filter(|prefetch| prefetch.started.elapsed() < FRESH);
    match prefetched {
        Some(prefetch) => prefetch.receiver.recv_timeout(TAKE_WAIT).ok().flatten(),
        None => window_text(TAKE_WAIT),
    }
}

#[cfg(target_os = "macos")]
mod platform {
    pub(super) fn window_text() -> Option<super::ScreenText> {
        crate::accessibility::screen_text::window_text(crate::dictation::context::frontmost_pid()?)
    }

    pub(super) fn focused_field() -> Option<super::FieldText> {
        crate::accessibility::screen_text::focused_field(crate::dictation::context::frontmost_pid()?)
    }
}

#[cfg(target_os = "windows")]
mod platform {
    pub(super) fn window_text() -> Option<super::ScreenText> {
        crate::windows::screen_text::window_text()
    }

    pub(super) fn focused_field() -> Option<super::FieldText> {
        crate::windows::screen_text::focused_field()
    }
}

#[cfg(target_os = "linux")]
mod platform {
    pub(super) fn window_text() -> Option<super::ScreenText> {
        crate::linux::atspi::screen_text::window_text()
    }

    pub(super) fn focused_field() -> Option<super::FieldText> {
        crate::linux::atspi::screen_text::focused_field()
    }
}

/// The char index of a UTF-16 offset into `text` (macOS and Windows count
/// text in UTF-16 units).
pub(crate) fn char_index_of_utf16(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (index, character) in text.chars().enumerate() {
        if units >= utf16 {
            return index;
        }
        units += character.len_utf16();
    }
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Node {
        text: Option<&'static str>,
        children: Vec<Node>,
    }

    impl TextNode for &Node {
        fn text(&self) -> Option<String> {
            self.text.map(str::to_string)
        }

        fn children(&self) -> Vec<Self> {
            self.children.iter().collect()
        }
    }

    fn node(text: Option<&'static str>, children: Vec<Node>) -> Node {
        Node { text, children }
    }

    #[test]
    fn the_walk_reads_text_in_tree_order_one_piece_per_line_without_repeats() {
        let tree = node(
            Some("Window"),
            vec![
                node(None, vec![node(Some("Heading one"), vec![])]),
                node(Some("first line\n  second line  \n\n"), vec![]),
                node(Some("Heading one"), vec![]),
                node(Some("  ---  "), vec![]),
            ],
        );
        let text = walk(&tree, Instant::now());
        assert_eq!(
            text.pieces,
            ["Window", "Heading one", "first line", "second line"]
        );
    }

    #[test]
    fn the_walk_stops_at_its_element_budget() {
        let children = (0..MAX_NODES + 50)
            .map(|_| node(Some("x1"), vec![]))
            .collect();
        let mut tree = node(None, children);
        // Distinct texts so repeats don't hide the bound.
        let texts: Vec<&'static str> = (0..MAX_NODES + 50)
            .map(|index| &*Box::leak(format!("piece {index}").into_boxed_str()))
            .collect();
        for (child, text) in tree.children.iter_mut().zip(texts) {
            child.text = Some(text);
        }
        let text = walk(&tree, Instant::now());
        assert!(text.pieces.len() < MAX_NODES);
    }

    #[test]
    fn one_long_line_is_cut_to_the_budget() {
        let mut collector = Collector::default();
        collector.push(&"తెలుగు ".repeat(10_000));
        let text = collector.finish();
        assert_eq!(text.pieces.len(), 1);
        assert_eq!(text.pieces[0].chars().count(), MAX_PIECE_CHARS);
    }

    #[test]
    fn utf16_offsets_become_char_indexes() {
        assert_eq!(char_index_of_utf16("abc", 2), 2);
        assert_eq!(char_index_of_utf16("a😀b", 3), 2);
        assert_eq!(char_index_of_utf16("తెలుగు", 2), 2);
        assert_eq!(char_index_of_utf16("ab", 9), 2);
    }
}
