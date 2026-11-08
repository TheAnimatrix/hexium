//! Hexium: Labs › Learn from your fixes. After a dictation is pasted, the
//! field it went into is read again once you stop editing it (a few seconds
//! without a change, another dictation, or focus moving on). A word or two
//! you changed that looks like a mishearing ("hexy um" → "Hexium", "github"
//! → "GitHub") goes into Vocabulary with what was heard as its "sounds
//! like"; rewrites, added sentences, and common-word swaps are left alone.
//! Later dictations then write the learned spelling: Parakeet, Whisper, and
//! Qwen3-ASR are steered toward it, and [`spell`] corrects every model's
//! transcript (Indic-Transcribe-Flex can't be steered).
//!
//! Reading the field goes through each OS's accessibility interface
//! (`screen_text`); an app that doesn't expose its text learns nothing.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::dictation::screen_quotes::{distance, word_char, words};
use crate::dictation::screen_text::{self, FieldText};
use crate::dictation::text_replacements::{ReplacementSet, TextReplacement};
use crate::dictation::vocabulary::VocabularyEntry;

/// How long after a paste the field is first read, so the paste has landed.
const FIRST_LOOK: Duration = Duration::from_millis(400);
/// How often the field is read while you may be editing it.
const POLL: Duration = Duration::from_millis(750);
/// How long the field stays unchanged after an edit before it's judged.
const SETTLED: Duration = Duration::from_secs(4);
/// The longest a dictation's field is watched.
const WATCH_LIMIT: Duration = Duration::from_secs(90);
/// How long one field read may take. An app asked for the first time
/// (Chromium, Electron) builds its accessibility tree first, which can take
/// over a second.
const READ_WAIT: Duration = Duration::from_millis(1_500);
/// How many times the field is looked for after a paste before giving up.
const FIRST_LOOKS: usize = 5;
/// Missed reads in a row that end a watch.
const MAX_MISSES: usize = 3;
/// Characters around the dictation that find it again after edits.
const ANCHOR_CHARS: usize = 40;
/// The most words on either side of one learned fix.
const MAX_FIX_WORDS: usize = 3;
/// The most fixes one dictation can teach; more changes are a rewrite.
const MAX_FIXES: usize = 3;
/// The longest dictation, in words, that is watched.
const MAX_WORDS: usize = 400;
/// A weaker fix (one capitalized word for another) is learned once it has
/// been made this many times.
const WEAK_FIX_REPEATS: u32 = 2;

/// Bumped by every paste, so a watcher knows a newer dictation started.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// A fix: what the transcript said, and what you changed it to.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Fix {
    pub(crate) heard: String,
    pub(crate) written: String,
}

/// How sure a fix is a mishearing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Confidence {
    /// Learned at once: words merged or split, a capital inside a word, a
    /// digit or symbol, or a script other than Latin.
    Strong,
    /// Learned once it's made again.
    Weak,
}

