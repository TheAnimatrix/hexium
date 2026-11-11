//! Hexium: Labs › Remove repeated words. A word or a short phrase said
//! again straight after itself ("about to about to about to find out", "I
//! I think", "the- the plan") is written once, with no model involved. The
//! last copy stays, since it's the one the sentence carries on from, and it
//! takes the first copy's capital and any opening quote or bracket.
//!
//! Repeats that mean something stay:
//! - a word doubled for emphasis or as a sound ("no no no", "very very",
//!   "bye bye", "knock knock") is never shortened;
//! - a single word said exactly twice is shortened only in English, and not
//!   when English grammar doubles it ("I had had enough", "what it is is",
//!   "that that") or both copies are capitalised mid-sentence (a name such
//!   as "Duran Duran"): other languages double words to change their
//!   meaning (Hindi "dheere dheere", "slowly");
//! - nothing is shortened across the end of a sentence, or when the words
//!   hold no letters (numbers).

/// The longest phrase, in words, whose repeats are shortened.
const MAX_PHRASE: usize = 4;

/// Words doubled on purpose, never shortened however often they come.
const EMPHATIC: &[&str] = &[
    "no", "yes", "yeah", "yep", "nope", "very", "really", "so", "bye", "ha", "haha", "hah", "hey",
    "oh", "ooh", "please", "knock", "now", "go", "come", "well", "wait", "okay", "ok", "hi", "la",
    "na", "blah", "bla", "tick", "tock", "ding", "dong", "boo", "woo", "yay", "hip", "more", "far",
    "many", "much", "tsk", "tut", "chop", "wow", "hmm", "mm", "rah", "cha", "choo", "yada",
    "again", "hush", "there", "pretty", "night", "tom",
];

/// Words English grammar can say twice in a row ("I had had enough").
const DOUBLES: &[&str] = &["that", "had", "is", "do", "does", "did", "was"];

/// One whitespace-separated word of the transcript.
struct Token {
    /// Its bytes in the transcript, edge punctuation included.
    start: usize,
    /// Where its letters begin, after an opening quote or bracket.
    core_start: usize,
    /// Its letters, lowercase, without edge punctuation.
    key: String,
    ends_sentence: bool,
    capitalised: bool,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, character) in text.char_indices().chain([(text.len(), ' ')]) {
        match (start, character.is_whitespace()) {
            (None, false) => start = Some(at),
            (Some(from), true) => {
                found.push(token(text, from, at));
                start = None;
            }
            _ => {}
        }
    }
    found
}

fn token(text: &str, start: usize, end: usize) -> Token {
    let raw = &text[start..end];
    let core = raw.trim_matches(|character: char| !character.is_alphanumeric());
    let core_start = start + raw.find(core).unwrap_or(0);
    Token {
        start,
        core_start,
        key: core.to_lowercase(),
        ends_sentence: raw
            .trim_end_matches(['"', '\'', ')', ']', '”', '’'])
            .ends_with(['.', '!', '?']),
        capitalised: core.chars().next().is_some_and(char::is_uppercase),
    }
}

/// Whether a dictation in `language` loses single words said twice. Other
/// languages keep them; longer repeats go in every language.
fn doubles_go(language: &str) -> bool {
    language == "en"
}

/// `transcript` with its repeats shortened when the Labs switch is
/// `enabled`; `None` leaves the caller's text untouched.
pub fn remove_if(transcript: &str, language: &str, enabled: bool) -> Option<String> {
    enabled.then(|| apply(transcript, language))
}

/// `transcript` with each run of repeats written once.
pub fn apply(transcript: &str, language: &str) -> String {
    let words = tokens(transcript);
    let mut out = String::with_capacity(transcript.len());
    let mut copied = 0;
    let mut index = 0;
    while index < words.len() {
        let Some((phrase, copies)) = repeat_at(&words, index, language) else {
            index += 1;
            continue;
        };
        let first = &words[index];
        let kept = &words[index + (copies - 1) * phrase];
        // Up to the first copy, then its opening quote or bracket.
        out.push_str(&transcript[copied..first.core_start]);
        // The first copy's opening quote is written; the kept one's isn't
        // written again.
        let rest = if first.core_start > first.start {
            &transcript[kept.core_start..]
        } else {
            &transcript[kept.start..]
        };
        // The kept copy takes the first one's capital.
        let mut chars = rest.chars();
        match chars.next() {
            Some(initial) if first.capitalised && initial.is_lowercase() => {
                out.extend(initial.to_uppercase());
                copied = transcript.len() - chars.as_str().len();
            }
            _ => copied = transcript.len() - rest.len(),
        }
        index += copies * phrase;
    }
    out.push_str(&transcript[copied..]);
    out
}

