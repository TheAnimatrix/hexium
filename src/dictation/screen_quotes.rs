//! Hexium: Labs › Quote what's on screen. A dictation that repeats a short
//! piece of text the focused window shows (a heading, a label, a button, a
//! short message) gets that piece in quotes, exactly as the window writes
//! it; a longer run of words it shares with the window takes the window's
//! spelling; and a word the window writes with inner capitals (GitHub,
//! iPhone) takes that casing. Then Labs › Learn from your fixes' learned
//! spellings apply. No model is involved: the transcript is matched word by
//! word against the window's text (`screen_text`).

use std::collections::HashMap;

use crate::dictation::screen_text::ScreenText;
use crate::dictation::vocabulary::VocabularyEntry;

/// The fewest words a quoted piece has.
const MIN_QUOTED_WORDS: usize = 3;
/// The most words a quoted piece has; longer text is prose, not a label.
const MAX_QUOTED_WORDS: usize = 15;
/// A quoted piece this long needs no unusual word to count.
const PLAIN_QUOTED_WORDS: usize = 5;
/// The fewest words a run must share with the window to take its spelling.
const MIN_SPELLED_WORDS: usize = 4;
/// Most window places one transcript word is tried at.
const MAX_SEEDS_PER_WORD: usize = 64;

/// The Hexium step after a mode's corrections: learned spellings, then the
/// window's quotes and spellings, each only while its lab is on.
pub(crate) fn finish(transcript: String, mode_vocabulary: &[VocabularyEntry]) -> String {
    let transcript = if crate::labs::learn_fixes() {
        crate::dictation::fix_learning::spell(&transcript, mode_vocabulary)
    } else {
        transcript
    };
    if !crate::labs::quote_screen() || transcript.trim().is_empty() {
        return transcript;
    }
    match crate::dictation::screen_text::take() {
        Some(screen) => apply(&transcript, &screen),
        None => transcript,
    }
}

/// One word of a text: where it is, and its form for comparing.
#[derive(Clone, Debug)]
pub(crate) struct Word {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) key: String,
}

/// Whether `character` is part of a word: letters, digits, and the marks
/// Indic scripts write vowels and viramas with.
pub(crate) fn word_char(character: char) -> bool {
    character.is_alphanumeric()
        || matches!(character as u32, 0x0300..=0x036F | 0x0900..=0x0DFF | 0x200C | 0x200D)
}

/// The words of `text`, an apostrophe between letters kept inside one.
pub(crate) fn words(text: &str) -> Vec<Word> {
    let mut words = Vec::new();
    let mut start = None;
    let mut chars = text.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        let inner_apostrophe = matches!(character, '\'' | '’')
            && start.is_some()
            && chars.peek().is_some_and(|(_, next)| word_char(*next));
        if word_char(character) || inner_apostrophe {
            start.get_or_insert(index);
        } else if let Some(begin) = start.take() {
            words.push(word(text, begin, index));
        }
    }
    if let Some(begin) = start {
        words.push(word(text, begin, text.len()));
    }
    words
}

fn word(text: &str, start: usize, end: usize) -> Word {
    Word {
        start,
        end,
        key: text[start..end]
            .chars()
            .filter(|character| !matches!(character, '\'' | '’'))
            .flat_map(char::to_lowercase)
            .collect(),
    }
}

/// Edit distance between two strings, in chars.
pub(crate) fn distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut row: Vec<usize> = (0..=right.len()).collect();
    for (index, left_char) in left.chars().enumerate() {
        let mut previous = row[0];
        row[0] = index + 1;
        for (column, right_char) in right.iter().enumerate() {
            let substituted = previous + usize::from(left_char != *right_char);
            previous = row[column + 1];
            row[column + 1] = substituted.min(previous + 1).min(row[column] + 1);
        }
    }
    row[right.len()]
}

/// Whether two word keys may be the same word, allowing a transcription's
/// small misspelling in a longer one; digits must agree. A run only keeps an
/// inexact pair when the window's word is a name ([`named`]).
fn same_word(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let digits = |key: &str| key.chars().filter(char::is_ascii_digit).collect::<String>();
    if digits(left) != digits(right) {
        return false;
    }
    let shorter = left.chars().count().min(right.chars().count());
    let allowed = match shorter {
        0..=3 => 0,
        4..=7 => 1,
        _ => 2,
    };
    allowed > 0 && distance(left, right) <= allowed
}