/// Called after a dictation is pasted into the focused app.
pub(crate) fn pasted(text: &str) {
    let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    if !crate::labs::learn_fixes() {
        return;
    }
    let pasted = text.trim().to_string();
    if pasted.is_empty() || words(&pasted).len() > MAX_WORDS {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("hexium-learn-fixes".into())
        .spawn(move || watch(generation, &pasted));
    if let Err(error) = spawned {
        tracing::info!(%error, "could not watch the dictation for fixes");
    }
}

/// The focused field holding the dictation, looked for a few times: the
/// paste may still be landing, and an app asked for the first time may not
/// have its accessibility tree yet.
fn find_dictation(generation: u64, pasted: &str) -> Option<(FieldText, Range<usize>)> {
    let mut exposed = false;
    for look in 0..FIRST_LOOKS {
        std::thread::sleep(if look == 0 { FIRST_LOOK } else { POLL });
        if GENERATION.load(Ordering::Acquire) != generation {
            return None;
        }
        if let Some(field) = screen_text::focused_field(READ_WAIT) {
            exposed = true;
            if let Some(span) = locate(&field, pasted) {
                return Some((field, span));
            }
        }
    }
    if exposed {
        tracing::info!(
            "learn from fixes: the dictation isn't in the focused field; nothing to watch"
        );
    } else {
        tracing::info!(
            "learn from fixes: the focused app doesn't expose its text; nothing to watch"
        );
    }
    None
}

fn watch(generation: u64, pasted: &str) {
    let Some((first, span)) = find_dictation(generation, pasted) else {
        return;
    };
    tracing::info!("learn from fixes: watching the field the dictation went into");
    let started = Instant::now();
    let mut last = first.text.clone();
    let mut changed_at: Option<Instant> = None;
    let mut misses = 0;
    loop {
        std::thread::sleep(POLL);
        if GENERATION.load(Ordering::Acquire) != generation || started.elapsed() > WATCH_LIMIT {
            break;
        }
        // Turning the lab off ends the watch, with nothing learned.
        if !crate::labs::learn_fixes() {
            return;
        }
        // One slow read doesn't end the watch; several in a row do.
        let Some(now) = screen_text::focused_field(READ_WAIT) else {
            misses += 1;
            if misses >= MAX_MISSES {
                break;
            }
            continue;
        };
        misses = 0;
        // Another element, even one holding the same words, isn't this one.
        if now.identity != first.identity {
            break;
        }
        if now.text == last {
            if changed_at.is_some_and(|at| at.elapsed() >= SETTLED) {
                break;
            }
            continue;
        }
        if !same_field(&first.text, &span, &now.text) {
            break;
        }
        last = now.text;
        changed_at = Some(Instant::now());
    }
    if last == first.text {
        tracing::info!("learn from fixes: the dictation wasn't changed");
        return;
    }
    let found = fixes(&first.text, span, &last);
    if found.is_empty() {
        tracing::info!("learn from fixes: the changes looked like edits, not misheard words");
    }
    for (fix, confidence) in found {
        if !crate::labs::learn_fixes() {
            return;
        }
        if confidence == Confidence::Strong || repeated(&fix) {
            learn(&fix);
        } else {
            tracing::info!("a fix was seen once; it's learned if you make it again");
        }
    }
}

/// Whether a weak fix has now been made [`WEAK_FIX_REPEATS`] times.
fn repeated(fix: &Fix) -> bool {
    static SEEN: Mutex<Option<HashMap<Fix, u32>>> = Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(|error| error.into_inner());
    let seen = seen.get_or_insert_with(HashMap::new);
    if seen.len() > 1_000 {
        seen.clear();
    }
    let count = seen.entry(fix.clone()).or_default();
    *count += 1;
    *count >= WEAK_FIX_REPEATS
}

/// Adds a fix to Vocabulary through the running Hexium, which saves it and
/// shows it in Settings › Vocabulary like any other entry.
fn learn(fix: &Fix) {
    let sounds_like = sounds_like(fix);
    let request =
        crate::control::ControlRequest::new(crate::control::ControlCommand::VocabularyAdd {
            term: fix.written.clone(),
            sounds_like,
        });
    match crate::local_api::call_control(&request) {
        Ok(_) => tracing::info!("learned a spelling from a fix"),
        Err(error) => tracing::warn!(%error, "could not add a learned spelling to Vocabulary"),
    }
}

/// What was heard, as the learned term's "sounds like": none when only the
/// case differs (the term itself matches then), else the heard form, which
/// a spacing fix ("data base") needs for later transcripts to match.
fn sounds_like(fix: &Fix) -> Vec<String> {
    if fix.heard.to_lowercase() == fix.written.to_lowercase() {
        Vec::new()
    } else {
        vec![fix.heard.clone()]
    }
}

/// The byte range `pasted` occupies in the field: the occurrence ending
/// nearest the insertion point, else the last. A continued dictation may
/// have had its first letter's case changed as it was joined.
fn locate(field: &FieldText, pasted: &str) -> Option<Range<usize>> {
    // Each match's own range: a recased first letter can change its length.
    let mut candidates: Vec<Range<usize>> = field
        .text
        .match_indices(pasted)
        .map(|(start, matched)| start..start + matched.len())
        .collect();
    if candidates.is_empty() {
        let flipped = flip_initial(pasted);
        candidates = field
            .text
            .match_indices(&flipped)
            .map(|(start, matched)| start..start + matched.len())
            .collect();
    }
    let caret = field.caret.map(|caret| {
        field
            .text
            .char_indices()
            .nth(caret)
            .map_or(field.text.len(), |(byte, _)| byte)
    });
    match caret {
        Some(caret) => candidates
            .into_iter()
            .min_by_key(|range| range.end.abs_diff(caret)),
        None => candidates.pop(),
    }
}

fn flip_initial(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let flipped: String = if first.is_uppercase() {
        first.to_lowercase().collect()
    } else {
        first.to_uppercase().collect()
    };
    flipped + chars.as_str()
}

/// Whether `now` is still the field the dictation went into: it keeps the
/// text just before or after the dictation, or most of its words.
fn same_field(first: &str, span: &Range<usize>, now: &str) -> bool {
    let before = tail(&first[..span.start], ANCHOR_CHARS);
    let after = head(&first[span.end..], ANCHOR_CHARS);
    if !before.trim().is_empty() {
        return now.contains(before);
    }
    if !after.trim().is_empty() {
        return now.contains(after);
    }
    let dictated: Vec<String> = words(&first[span.clone()])
        .into_iter()
        .map(|word| word.key)
        .collect();
    let present = words(now)
        .into_iter()
        .map(|word| word.key)
        .collect::<std::collections::HashSet<_>>();
    let kept = dictated.iter().filter(|key| present.contains(*key)).count();
    kept * 2 >= dictated.len()
}

fn tail(text: &str, chars: usize) -> &str {
    let start = text
        .char_indices()
        .rev()
        .nth(chars.saturating_sub(1))
        .map_or(0, |(byte, _)| byte);
    &text[start..]
}

fn head(text: &str, chars: usize) -> &str {
    let end = text
        .char_indices()
        .nth(chars)
        .map_or(text.len(), |(byte, _)| byte);
    &text[..end]
}

/// The fixes made to the dictation at `span` of `old`, given the field now
/// reads `new`.
fn fixes(old: &str, span: Range<usize>, new: &str) -> Vec<(Fix, Confidence)> {
    let before = &old[..span.start];
    let after = &old[span.end..];
    let start = if new.starts_with(before) {
        before.len()
    } else {
        let anchor = tail(before, ANCHOR_CHARS);
        match new
            .match_indices(anchor)
            .min_by_key(|(at, _)| at.abs_diff(span.start))
        {
            Some((at, _)) => at + anchor.len(),
            None => return Vec::new(),
        }
    };
    let end = if after.is_empty() {
        new.len()
    } else if new.ends_with(after) && new.len() - after.len() >= start {
        new.len() - after.len()
    } else {
        let anchor = head(after, ANCHOR_CHARS);
        new[start..].find(anchor).map_or(new.len(), |at| start + at)
    };
    let starts_sentence =
        before.trim_end().is_empty() || before.trim_end().ends_with(['.', '!', '?', '\n']);
    judge(&old[span], &new[start..end], starts_sentence)
}

/// One whitespace-separated token: its text without edge punctuation, and
/// whether a sentence ends right after it.
struct Token<'a> {
    core: &'a str,
    ends_sentence: bool,
}

