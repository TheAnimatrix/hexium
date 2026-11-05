//! Hexium: spoken language identification, so Auto works with a model that
//! has to be told the language (Indic-Transcribe-Flex). VoxLingua107's
//! ECAPA-TDNN ranks the languages the model knows for each clip, in tens of
//! milliseconds on the CPU, and the model then transcribes in the winner.
//!
//! The identifier is optional, like the speech detector: while its file is
//! missing it downloads in the background, and Auto uses the language it
//! found last (English before the first).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use color_eyre::eyre::Result;
use transcribe_cpp::{Backend, LangIdOptions, LangIdSession, Model, ModelOptions};

use crate::transcription::models as transcription_models;

/// An Indic language this likely, when English ranks first, still wins: the
/// Indic models write English words in English with any Indic language, but
/// told English they write Telugu or Hindi words in English letters.
const INDIC_OVER_ENGLISH: f32 = 0.25;

/// A clip no louder than this (-60 dBFS) is silence.
const SILENT: f32 = 0.001;

/// The language before the identifier first speaks.
const FIRST_GUESS: &str = "en";

#[cfg_attr(test, allow(dead_code))]
enum State {
    Unloaded,
    Loaded(LangIdSession),
    Failed,
}

#[cfg_attr(test, allow(dead_code))]
static STATE: Mutex<State> = Mutex::new(State::Unloaded);
#[cfg_attr(test, allow(dead_code))]
static DOWNLOAD_STARTED: AtomicBool = AtomicBool::new(false);
static LAST: Mutex<Option<String>> = Mutex::new(None);

/// The language `samples` (16 kHz mono) are spoken in, out of `allowed`
/// (Hex's codes, which VoxLingua107 shares for every language it offers
/// here). Without the identifier, or for a clip too short to tell, the
/// language found last.
pub(crate) fn identify(samples: &[f32], allowed: &[&str]) -> String {
    // The silence a model warms on says nothing, and must not become the
    // language found last.
    let heard = samples.iter().any(|sample| sample.abs() > SILENT);
    let found = heard
        .then(|| ranked(samples, allowed))
        .flatten()
        .map(|ranked| choose(&ranked));
    let mut last = LAST.lock().unwrap_or_else(|error| error.into_inner());
    match found {
        Some(language) => {
            *last = Some(language.clone());
            language
        }
        None => last.clone().unwrap_or_else(|| FIRST_GUESS.to_string()),
    }
}

/// The winner of `ranked` (code, probability; best first): the first, except
/// that English gives way to a likely Indic language, since mixed speech
/// ranks English high and only an Indic language keeps both scripts.
fn choose(ranked: &[(String, f32)]) -> String {
    let Some((first, _)) = ranked.first() else {
        return FIRST_GUESS.to_string();
    };
    if first == "en"
        && let Some((indic, _)) = ranked
            .iter()
            .find(|(code, p)| code != "en" && *p >= INDIC_OVER_ENGLISH)
    {
        return indic.clone();
    }
    first.clone()
}

/// The allowed languages ranked for `samples`, best first. A test thread
/// that set [`override_for_test`] gets its ranking instead.
fn ranked(samples: &[f32], allowed: &[&str]) -> Option<Vec<(String, f32)>> {
    #[cfg(test)]
    if let Some(ranked) = OVERRIDE.with(|ranked| ranked.borrow().clone()) {
        return ranked;
    }
    let mut state = lock_state();
    let session = loaded_session(&mut state)?;
    let options = LangIdOptions {
        allowed: Some(allowed.iter().map(|code| code.to_string()).collect()),
    };
    match session.run(samples, &options) {
        Ok(result) => {
            let ranked: Vec<(String, f32)> = result
                .candidates
                .into_iter()
                .map(|candidate| (candidate.code, candidate.p))
                .collect();
            tracing::info!(
                best = ranked.first().map(|(code, _)| code.as_str()),
                p = ranked.first().map(|(_, p)| *p),
                "identified the spoken language"
            );
            Some(ranked)
        }
        Err(transcribe_cpp::Error::InputTooShort(_)) => None,
        Err(error) => {
            tracing::warn!(%error, "language identification failed");
            None
        }
    }
}