/// Short words too common to anchor a match on.
fn common(key: &str) -> bool {
    const COMMON: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "but", "by", "do", "for", "from", "has", "have",
        "he", "her", "his", "i", "if", "in", "is", "it", "its", "me", "my", "no", "not", "of",
        "on", "or", "our", "she", "so", "that", "the", "their", "them", "then", "there", "they",
        "this", "to", "up", "us", "was", "we", "were", "what", "when", "which", "who", "will",
        "with", "you", "your",
    ];
    key.chars().count() < 2 || COMMON.contains(&key)
}

struct Piece<'a> {
    text: &'a str,
    words: Vec<Word>,
}

/// A run of transcript words matching a run of one piece's words.
#[derive(Clone, Copy, Debug)]
struct Run {
    said: (usize, usize),
    piece: usize,
    shown: (usize, usize),
    inexact: usize,
}

impl Run {
    fn len(&self) -> usize {
        self.said.1 - self.said.0
    }
}

/// Quotes, spells, and cases `transcript` from what `screen` shows.
pub(crate) fn apply(transcript: &str, screen: &ScreenText) -> String {
    let said = words(transcript);
    if said.is_empty() {
        return transcript.to_string();
    }
    let pieces: Vec<Piece> = screen
        .pieces
        .iter()
        .map(|text| Piece {
            text,
            words: words(text),
        })
        .collect();
    let mut index: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for (piece_index, piece) in pieces.iter().enumerate() {
        for (word_index, word) in piece.words.iter().enumerate() {
            if !common(&word.key) {
                let seeds = index.entry(word.key.as_str()).or_default();
                if seeds.len() < MAX_SEEDS_PER_WORD {
                    seeds.push((piece_index, word_index));
                }
            }
        }
    }
    let mut runs = Vec::new();
    for (said_index, word) in said.iter().enumerate() {
        for &(piece_index, shown_index) in index.get(word.key.as_str()).into_iter().flatten() {
            runs.push(extend(
                &said,
                &pieces[piece_index],
                piece_index,
                said_index,
                shown_index,
            ));
        }
    }
    // Longest first, then the fewest inexact words; overlapping runs lose.
    runs.sort_by(|left, right| {
        right
            .len()
            .cmp(&left.len())
            .then(left.inexact.cmp(&right.inexact))
            .then(left.said.0.cmp(&right.said.0))
    });
    let mut chosen: Vec<(Run, bool)> = Vec::new();
    for run in runs {
        if chosen
            .iter()
            .any(|(other, _)| run.said.0 < other.said.1 && other.said.0 < run.said.1)
        {
            continue;
        }
        let piece = &pieces[run.piece];
        // A near miss stands only for a name the model misspelled, never an
        // ordinary word it heard right (decrease, not the window's increase).
        if !inexact_words_are_names(&said, piece, run) {
            continue;
        }
        let close = run.inexact * 4 <= run.len();
        let whole = run.shown == (0, piece.words.len());
        let quoted = whole
            && close
            && (MIN_QUOTED_WORDS..=MAX_QUOTED_WORDS).contains(&run.len())
            && run.len() < said.len()
            && (run.len() >= PLAIN_QUOTED_WORDS || unusual(piece, run.shown))
            && content_words(piece, run.shown) >= 2;
        let spelled =
            close && run.len() >= MIN_SPELLED_WORDS && content_words(piece, run.shown) >= 2;
        if quoted || spelled {
            chosen.push((run, quoted));
        }
    }
    chosen.sort_by_key(|(run, _)| run.said.0);
    let cased = inner_capitals(&pieces);
    let mut output = String::with_capacity(transcript.len() + 16);
    let mut cursor = 0;
    let mut said_index = 0;
    for (run, quoted) in &chosen {
        while said_index < run.said.0 {
            recase(
                transcript,
                &said[said_index],
                &cased,
                &mut output,
                &mut cursor,
            );
            said_index += 1;
        }
        let first = &said[run.said.0];
        let last = &said[run.said.1 - 1];
        output.push_str(&transcript[cursor..first.start]);
        let piece = &pieces[run.piece];
        if *quoted {
            let shown =
                &piece.text[piece.words[run.shown.0].start..piece.words[run.shown.1 - 1].end];
            let already = transcript[..first.start].ends_with(['"', '“', '\''])
                && transcript[last.end..].starts_with(['"', '”', '\'']);
            if already {
                output.push_str(shown);
            } else {
                output.push('"');
                output.push_str(shown);
                output.push('"');
            }
        } else {
            spell_run(transcript, &said, piece, *run, &mut output);
        }
        cursor = last.end;
        said_index = run.said.1;
    }
    while said_index < said.len() {
        recase(
            transcript,
            &said[said_index],
            &cased,
            &mut output,
            &mut cursor,
        );
        said_index += 1;
    }
    output.push_str(&transcript[cursor..]);
    output
}

