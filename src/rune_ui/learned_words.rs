//! Hexium: the toast Labs › Learn from your fixes shows before it adds a
//! word to Vocabulary. It sits in the bottom-right corner of the display
//! with a bar that drains; Undo before it empties and nothing is learned.
//! The pointer on it holds the bar. A newer fix while one shows keeps the
//! older and shows the newer.

use std::sync::mpsc::Sender;
use std::time::Duration;

use rune::{Align, Cx, El, OverlayHandle, OverlaySpec, ScreenEdge, el};
use rune_kit::Toast;

use super::controls::Paint;
use crate::desktop::i18n::{tr, tr_named};
use crate::dictation::fix_learning::Fix;

/// rune-kit's toast is 320 points wide; room below for three fixes.
const CARD: (f32, f32) = (320.0, 150.0);
/// Around the card, for its shadow.
const PAD: f32 = 16.0;
const SIZE: (f32, f32) = (CARD.0 + 2.0 * PAD, CARD.1 + 2.0 * PAD);
/// From the display's edges to the card's.
const INSET: f32 = 16.0;
const MARGIN: f32 = INSET - PAD;
/// How long the bar takes to drain while the pointer isn't on it.
pub(crate) const COUNTDOWN: Duration = Duration::from_secs(6);
/// How long leaving takes, before the overlay hides.
const LEAVE_S: f64 = 0.4;
/// The bar's inset inside the card (rune-kit's toast padding).
const BAR_INSET: f32 = 12.0;

/// Fixes waiting for Undo or the bar to run out.
struct Proposal {
    fixes: Vec<Fix>,
    reply: Sender<bool>,
}

#[derive(Default)]
pub(crate) struct Card {
    proposal: Option<Proposal>,
    /// What the card says, kept while it leaves.
    fixes: Vec<Fix>,
    on: bool,
    hovered: bool,
    /// Seconds of the countdown still to run.
    left: f64,
    /// Animation seconds of the last countdown step; none after a pause.
    stepped: Option<f64>,
    leaving: Option<f64>,
    now: f64,
    overlay: Option<OverlayHandle<Card>>,
}

impl Card {
    fn propose(&mut self, fixes: Vec<Fix>, reply: Sender<bool>, tx: &mut rune::Tx) {
        // The one showing was never undone: learn it.
        self.answer(true);
        self.fixes = fixes.clone();
        self.proposal = Some(Proposal { fixes, reply });
        self.on = true;
        self.hovered = false;
        self.left = COUNTDOWN.as_secs_f64();
        self.stepped = None;
        self.leaving = None;
        if let Some(overlay) = &self.overlay {
            overlay.show();
        }
        tx.animated();
        tx.wake_after(Duration::ZERO);
    }

    /// Answers the fix-learning thread waiting on the proposal.
    fn answer(&mut self, keep: bool) {
        if let Some(proposal) = self.proposal.take() {
            if !keep {
                tracing::info!(
                    fixes = proposal.fixes.len(),
                    "a learned spelling was undone"
                );
            }
            let _ = proposal.reply.send(keep);
        }
    }

    fn close(&mut self, keep: bool, tx: &mut rune::Tx) {
        self.answer(keep);
        if self.on {
            self.on = false;
            self.leaving = Some(self.now);
            tx.animated();
            tx.wake_after(Duration::from_secs_f64(LEAVE_S));
        }
    }

    /// The share of the countdown still to run, 1 to 0.
    fn remaining(&self) -> f32 {
        (self.left / COUNTDOWN.as_secs_f64()).clamp(0.0, 1.0) as f32
    }
}

fn poll(s: &mut Card, now: f64, tx: &mut rune::Tx) -> bool {
    s.now = now;
    if let Some(at) = s.leaving {
        if now - at >= LEAVE_S {
            s.leaving = None;
            s.fixes.clear();
            if let Some(overlay) = &s.overlay {
                overlay.hide();
            }
            return true;
        }
        tx.wake_after(Duration::from_secs_f64(at + LEAVE_S - now));
        return false;
    }
    if !s.on || s.hovered {
        s.stepped = None;
        return false;
    }
    let step = s.stepped.map_or(0.0, |then| (now - then).max(0.0));
    s.stepped = Some(now);
    s.left -= step;
    if s.left <= 0.0 {
        s.left = 0.0;
        s.close(true, tx);
        return true;
    }
    // The bar follows the frame clock while it drains.
    tx.instant();
    tx.wake_after(Duration::ZERO);
    true
}