/// Starts the identifier's download, if it is missing, for a model that
/// relies on it. Tests only count the request.
pub(crate) fn ensure_installed() {
    #[cfg(test)]
    DOWNLOAD_REQUESTS.with(|requests| requests.set(requests.get() + 1));
    #[cfg(not(test))]
    if !transcription_models::language_identifier_installed() {
        start_download_once();
    }
}

#[cfg_attr(test, allow(dead_code))]
fn lock_state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

/// The loaded session, loading it on first use. A missing identifier starts
/// a background download and stays unloaded, so it loads once the file lands.
#[cfg_attr(test, allow(dead_code))]
fn loaded_session(state: &mut State) -> Option<&mut LangIdSession> {
    if matches!(state, State::Unloaded) {
        if !transcription_models::language_identifier_installed() {
            // Tests never download it.
            #[cfg(not(test))]
            start_download_once();
            return None;
        }
        *state = match transcription_models::language_identifier_path().and_then(|path| load(&path))
        {
            Ok(session) => State::Loaded(session),
            Err(error) => {
                tracing::warn!(%error, "could not load the language identifier");
                State::Failed
            }
        };
    }
    match state {
        State::Loaded(session) => Some(session),
        State::Unloaded | State::Failed => None,
    }
}

#[cfg_attr(test, allow(dead_code))]
fn start_download_once() {
    if DOWNLOAD_STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("hex-language-identifier-download".into())
        .spawn(|| {
            if let Err(error) =
                transcription_models::download_language_identifier(&AtomicBool::new(false))
            {
                tracing::warn!(%error, "could not download the language identifier");
                // The next Auto dictation tries again.
                DOWNLOAD_STARTED.store(false, Ordering::Relaxed);
            }
        });
}

/// Loads the identifier on the CPU, where it loads and runs in tens of
/// milliseconds, as the speech detector does.
#[cfg_attr(test, allow(dead_code))]
fn load(path: &Path) -> Result<LangIdSession> {
    #[cfg(target_os = "windows")]
    crate::transcription::local_transcriber::init_backends()?;
    let model = Model::load_with(
        path,
        &ModelOptions {
            backend: Backend::Cpu,
            ..Default::default()
        },
    )?;
    Ok(model.langid_session()?)
}