fn tokens(text: &str) -> Vec<Token<'_>> {
    text.split_whitespace()
        .filter_map(|raw| {
            let core = raw.trim_matches(|character: char| !word_char(character));
            (!core.is_empty()).then(|| Token {
                core,
                ends_sentence: raw.ends_with(['.', '!', '?']),
            })
        })
        .collect()
}

/// The fixes between the dictation as pasted (`old`) and as it reads now
/// (`new`, which may run on into text typed after it).
fn judge(old: &str, new: &str, starts_sentence: bool) -> Vec<(Fix, Confidence)> {
    let old = tokens(old);
    let mut new = tokens(new);
    if old.is_empty() || old.len() > MAX_WORDS {
        return Vec::new();
    }
    new.truncate(old.len() * 2 + 20);
    let hunks = differences(&old, &new);
    let changed: usize = hunks.iter().map(|hunk| hunk.0.len()).sum();
    let edits = hunks.iter().filter(|hunk| !hunk.0.is_empty()).count();
    if changed * 2 > old.len() || edits > MAX_FIXES {
        return Vec::new();
    }
    let mut fixes = Vec::new();
    for (old_range, mut new_range) in hunks {
        if old_range.is_empty() || new_range.is_empty() {
            continue;
        }
        // Text typed on after the dictation's last word joins its last
        // change; keep only the words that best match what was heard.
        if old_range.end == old.len()
            && new_range.end == new.len()
            && new_range.len() > old_range.len()
        {
            let heard = joined(&old[old_range.clone()]);
            let best = (1..=MAX_FIX_WORDS.min(new_range.len()))
                .min_by_key(|count| {
                    distance(
                        &squashed(&heard),
                        &squashed(&joined(&new[new_range.start..new_range.start + count])),
                    )
                })
                .unwrap_or(1);
            new_range.end = new_range.start + best;
        }
        let at_start = if old_range.start == 0 {
            starts_sentence
        } else {
            old[old_range.start - 1].ends_sentence
        };
        let fix = Fix {
            heard: joined(&old[old_range]),
            written: joined(&new[new_range]),
        };
        if let Some(confidence) = confidence(&fix, at_start) {
            fixes.push((fix, confidence));
        }
    }
    fixes
}

