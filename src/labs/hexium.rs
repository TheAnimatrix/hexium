//! Hexium's own Labs switches, kept in their own word: upstream's
//! [`super::STATE`] uses every one of its 64 bits, so sharing it would make
//! one of Hexium's switches turn an upstream lab on (or off) with it.

use std::sync::atomic::{AtomicU64, Ordering};

use super::{Feature, LabsSettings};
use crate::app_settings::AppSettings;

/// Set once this process knows its settings.
const KNOWN: u64 = 1;
pub(super) const LEARN_FIXES: u64 = 1 << 1;
pub(super) const QUOTE_SCREEN: u64 = 1 << 2;

static STATE: AtomicU64 = AtomicU64::new(0);

pub(super) fn on(bit: u64) -> bool {
    state() & bit != 0
}

fn state() -> u64 {
    match STATE.load(Ordering::Acquire) {
        0 => {
            let saved = encode(&AppSettings::saved_labs());
            let _ = STATE.compare_exchange(0, saved, Ordering::AcqRel, Ordering::Acquire);
            STATE.load(Ordering::Acquire)
        }
        state => state,
    }
}

/// Records the switches the loaded or saved settings hold.
pub(super) fn apply(settings: &LabsSettings) {
    STATE.store(encode(settings), Ordering::Release);
}

fn encode(settings: &LabsSettings) -> u64 {
    let policy = crate::policy::Policy::current();
    encode_where(settings, |feature| {
        feature.shipped() && policy.labs_lock(feature).is_none()
    })
}

/// The switches `settings` hold for the features `reaches` admits.
pub(super) fn encode_where(settings: &LabsSettings, reaches: impl Fn(Feature) -> bool) -> u64 {
    [
        (Feature::LearnFixes, settings.learn_fixes, LEARN_FIXES),
        (Feature::QuoteScreen, settings.quote_screen, QUOTE_SCREEN),
    ]
    .into_iter()
    .filter(|(feature, on, _)| *on && reaches(*feature))
    .fold(KNOWN, |state, (_, _, bit)| state | bit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_switch_is_its_own_bit_and_off_by_default() {
        let mut labs = LabsSettings::default();
        assert_eq!(encode_where(&labs, |_| true), KNOWN);
        labs.learn_fixes = true;
        assert_eq!(encode_where(&labs, |_| true), KNOWN | LEARN_FIXES);
        labs.quote_screen = true;
        assert_eq!(
            encode_where(&labs, |_| true),
            KNOWN | LEARN_FIXES | QUOTE_SCREEN
        );
    }

    #[test]
    fn upstream_switches_stay_off() {
        let labs = LabsSettings {
            learn_fixes: true,
            quote_screen: true,
            ..LabsSettings::default()
        };
        assert_eq!(
            super::super::encode_where(&labs, |_| true) & !super::super::KNOWN,
            {
                let defaults = LabsSettings::default();
                super::super::encode_where(&defaults, |_| true) & !super::super::KNOWN
            }
        );
    }
}