/// The longest run through `said[said_index]` and the piece's
/// `shown_index` word, extended both ways while the words agree.
fn extend(
    said: &[Word],
    piece: &Piece,
    piece_index: usize,
    said_index: usize,
    shown_index: usize,
) -> Run {
    let mut inexact = 0;
    let (mut said_start, mut shown_start) = (said_index, shown_index);
    while said_start > 0 && shown_start > 0 {
        let (left, right) = (&said[said_start - 1].key, &piece.words[shown_start - 1].key);
        if !same_word(left, right) {
            break;
        }
        inexact += usize::from(left != right);
        said_start -= 1;
        shown_start -= 1;
    }
    let (mut said_end, mut shown_end) = (said_index + 1, shown_index + 1);
    while said_end < said.len() && shown_end < piece.words.len() {
        let (left, right) = (&said[said_end].key, &piece.words[shown_end].key);
        if !same_word(left, right) {
            break;
        }
        inexact += usize::from(left != right);
        said_end += 1;
        shown_end += 1;
    }
    Run {
        said: (said_start, said_end),
        piece: piece_index,
        shown: (shown_start, shown_end),
        inexact,
    }
}

fn inexact_words_are_names(said: &[Word], piece: &Piece, run: Run) -> bool {
    (0..run.len()).all(|offset| {
        let shown = &piece.words[run.shown.0 + offset];
        said[run.said.0 + offset].key == shown.key || named(piece, run.shown.0 + offset)
    })
}

/// Whether the piece's word at `index` is written as a name: a capital
/// past its first letter (GitHub), or a capital where no sentence starts.
fn named(piece: &Piece, index: usize) -> bool {
    let word = &piece.words[index];
    let text = &piece.text[word.start..word.end];
    if text.chars().skip(1).any(char::is_uppercase) {
        return true;
    }
    let sentence_start = index == 0
        || piece.text[..word.start]
            .trim_end()
            .ends_with(['.', '!', '?', ':']);
    !sentence_start && text.chars().next().is_some_and(char::is_uppercase)
}

/// Words of the run that aren't [`common`].
fn content_words(piece: &Piece, (start, end): (usize, usize)) -> usize {
    piece.words[start..end]
        .iter()
        .filter(|word| !common(&word.key))
        .count()
}

/// Whether the run holds a word a phrase rarely has by chance: a capital
/// after its first word, a digit, or eight letters or more.
fn unusual(piece: &Piece, (start, end): (usize, usize)) -> bool {
    piece.words[start..end]
        .iter()
        .enumerate()
        .any(|(offset, word)| {
            let text = &piece.text[word.start..word.end];
            (start + offset > 0 && text.chars().next().is_some_and(char::is_uppercase))
                || text.chars().any(|character| character.is_ascii_digit())
                || text.chars().count() >= 8
        })
}

/// A word whose letters past the first hold a capital (GitHub, iPhone,
/// macOS), keyed by its lowercase form: how the window writes it.
fn inner_capitals<'a>(pieces: &'a [Piece]) -> HashMap<&'a str, &'a str> {
    let mut cased = HashMap::new();
    for piece in pieces {
        for word in &piece.words {
            let text = &piece.text[word.start..word.end];
            if word.key.chars().count() >= 4
                && text.chars().skip(1).any(char::is_uppercase)
                && text.chars().any(char::is_lowercase)
            {
                cased.entry(word.key.as_str()).or_insert(text);
            }
        }
    }
    cased
}

fn recase(
    transcript: &str,
    word: &Word,
    cased: &HashMap<&str, &str>,
    output: &mut String,
    cursor: &mut usize,
) {
    if let Some(shown) = cased.get(word.key.as_str())
        && transcript[word.start..word.end].chars().count() == shown.chars().count()
    {
        output.push_str(&transcript[*cursor..word.start]);
        output.push_str(shown);
        *cursor = word.end;
    }
}

/// Writes the run's words as the window spells them, keeping the
/// transcript's text between words. A word capitalized only because it
/// starts the window's sentence keeps the transcript's first letter.
fn spell_run(transcript: &str, said: &[Word], piece: &Piece, run: Run, output: &mut String) {
    for offset in 0..run.len() {
        let said_word = &said[run.said.0 + offset];
        let shown_word = &piece.words[run.shown.0 + offset];
        if offset > 0 {
            output.push_str(&transcript[said[run.said.0 + offset - 1].end..said_word.start]);
        }
        let heard = &transcript[said_word.start..said_word.end];
        let shown = &piece.text[shown_word.start..shown_word.end];
        let starts_sentence = run.shown.0 + offset == 0
            || piece.text[..shown_word.start]
                .trim_end()
                .ends_with(['.', '!', '?', ':']);
        let only_initial_capital = !shown.chars().skip(1).any(char::is_uppercase);
        if starts_sentence && only_initial_capital {
            output.push_str(&with_initial_of(shown, heard));
        } else {
            output.push_str(shown);
        }
    }
}