/// The shortest phrase starting at `index` that is said again straight
/// after itself, and how many times in a row, when it should be shortened.
fn repeat_at(words: &[Token], index: usize, language: &str) -> Option<(usize, usize)> {
    for phrase in 1..=MAX_PHRASE {
        let copies = copies(words, index, phrase);
        if copies >= 2 && shortened(words, index, phrase, copies, language) {
            return Some((phrase, copies));
        }
    }
    None
}

/// How many times the `phrase` words at `index` come in a row; a copy that
/// ends a sentence ends the run after it.
fn copies(words: &[Token], index: usize, phrase: usize) -> usize {
    let first = &words[index..];
    if first.len() < phrase
        || first[..phrase]
            .iter()
            .any(|word| !word.key.chars().any(char::is_alphabetic))
    {
        return 0;
    }
    let mut copies = 1;
    loop {
        let previous = &first[(copies - 1) * phrase..copies * phrase];
        if previous.iter().any(|word| word.ends_sentence) {
            return copies;
        }
        let Some(next) = first.get(copies * phrase..(copies + 1) * phrase) else {
            return copies;
        };
        if next
            .iter()
            .zip(&first[..phrase])
            .any(|(a, b)| a.key != b.key)
        {
            return copies;
        }
        copies += 1;
    }
}

fn shortened(words: &[Token], index: usize, phrase: usize, copies: usize, language: &str) -> bool {
    if phrase > 1 {
        return true;
    }
    let word = &words[index];
    if EMPHATIC.contains(&word.key.as_str()) {
        return false;
    }
    if copies > 2 {
        return true;
    }
    let second = &words[index + 1];
    let starts_sentence = index == 0 || words[index - 1].ends_sentence;
    let name = word.capitalised && second.capitalised && !starts_sentence && word.key != "i";
    doubles_go(language) && !DOUBLES.contains(&word.key.as_str()) && !name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en(text: &str) -> String {
        apply(text, "en")
    }

    #[test]
    fn a_repeated_phrase_is_said_once() {
        assert_eq!(
            en("What I'm about to about to about to find out"),
            "What I'm about to find out"
        );
        assert_eq!(
            en("We should, we should, we should ship it."),
            "We should ship it."
        );
    }

    #[test]
    fn a_stuttered_word_is_said_once() {
        assert_eq!(en("I I think so"), "I think so");
        assert_eq!(en("I- I- I think so"), "I think so");
        assert_eq!(en("go to the the store"), "go to the store");
        assert_eq!(en("The the plan works."), "The plan works.");
    }

    #[test]
    fn the_first_copy_keeps_its_capital_and_quote() {
        assert_eq!(en("It it works"), "It works");
        assert_eq!(en("She said \"we we can\""), "She said \"we can\"");
    }

    #[test]
    fn meaningful_repeats_stay() {
        for kept in [
            "no no no",
            "very very good",
            "bye bye",
            "knock knock",
            "I had had enough",
            "what it is is fine",
            "I knew that that was wrong",
            "we saw Duran Duran live",
            "call 911 911",
        ] {
            assert_eq!(en(kept), kept);
        }
    }

    #[test]
    fn nothing_goes_across_a_sentence() {
        assert_eq!(
            en("I went home. Home was quiet."),
            "I went home. Home was quiet."
        );
        assert_eq!(en("Stop. Stop. Stop."), "Stop. Stop. Stop.");
    }

    #[test]
    fn other_languages_keep_a_word_said_twice() {
        assert_eq!(apply("dheere dheere chalo", "hi"), "dheere dheere chalo");
        assert_eq!(apply("dheere dheere chalo", "auto"), "dheere dheere chalo");
        assert_eq!(apply("main main main jaunga", "hi"), "main jaunga");
        assert_eq!(
            apply("kal kal ke baad kal ke baad", "hi"),
            "kal kal ke baad"
        );
    }

    #[test]
    fn line_breaks_and_spacing_are_kept() {
        assert_eq!(en("first line\nwe we go"), "first line\nwe go");
        assert_eq!(en("  leading  the the  end "), "  leading  the  end ");
    }

    #[test]
    fn off_leaves_the_text() {
        assert_eq!(remove_if("the the", "en", false), None);
        assert_eq!(remove_if("the the", "en", true).as_deref(), Some("the"));
    }
}
