//! Hexium: the active window's text and the focused field's text on Linux,
//! read over AT-SPI for `dictation::screen_text`. GTK, Qt, and Firefox
//! expose their text whenever the accessibility bus is up; Chromium and
//! Electron apps only with `--force-renderer-accessibility` or a running
//! screen reader. Every call is bounded, and the whole read has a deadline.

use std::collections::HashMap;
use std::time::Duration;

use zbus::{Connection, OwnedVariant};

use super::{
    ACCESSIBLE, MATCH_ALL, MatchRule, REGISTRY, ROOT, Reference, SORT_CANONICAL, WindowHint,
    bitset, call, matching_window,
};
use crate::dictation::screen_text::{Collector, FieldText, MAX_FIELD_CHARS, MAX_NODES, ScreenText};

const STATE_FOCUSED: u32 = 12;
const STATE_SHOWING: u32 = 25;
const ROLE_PASSWORD_TEXT: u32 = 40;
const MATCH_NONE: i32 = 3;
const TEXT: &str = "org.a11y.atspi.Text";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// Elements read at once.
const BATCH: usize = 48;
/// How long a whole read may take.
const READ_DEADLINE: Duration = Duration::from_millis(700);

/// The showing text of the active window.
pub(crate) fn window_text() -> Option<ScreenText> {
    block(async {
        let bus = accessibility_bus().await?;
        let window = active_window(&bus).await?;
        let elements: Vec<Reference> = call(
            &bus,
            &window.0,
            window.1.as_str(),
            "org.a11y.atspi.Collection",
            "GetMatches",
            &(rule(STATE_SHOWING), SORT_CANONICAL, MAX_NODES as i32, true),
        )
        .await?;
        let mut collector = Collector::default();
        for batch in elements.chunks(BATCH) {
            let texts = futures_util::future::join_all(
                batch.iter().map(|element| element_text(&bus, element)),
            )
            .await;
            for (name, text) in texts {
                for piece in [name, text].into_iter().flatten() {
                    collector.push(&piece);
                }
            }
            if collector.full() {
                break;
            }
        }
        Some(collector.finish())
    })
    .flatten()
}

/// The focused element of the active window: its text and caret.
pub(crate) fn focused_field() -> Option<FieldText> {
    block(async {
        let bus = accessibility_bus().await?;
        let window = active_window(&bus).await?;
        let focused: Vec<Reference> = call(
            &bus,
            &window.0,
            window.1.as_str(),
            "org.a11y.atspi.Collection",
            "GetMatches",
            &(rule(STATE_FOCUSED), SORT_CANONICAL, 1_i32, true),
        )
        .await?;
        let (name, path) = focused.into_iter().next()?;
        let count: OwnedVariant = call(
            &bus,
            &name,
            path.as_str(),
            PROPERTIES,
            "Get",
            &(TEXT, "CharacterCount"),
        )
        .await?;
        if i32::try_from(&count).ok()? as usize > MAX_FIELD_CHARS {
            return None;
        }
        let text: String = call(
            &bus,
            &name,
            path.as_str(),
            TEXT,
            "GetText",
            &(0_i32, -1_i32),
        )
        .await?;
        let caret = call::<_, OwnedVariant>(
            &bus,
            &name,
            path.as_str(),
            PROPERTIES,
            "Get",
            &(TEXT, "CaretOffset"),
        )
        .await
        .and_then(|offset| i32::try_from(&offset).ok())
        .and_then(|offset| usize::try_from(offset).ok());
        Some(FieldText {
            identity: format!("{name}{}", path.as_str()),
            text,
            caret,
        })
    })
    .flatten()
}

fn block<T>(read: impl std::future::Future<Output = T>) -> Option<T> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async { tokio::time::timeout(READ_DEADLINE, read).await.ok() })
}

async fn accessibility_bus() -> Option<Connection> {
    let session = Connection::session().await.ok()?;
    let address: String = call(
        &session,
        "org.a11y.Bus",
        "/org/a11y/bus",
        "org.a11y.Bus",
        "GetAddress",
        &(),
    )
    .await?;
    zbus::connection::Builder::address(address.as_str())
        .build()
        .await
        .ok()
}

/// The window an application reports active.
async fn active_window(bus: &Connection) -> Option<Reference> {
    let applications: Vec<Reference> =
        call(bus, REGISTRY, ROOT, ACCESSIBLE, "GetChildren", &()).await?;
    let hint = WindowHint::default();
    let candidates = futures_util::future::join_all(
        applications
            .iter()
            .map(|(name, root)| matching_window(bus, name, root, &hint)),
    )
    .await;
    candidates
        .into_iter()
        .flatten()
        .find(|candidate| candidate.active)
        .map(|candidate| candidate.reference)
}

/// Elements in `state`, any role but a password field's.
fn rule(state: u32) -> MatchRule {
    (
        bitset::<2>(state),
        MATCH_ALL,
        HashMap::new(),
        MATCH_ALL,
        bitset::<4>(ROLE_PASSWORD_TEXT),
        MATCH_NONE,
        Vec::new(),
        MATCH_ALL,
        false,
    )
}

/// An element's name and, when it has the Text interface, its text.
async fn element_text(
    bus: &Connection,
    (name, path): &Reference,
) -> (Option<String>, Option<String>) {
    let label = async {
        let value: OwnedVariant = call(
            bus,
            name,
            path.as_str(),
            PROPERTIES,
            "Get",
            &(ACCESSIBLE, "Name"),
        )
        .await?;
        <&str>::try_from(&value).ok().map(str::to_owned)
    };
    let text = call::<_, String>(bus, name, path.as_str(), TEXT, "GetText", &(0_i32, -1_i32));
    futures_util::future::join(label, text).await
}