/// `shown` with `heard`'s first-letter case.
fn with_initial_of(shown: &str, heard: &str) -> String {
    let mut chars = shown.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let upper = heard.chars().next().is_some_and(char::is_uppercase);
    let first: String = if upper {
        first.to_uppercase().collect()
    } else {
        first.to_lowercase().collect()
    };
    first + chars.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(pieces: &[&str]) -> ScreenText {
        ScreenText {
            pieces: pieces.iter().map(|piece| piece.to_string()).collect(),
        }
    }

    #[test]
    fn a_heading_said_inside_a_sentence_is_quoted_as_the_window_writes_it() {
        let shown = screen(&[
            "Here's what I'd build:",
            "Indian-language features nobody else has.",
            "This is where Hexium can stand out on its own.",
        ]);
        assert_eq!(
            apply(
                "I like the idea of indian language features nobody else has, let's do it.",
                &shown
            ),
            "I like the idea of \"Indian-language features nobody else has\", let's do it."
        );
    }

    #[test]
    fn a_misspelled_name_still_matches() {
        let shown = screen(&["Ship the Hexium release notes"]);
        assert_eq!(
            apply("we should ship the hexiam release notes today", &shown),
            "we should \"Ship the Hexium release notes\" today"
        );
    }

    #[test]
    fn a_near_miss_on_an_ordinary_word_or_a_digit_changes_nothing() {
        let shown = screen(&[
            "We should increase the timeout tomorrow",
            "The temporary password is hunter2 for now",
        ]);
        for said in [
            "I think we should decrease the timeout tomorrow morning",
            "it says the temporary password is hunter3 for now, ok",
        ] {
            assert_eq!(apply(said, &shown), said);
        }
    }

    #[test]
    fn a_short_common_phrase_from_the_window_is_left_alone() {
        let shown = screen(&["Sounds good to me", "See you then"]);
        let said = "sounds good to me, see you at five";
        assert_eq!(apply(said, &shown), said);
    }

    #[test]
    fn the_whole_dictation_matching_a_piece_is_not_quoted() {
        let shown = screen(&["Indian-language features nobody else has"]);
        assert_eq!(
            apply("Indian language features nobody else has.", &shown),
            "Indian language features nobody else has."
        );
    }

    #[test]
    fn existing_quotes_are_not_doubled() {
        let shown = screen(&["Quote what's on screen for Hexium"]);
        assert_eq!(
            apply(
                "turn on \"quote what's on screen for hexium\" please",
                &shown
            ),
            "turn on \"Quote what's on screen for Hexium\" please"
        );
    }

    #[test]
    fn a_longer_shared_run_takes_the_windows_spelling_without_quotes() {
        let shown = screen(&[
            "The patch queue rebases every Hexium change onto upstream each Monday morning.",
        ]);
        assert_eq!(
            apply(
                "so the patch queue rebases every hexiam change and that's it",
                &shown
            ),
            "so the patch queue rebases every Hexium change and that's it"
        );
    }

    #[test]
    fn a_word_the_window_writes_with_inner_capitals_takes_that_casing() {
        let shown = screen(&["Open a pull request on GitHub", "Sync from iPhone"]);
        assert_eq!(
            apply("push it to github and check the iphone build", &shown),
            "push it to GitHub and check the iPhone build"
        );
    }

    #[test]
    fn telugu_text_on_screen_is_quoted_too() {
        let shown = screen(&["తెలుగు భాషా లక్షణాలు ఎవరికీ లేవు"]);
        assert_eq!(
            apply("ఈ విభాగం తెలుగు భాషా లక్షణాలు ఎవరికీ లేవు అని చెబుతుంది", &shown),
            "ఈ విభాగం \"తెలుగు భాషా లక్షణాలు ఎవరికీ లేవు\" అని చెబుతుంది"
        );
    }

    #[test]
    fn nothing_on_screen_changes_nothing() {
        assert_eq!(apply("hello there", &ScreenText::default()), "hello there");
        assert_eq!(apply("", &screen(&["anything at all here"])), "");
    }

    #[test]
    fn words_keep_inner_apostrophes_and_indic_marks() {
        let text = "Don't stop — తెలుగు's fine";
        let keys: Vec<_> = words(text).into_iter().map(|word| word.key).collect();
        assert_eq!(keys, ["dont", "stop", "తెలుగుs", "fine"]);
    }
}