/// One line per fix: the learned spelling, and what was heard when it
/// isn't only a change of case.
fn describe(fixes: &[Fix]) -> String {
    fixes
        .iter()
        .map(|fix| {
            if fix.heard.to_lowercase() == fix.written.to_lowercase() {
                fix.written.clone()
            } else {
                tr_named(
                    "{term} instead of {heard}",
                    &[("term", &fix.written), ("heard", &fix.heard)],
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn view(s: &Card, cx: &mut Cx<Card>) -> El {
    let p = Paint::new(cx.system_dark(), cx.env().scale);
    let k = p.k;
    let root = el().key("learned-words").size(SIZE.0, SIZE.1);
    if !s.on || s.fixes.is_empty() {
        return root;
    }
    let track = CARD.0 - 2.0 * BAR_INSET;
    let bar = el()
        .key("time-left")
        .role("progressbar")
        .label(tr("Added to Vocabulary"))
        .w(track)
        .h(3.0)
        .radius(1.5)
        .fill(k.c.border_muted)
        .child(
            el().w(track * s.remaining())
                .h(3.0)
                .radius(1.5)
                .fill(k.c.icon_accent),
        );
    let card = Toast::new("toast", tr("Added to Vocabulary"))
        .icon(k, &super::icons::BOOK)
        .description(describe(&s.fixes))
        .action(k, tr("Undo"), false, |s: &mut Card, tx| s.close(false, tx))
        .on_close(|s: &mut Card| s.answer(true))
        .build(k)
        .child(bar);
    let group = el()
        .key("card")
        .pin(None, None, Some(PAD), Some(PAD))
        .hit()
        .on_hover_change(|s: &mut Card, on, tx| {
            s.hovered = on;
            s.stepped = None;
            if !on {
                tx.wake_after(Duration::ZERO);
            }
        })
        .child(card);
    root.child(group)
}

/// Adds the overlay, hidden, and takes Learn from your fixes' proposals.
pub(crate) fn install<S: 'static>(app: &mut rune::App<S>) {
    let spec = OverlaySpec {
        name: "Hexium learned words",
        size: SIZE,
        edge: ScreenEdge::Bottom,
        margin: MARGIN,
        align: Align::End,
        inset: MARGIN,
        interactive: true,
        view,
    }
    .options(rune::OverlayOptions {
        #[cfg(target_os = "macos")]
        display: rune::DisplayTarget::UnderPointer,
        #[cfg(not(target_os = "macos"))]
        display: crate::rune_hud::display(),
        exclude_from_capture: true,
        input: rune::OverlayInput::Regions,
        ..Default::default()
    })
    .animate_reveal()
    .poll(poll)
    .release_gpu_when_hidden();
    let overlay = app.overlay(spec, Card::default());
    let handle = overlay.clone();
    overlay.update(move |s, _| s.overlay = Some(handle));
    crate::dictation::fix_learning::on_proposal(move |fixes, reply| {
        overlay.update(move |s, tx| s.propose(fixes, reply, tx));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, channel};

    fn fix(heard: &str, written: &str) -> Fix {
        Fix {
            heard: heard.into(),
            written: written.into(),
        }
    }

    fn proposed(card: &mut Card) -> Receiver<bool> {
        let (reply, answer) = channel();
        card.propose(
            vec![fix("hexy um", "Hexium")],
            reply,
            &mut rune::Tx::default(),
        );
        answer
    }

    #[test]
    fn the_bar_running_out_learns_it() {
        let mut card = Card::default();
        let answer = proposed(&mut card);
        let mut tx = rune::Tx::default();
        poll(&mut card, 10.0, &mut tx);
        poll(&mut card, 15.9, &mut tx);
        assert!(card.on);
        assert!(answer.try_recv().is_err());
        poll(&mut card, 16.0, &mut tx);
        assert!(!card.on);
        assert_eq!(answer.try_recv(), Ok(true));
    }

    #[test]
    fn undo_learns_nothing() {
        let mut card = Card::default();
        let answer = proposed(&mut card);
        card.close(false, &mut rune::Tx::default());
        assert_eq!(answer.try_recv(), Ok(false));
        assert!(!card.on);
    }

    #[test]
    fn the_pointer_on_it_holds_the_bar() {
        let mut card = Card::default();
        let answer = proposed(&mut card);
        let mut tx = rune::Tx::default();
        poll(&mut card, 0.0, &mut tx);
        poll(&mut card, 3.0, &mut tx);
        card.hovered = true;
        card.stepped = None;
        poll(&mut card, 60.0, &mut tx);
        card.hovered = false;
        card.stepped = None;
        poll(&mut card, 61.0, &mut tx);
        poll(&mut card, 63.9, &mut tx);
        assert!(answer.try_recv().is_err());
        poll(&mut card, 64.1, &mut tx);
        assert_eq!(answer.try_recv(), Ok(true));
    }

    #[test]
    fn a_newer_fix_keeps_the_one_showing() {
        let mut card = Card::default();
        let first = proposed(&mut card);
        let second = proposed(&mut card);
        assert_eq!(first.try_recv(), Ok(true));
        assert!(second.try_recv().is_err());
        assert!(card.on);
    }

    #[test]
    fn a_spelling_says_what_was_heard_unless_only_its_case_changed() {
        assert_eq!(
            describe(&[fix("hexy um", "Hexium"), fix("github", "GitHub")]),
            "Hexium instead of hexy um\nGitHub"
        );
    }
}