#[cfg(test)]
thread_local! {
    static OVERRIDE: std::cell::RefCell<Option<Option<Vec<(String, f32)>>>> =
        const { std::cell::RefCell::new(None) };
    static DOWNLOAD_REQUESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Makes the identifier rank `ranked` on this thread (`None`: unavailable).
#[cfg(test)]
pub(crate) fn override_for_test(ranked: Option<&[(&str, f32)]>) {
    OVERRIDE.with(|slot| {
        *slot.borrow_mut() = Some(ranked.map(|ranked| {
            ranked
                .iter()
                .map(|(code, p)| (code.to_string(), *p))
                .collect()
        }))
    });
}

#[cfg(test)]
pub(crate) fn download_requests_for_test() -> usize {
    DOWNLOAD_REQUESTS.with(std::cell::Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranked(pairs: &[(&str, f32)]) -> Vec<(String, f32)> {
        pairs
            .iter()
            .map(|(code, p)| (code.to_string(), *p))
            .collect()
    }

    #[test]
    fn the_most_likely_language_wins() {
        assert_eq!(
            choose(&ranked(&[("te", 0.65), ("mr", 0.11), ("ur", 0.08)])),
            "te"
        );
        assert_eq!(choose(&ranked(&[("hi", 0.99), ("ur", 0.01)])), "hi");
        assert_eq!(choose(&ranked(&[("en", 1.0), ("ur", 0.0)])), "en");
    }

    #[test]
    fn a_likely_indic_language_beats_english_so_mixed_speech_keeps_its_script() {
        assert_eq!(
            choose(&ranked(&[("en", 0.6), ("te", 0.35), ("kn", 0.05)])),
            "te"
        );
        assert_eq!(
            choose(&ranked(&[("en", 0.8), ("hi", 0.15), ("ur", 0.05)])),
            "en"
        );
        assert_eq!(choose(&[]), FIRST_GUESS);
    }

    #[test]
    fn without_an_answer_the_last_language_stays() {
        override_for_test(Some(&[("te", 0.9), ("kn", 0.1)]));
        assert_eq!(identify(&[0.1; 16_000], &["te", "kn", "en"]), "te");
        override_for_test(None);
        assert_eq!(identify(&[0.1; 160], &["te", "kn", "en"]), "te");
    }

    /// Indic-Transcribe-Flex on Auto with the real identifier, through the
    /// session dictation and meetings use. `HEXIUM_FLEX_GGUF` is the Flex
    /// GGUF, `HEXIUM_MODELS_DIR` holds the identifier, and `HEXIUM_CLIPS`
    /// has `te.wav`, `te-mixed.wav`, `hi.wav`, `hi-mixed.wav`, `ta.wav`,
    /// `kn.wav`, `bn.wav` and `en.wav` (16 kHz mono).
    #[test]
    #[ignore = "needs the Flex GGUF, the language identifier and clips"]
    fn flex_on_auto_writes_each_clip_in_its_own_script() {
        use crate::transcription::models::{TranscriptionModelId, definition};
        let var = |name: &str| std::path::PathBuf::from(std::env::var(name).unwrap());
        crate::transcription::models::set_test_models_dir(Some(var("HEXIUM_MODELS_DIR")));
        let languages = definition(TranscriptionModelId::IndicTranscribeFlex)
            .identified_languages(crate::transcription::models::AUTO_LANGUAGE);
        assert!(languages.is_some());
        let model = transcribe_cpp::Model::load(var("HEXIUM_FLEX_GGUF")).unwrap();
        let mut session =
            crate::transcription::gguf_session::OfflineGgufSession::new(model).unwrap();
        session.identify_language_among(languages);
        // The script each clip's own words are written in.
        let scripts: [(&str, std::ops::RangeInclusive<char>); 8] = [
            ("te", '\u{0C00}'..='\u{0C7F}'),
            ("te-mixed", '\u{0C00}'..='\u{0C7F}'),
            ("hi", '\u{0900}'..='\u{097F}'),
            ("hi-mixed", '\u{0900}'..='\u{097F}'),
            ("ta", '\u{0B80}'..='\u{0BFF}'),
            ("kn", '\u{0C80}'..='\u{0CFF}'),
            ("bn", '\u{0980}'..='\u{09FF}'),
            ("en", 'a'..='z'),
        ];
        for (clip, script) in scripts {
            let mut reader =
                hound::WavReader::open(var("HEXIUM_CLIPS").join(format!("{clip}.wav"))).unwrap();
            let samples: Vec<f32> = reader
                .samples::<i16>()
                .map(|sample| f32::from(sample.unwrap()) / 32768.0)
                .collect();
            let text = session
                .run(&samples, &transcribe_cpp::RunOptions::default())
                .unwrap()
                .text;
            eprintln!("{clip}: {text}");
            let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
            let own = letters
                .iter()
                .filter(|c| script.contains(&c.to_ascii_lowercase()))
                .count();
            assert!(
                own * 2 > letters.len(),
                "{clip} is not mostly in its script: {text}"
            );
            if clip.ends_with("-mixed") {
                assert!(
                    letters.iter().any(char::is_ascii_alphabetic),
                    "{clip} lost its English words: {text}"
                );
            }
        }
    }

    #[test]
    fn silence_is_not_identified_and_keeps_the_last_language() {
        override_for_test(Some(&[("hi", 0.9), ("ur", 0.1)]));
        assert_eq!(identify(&[0.1; 16_000], &["hi", "ur", "en"]), "hi");
        override_for_test(Some(&[("en", 1.0)]));
        assert_eq!(identify(&[0.0; 16_000], &["hi", "ur", "en"]), "hi");
    }
}