fn joined(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|token| token.core)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The changed runs between `old` and `new`, as (old range, new range), from
/// a longest-common-subsequence alignment of their words.
fn differences(old: &[Token], new: &[Token]) -> Vec<(Range<usize>, Range<usize>)> {
    let (rows, columns) = (old.len(), new.len());
    // common[i][j]: the longest common run of old[i..] and new[j..].
    let mut common = vec![vec![0_u16; columns + 1]; rows + 1];
    for i in (0..rows).rev() {
        for j in (0..columns).rev() {
            common[i][j] = if old[i].core == new[j].core {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let mut hunks = Vec::new();
    let (mut i, mut j) = (0, 0);
    let (mut hunk_i, mut hunk_j) = (0, 0);
    while i < rows || j < columns {
        if i < rows && j < columns && old[i].core == new[j].core {
            if hunk_i < i || hunk_j < j {
                hunks.push((hunk_i..i, hunk_j..j));
            }
            i += 1;
            j += 1;
            (hunk_i, hunk_j) = (i, j);
        } else if j == columns || (i < rows && common[i + 1][j] >= common[i][j + 1]) {
            i += 1;
        } else {
            j += 1;
        }
    }
    if hunk_i < rows || hunk_j < columns {
        hunks.push((hunk_i..rows, hunk_j..columns));
    }
    hunks
}

/// Letters and digits only, lowercase: how a fix's two sides compare.
fn squashed(text: &str) -> String {
    text.chars()
        .filter(|character| word_char(*character))
        .flat_map(char::to_lowercase)
        .collect()
}

/// How sure `fix` is a mishearing worth learning, or `None` when it looks
/// like an edit: too many words, no letters, a sentence's first capital, a
/// spelling too far from what was heard, or one lowercase word for another.
fn confidence(fix: &Fix, at_start: bool) -> Option<Confidence> {
    let heard_words = fix.heard.split_whitespace().count();
    let written_words = fix.written.split_whitespace().count();
    if !(1..=MAX_FIX_WORDS).contains(&heard_words)
        || !(1..=MAX_FIX_WORDS).contains(&written_words)
        || !fix.written.chars().any(char::is_alphabetic)
        || fix.heard == fix.written
    {
        return None;
    }
    let (heard, written) = (squashed(&fix.heard), squashed(&fix.written));
    if heard.is_empty() || written.is_empty() {
        return None;
    }
    let inner_capital = fix
        .written
        .split_whitespace()
        .any(|word| word.chars().skip(1).any(char::is_uppercase));
    let symbol = fix
        .written
        .chars()
        .any(|character| character.is_ascii_digit() || matches!(character, '_' | '.' | '-' | '/'));
    // A script other than Latin (accented Latin letters don't count): its
    // words have no capitals to tell a name, so a fix there must be repeated.
    let other_script = fix
        .written
        .chars()
        .any(|character| character.is_alphabetic() && u32::from(character) > 0x024F);
    let capitalized = !at_start && fix.written.chars().next().is_some_and(char::is_uppercase);
    let strong = inner_capital || symbol || heard_words != written_words;
    let weak = capitalized || other_script;
    if heard == written {
        // Only spacing or case changed: worth it for a name's casing, not a
        // sentence's first letter.
        return if strong {
            Some(Confidence::Strong)
        } else if weak {
            Some(Confidence::Weak)
        } else {
            None
        };
    }
    // Spellings within half of each other for a strong fix, two fifths for
    // a weak one (Friday → Monday is an edit, Harry → Hari a mishearing).
    let longest = heard.chars().count().max(written.chars().count());
    let changed = distance(&heard, &written);
    if (strong && changed * 2 > longest) || (!strong && changed * 5 > longest * 2) {
        return None;
    }
    if strong {
        Some(Confidence::Strong)
    } else if weak {
        Some(Confidence::Weak)
    } else {
        None
    }
}

/// Learned spellings: each Vocabulary entry's "sounds like" spellings are
/// written as its term, and a term with capitals keeps them, on every model.
/// Runs while Labs › Learn from your fixes is on.
pub(crate) fn spell(transcript: &str, mode_vocabulary: &[VocabularyEntry]) -> String {
    static RULES: Mutex<Option<(Vec<VocabularyEntry>, ReplacementSet)>> = Mutex::new(None);
    let entries = crate::dictation::vocabulary::merged(
        mode_vocabulary,
        &crate::dictation::vocabulary::entries(),
    );
    if entries.is_empty() {
        return transcript.to_string();
    }
    let mut rules = RULES.lock().unwrap_or_else(|error| error.into_inner());
    if rules.as_ref().is_none_or(|(known, _)| *known != entries) {
        let replacements: Vec<TextReplacement> = entries
            .iter()
            .flat_map(|entry| {
                let cased = entry
                    .term
                    .chars()
                    .any(char::is_uppercase)
                    .then(|| entry.term.clone());
                entry
                    .sounds_like
                    .iter()
                    .cloned()
                    .chain(cased)
                    .map(|matched_phrase| TextReplacement {
                        matched_phrase,
                        output: entry.term.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        *rules = Some((entries, ReplacementSet::new(&replacements)));
    }
    rules.as_ref().map_or_else(
        || transcript.to_string(),
        |(_, set)| set.replace(transcript),
    )
}

/// The fixes a watch would find between a field read before and after an
/// edit, as (heard, written): for live checks against a real app.
#[cfg(all(test, target_os = "windows"))]
pub(crate) fn fixes_between(first: &FieldText, pasted: &str, now: &str) -> Vec<(String, String)> {
    locate(first, pasted)
        .map(|span| fixes(&first.text, span, now))
        .unwrap_or_default()
        .into_iter()
        .map(|(fix, _)| (fix.heard, fix.written))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(heard: &str, written: &str) -> Fix {
        Fix {
            heard: heard.into(),
            written: written.into(),
        }
    }

    /// The fixes found once the field reads `new_field`; an empty
    /// `old_field` is a field that held only the dictation.
    fn learned(old_field: &str, pasted: &str, new_field: &str) -> Vec<(Fix, Confidence)> {
        let old_field = if old_field.is_empty() {
            pasted
        } else {
            old_field
        };
        let field = FieldText {
            text: old_field.into(),
            caret: None,
            ..FieldText::default()
        };
        let span = locate(&field, pasted).expect("the dictation is in the field");
        fixes(old_field, span, new_field)
    }

    #[test]
    fn a_misheard_name_fixed_in_place_is_learned() {
        assert_eq!(
            learned(
                "Notes: I pushed the hexy um release today.",
                "I pushed the hexy um release today.",
                "Notes: I pushed the Hexium release today.",
            ),
            [(fix("hexy um", "Hexium"), Confidence::Strong)]
        );
    }

    #[test]
    fn a_names_casing_is_learned() {
        assert_eq!(
            learned("", "open it on github please", "open it on GitHub please"),
            [(fix("github", "GitHub"), Confidence::Strong)]
        );
    }

    #[test]
    fn a_capitalized_name_needs_a_second_fix() {
        assert_eq!(
            learned("", "I met harry at noon", "I met Hari at noon"),
            [(fix("harry", "Hari"), Confidence::Weak)]
        );
    }

    #[test]
    fn telugu_fixes_are_learned() {
        assert_eq!(
            learned("", "నేను హెక్సియం వాడుతున్నాను", "నేను హెక్సియమ్ వాడుతున్నాను"),
            [(fix("హెక్సియం", "హెక్సియమ్"), Confidence::Weak)]
        );
    }

    #[test]
    fn typing_on_after_the_dictation_still_finds_the_fix() {
        assert_eq!(
            learned(
                "",
                "deploy it with cube control",
                "deploy it with kubectl and then check the pods"
            ),
            [(fix("cube control", "kubectl"), Confidence::Strong)]
        );
    }

    #[test]
    fn edits_that_arent_mishearings_are_ignored() {
        // A common-word swap.
        assert!(learned("", "put it over their now", "put it over there now").is_empty());
        // An accented Latin word for another (peach → spade).
        assert!(learned("", "Je veux une pêche demain", "Je veux une bêche demain").is_empty());
        // A sentence's first letter.
        assert!(learned("", "send it today", "Send it today").is_empty());
        // A different word altogether.
        assert!(learned("", "we ship on friday morning", "we ship on Monday morning").is_empty());
        // Punctuation only.
        assert!(learned("", "ok see you then", "OK, see you then.").len() <= 1);
        // A rewrite.
        assert!(learned("", "let's meet on the call", "no meeting needed after all").is_empty());
        // Added words with nothing replaced.
        assert!(learned("", "ship it", "ship it now please").is_empty());
    }

    #[test]
    fn a_continued_dictation_with_its_first_letter_recased_is_found() {
        let field = FieldText {
            text: "First part. second part here".into(),
            caret: Some(28),
            ..FieldText::default()
        };
        assert_eq!(locate(&field, "Second part here"), Some(12..28));
    }

    #[test]
    fn a_recased_match_of_another_length_is_sliced_at_its_own_end() {
        let field = FieldText {
            text: "i\u{307}é and more".into(),
            caret: None,
            ..FieldText::default()
        };
        let span = locate(&field, "İé").expect("found recased");
        assert_eq!(&field.text[span.clone()], "i\u{307}é");
        assert!(same_field(&field.text, &span, "i\u{307}é and more!"));
    }

    #[test]
    fn a_spacing_fix_keeps_what_was_heard_and_a_case_fix_does_not() {
        assert_eq!(sounds_like(&fix("data base", "database")), ["data base"]);
        assert!(sounds_like(&fix("github", "GitHub")).is_empty());
        let vocabulary = [VocabularyEntry {
            term: "database".into(),
            sounds_like: sounds_like(&fix("data base", "database")),
        }];
        assert_eq!(
            spell("open the data base", &vocabulary),
            "open the database"
        );
    }

    #[test]
    fn the_occurrence_nearest_the_caret_is_the_dictation() {
        let field = FieldText {
            text: "ok then ok then".into(),
            caret: Some(7),
            ..FieldText::default()
        };
        assert_eq!(locate(&field, "ok then"), Some(0..7));
    }

    #[test]
    fn another_field_is_not_the_same_field() {
        let old = "Dear Sam, the hexy um build is out.";
        let span = 10..old.len();
        assert!(same_field(old, &span, "Dear Sam, the Hexium build is out."));
        assert!(!same_field(old, &span, "Search"));
    }

    #[test]
    fn learned_spellings_correct_later_transcripts() {
        let vocabulary = [
            VocabularyEntry {
                term: "Hexium".into(),
                sounds_like: vec!["hexy um".into()],
            },
            VocabularyEntry {
                term: "GitHub".into(),
                sounds_like: Vec::new(),
            },
        ];
        assert_eq!(
            spell("the hexy um build is on github", &vocabulary),
            "the Hexium build is on GitHub"
        );
    }
}
