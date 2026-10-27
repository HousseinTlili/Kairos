//! The wake word "Kairos" (M9 Step 9): sherpa-onnx's keyword spotter, offline, on a small
//! streaming model downloaded on the user's click. The page streams the microphone in short chunks;
//! a detection opens the quick box listening.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::VoiceError;
use super::models::Expected;
use super::tts::{Package, RUNTIME};

pub const MODEL: Package = Package {
    expected: || {
        Expected {
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2".into(),
        file: "sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2".into(),
        size: 17_626_723,
        // From the release's checksum.txt (GitHub lists no digest for this asset).
        sha256: "f170013b4716e41b62b9bfd809687c207cef798ef9bc6534d524e17af9b6561a",
        host: "github.com",
    }
    },
    folder: "sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01",
    marker: "encoder-epoch-12-avg-2-chunk-16-left-64.onnx",
};

/// The wake word needs the sherpa-onnx runtime too (it comes with the natural voice).
pub const PACKAGES: [&Package; 2] = [&RUNTIME, &MODEL];

/// How "Kairos" may be heard: the spotter matches the model's spellings of sounds, so a few
/// spellings of the same sound are listed. Tested 2026-10-27 with George saying "Kairos, open
/// Notepad" and "Hey Kairos, what time is it?" (both found) and two sentences without it (none).
const SPELLINGS: [&str; 6] = ["KAIROS", "KYROS", "KIROS", "CAIROS", "KYRUS", "KIRUS"];

/// Sensitivity → the spotter's threshold: lower hears more, and wakes by mistake more. The window
/// is narrow: George's "Kairos" clips are found up to 0.32 and missed at 0.35, and at 0.25 the
/// live test sent two requests nobody made (2026-10-27), so normal leans stricter than that.
pub fn threshold(sensitivity: &str) -> f32 {
    match sensitivity {
        "strict" => 0.32,
        "eager" => 0.20,
        _ => 0.28,
    }
}

/// After a wake, the next this long of audio is ignored: one "Kairos" is one wake. Counted in
/// audio, not on the clock: Whisper's check takes seconds of its own.
pub const COOLDOWN: u64 = 4 * SAMPLE_RATE as u64;

pub const SAMPLE_RATE: i32 = 16_000;

// --- Two steps (2026-10-27) ----------------------------------------------------------------------
//
// The spotter alone missed the product owner every time: on their recordings it heard only the
// start of the name ("KA I O", "GA U"), never the "-ros", at any threshold. Whisper heard "Kairos"
// in three of five and "Cairo's" in a fourth. So a cheap first step picks moments that may hold the
// name, and Whisper confirms it:
// - the start of anything said after a moment's quiet (the name comes first: "Kairos, …");
// - a spotter hit, for the name in the middle of a sentence.

const RATE: usize = SAMPLE_RATE as usize;
/// The gate's frame: 20 ms.
const FRAME: usize = RATE / 50;
/// Quiet before speech that makes it the start of something said.
const QUIET_BEFORE: usize = RATE * 2 / 5;
/// Audio kept from before the start, so the first sound isn't clipped.
const PRE_ROLL: usize = RATE * 3 / 10;
/// How much of an utterance Whisper checks for the name: "Hey Kairos" fits.
const CHECKED: usize = RATE * 9 / 5;
/// Shorter than this is a click or a knock.
const MIN_SPEECH: usize = RATE / 4;
/// The audio history; a request kept after a wake is at most this plus what follows.
const HISTORY: usize = RATE * 12;
/// A request after a wake ends by itself after this much audio.
const MAX_CAPTURE: usize = RATE * 60;

/// A moment that may hold the name: the samples Whisper checks, and where they start in the
/// stream (a request after a wake is kept from there).
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub from: u64,
    pub samples: Vec<f32>,
}

/// Step one: finds where something is said after quiet, and keeps the last seconds of audio.
#[derive(Debug)]
pub struct Gate {
    history: std::collections::VecDeque<f32>,
    /// Samples ever pushed; positions in the stream are counted from the first.
    total: u64,
    frame: Vec<f32>,
    /// Background level in dBFS, following the room down quickly and up slowly.
    floor: f32,
    quiet: usize,
    /// The utterance being heard: where it started, how much was speech, whether it was checked.
    utterance: Option<(u64, usize, bool)>,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            history: std::collections::VecDeque::with_capacity(HISTORY),
            total: 0,
            frame: Vec::with_capacity(FRAME),
            floor: -70.0,
            quiet: QUIET_BEFORE,
            utterance: None,
        }
    }
}

impl Gate {
    /// Feeds samples; returns the start of each utterance once enough of it was heard.
    pub fn push(&mut self, samples: &[f32]) -> Vec<Candidate> {
        let mut found = Vec::new();
        for &s in samples {
            if self.history.len() == HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(s);
            self.total += 1;
            self.frame.push(s);
            if self.frame.len() == FRAME {
                found.extend(self.frame_done());
                self.frame.clear();
            }
        }
        found
    }

    fn frame_done(&mut self) -> Option<Candidate> {
        let energy = self.frame.iter().map(|s| s * s).sum::<f32>() / FRAME as f32;
        let db = 10.0 * energy.max(1e-12).log10();
        // Speech stands well above the room; the browser's noise suppression keeps the room low.
        let speech = db > (self.floor + 12.0).max(-55.0);
        if db < self.floor {
            self.floor = db;
        } else if !speech {
            self.floor += 0.02;
        }
        if speech {
            if self.utterance.is_none() && self.quiet >= QUIET_BEFORE {
                let start = self.total - FRAME as u64;
                self.utterance = Some((start, 0, false));
            }
            self.quiet = 0;
            if let Some((_, spoken, _)) = &mut self.utterance {
                *spoken += FRAME;
            }
        } else {
            self.quiet += FRAME;
        }
        let (start, spoken, checked) = self.utterance?;
        let heard = (self.total - start) as usize;
        let ended = self.quiet >= QUIET_BEFORE;
        let candidate = if !checked && spoken >= MIN_SPEECH && (heard >= CHECKED || ended) {
            self.utterance = Some((start, spoken, true));
            let from = start.saturating_sub(PRE_ROLL as u64);
            Some(Candidate {
                from,
                samples: self.since(from, CHECKED + PRE_ROLL),
            })
        } else {
            None
        };
        if ended {
            self.utterance = None;
        }
        candidate
    }

    /// The last `len` samples, for a spotter hit (the name just ended).
    pub fn recent(&self, len: usize) -> Candidate {
        let from = self.total.saturating_sub(len as u64);
        Candidate {
            from,
            samples: self.since(from, len),
        }
    }

    /// Up to `len` samples from stream position `from`, as far as the history still holds them.
    pub fn since(&self, from: u64, len: usize) -> Vec<f32> {
        let oldest = self.total - self.history.len() as u64;
        let skip = from.saturating_sub(oldest) as usize;
        self.history.iter().skip(skip).take(len).copied().collect()
    }

    pub fn position(&self) -> u64 {
        self.total
    }

    /// Whether an utterance started at or after stream position `from` is being heard.
    pub fn hearing_since(&self, from: u64) -> bool {
        self.utterance.is_some_and(|(start, _, _)| start >= from)
    }
}

/// How far a heard word may be from "kairos" (letters only) and still be the name.
fn distance(sensitivity: &str) -> usize {
    match sensitivity {
        "strict" => 1,
        "eager" => 3,
        _ => 2,
    }
}

fn levenshtein(a: &[u8], b: &[u8]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

/// Step two: whether Whisper's text starts with the name, within the first three words ("Hey,
/// Kairos." "Okay Kairos"). Returns what follows the name, or `None` when it isn't there. The name
/// must end in an s sound, so "Cairo" alone (a city) is not it, while "Cairo's" is.
pub fn after_name<'a>(text: &'a str, sensitivity: &str) -> Option<&'a str> {
    let mut rest = text;
    for _ in 0..3 {
        let trimmed = rest.trim_start_matches(|c: char| !c.is_alphanumeric());
        let end = trimmed
            .find(|c: char| c.is_whitespace() || matches!(c, ',' | '.' | '!' | '?' | ';' | ':'))
            .unwrap_or(trimmed.len());
        if end == 0 {
            return None;
        }
        let word: Vec<u8> = trimmed[..end]
            .bytes()
            .filter(u8::is_ascii_alphabetic)
            .map(|b| b.to_ascii_lowercase())
            .collect();
        let named = (4..=9).contains(&word.len())
            && matches!(word.first(), Some(b'k' | b'c' | b'q' | b'g'))
            && matches!(word.last(), Some(b's' | b'z'))
            && levenshtein(&word, b"kairos") <= distance(sensitivity);
        rest = &trimmed[end..];
        if named {
            return Some(rest.trim_start_matches(|c: char| !c.is_alphanumeric()));
        }
    }
    None
}

/// A request kept after a wake: from the start of the utterance that held the name, as it grows.
#[derive(Debug, Default)]
struct Capture {
    samples: Vec<f32>,
}

/// Every way `word` splits into the model's BPE pieces (the first marked as a word start).
pub fn spellings(word: &str, tokens: &std::collections::HashSet<String>) -> Vec<String> {
    fn split(
        word: &str,
        at: usize,
        acc: &mut Vec<String>,
        tokens: &std::collections::HashSet<String>,
        out: &mut Vec<String>,
    ) {
        if at == word.len() {
            out.push(acc.join(" "));
            return;
        }
        for end in at + 1..=word.len() {
            let piece = if at == 0 {
                format!("▁{}", &word[at..end])
            } else {
                word[at..end].to_string()
            };
            if tokens.contains(&piece) {
                acc.push(piece);
                split(word, end, acc, tokens, out);
                acc.pop();
            }
        }
    }
    let mut out = Vec::new();
    split(word, 0, &mut Vec::new(), tokens, &mut out);
    out
}

/// The keyword list for the spotter, from the model's `tokens.txt`: each spelling alone and after
/// "Hey" (which the spotter otherwise misses in "Hey Kairos, …"), all reported as KAIROS.
pub fn keywords(tokens_txt: &str) -> String {
    let tokens: std::collections::HashSet<String> = tokens_txt
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_string)
        .collect();
    let hey = spellings("HEY", &tokens);
    let mut lines = Vec::new();
    for word in SPELLINGS {
        for spelling in spellings(word, &tokens) {
            lines.push(format!("{spelling} @KAIROS"));
            for h in &hey {
                lines.push(format!("{h} {spelling} @KAIROS"));
            }
        }
    }
    lines.join("\n") + "\n"
}

/// A capture nobody collected (the quick box closed) is dropped after this long.
const CAPTURE_EXPIRES: Duration = Duration::from_secs(75);

/// A wake: whether words already followed the name in what Whisper checked ("Kairos, open…").
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Woken {
    pub spoken: bool,
}

/// The spotter (loaded on first use and kept while the wake word is on), the gate, and the
/// request kept after a wake.
#[derive(Default)]
pub struct Wake {
    loaded: Mutex<Option<(ffi::Spotter, f32)>>,
    /// Where in the stream the last wake was.
    last: Mutex<Option<u64>>,
    gate: Mutex<Gate>,
    capture: Mutex<Option<(Capture, Instant)>>,
}

impl Wake {
    /// Feeds 16 kHz mono samples. `confirm` is Whisper: it gets a moment that may hold the name
    /// and returns what it heard. Blocking (Whisper takes about 2 s on the product owner's PC).
    pub fn feed(
        &self,
        dir: &Path,
        samples: &[f32],
        sensitivity: &str,
        mut confirm: impl FnMut(&[f32]) -> Result<String, VoiceError>,
    ) -> Result<Option<Woken>, VoiceError> {
        let spotted = self.spot(dir, samples, sensitivity)?;
        let mut gate = self.gate.lock().map_err(|_| VoiceError::Busy)?;
        let mut candidates = gate.push(samples);
        {
            let mut capture = self.capture.lock().map_err(|_| VoiceError::Busy)?;
            if capture
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() > CAPTURE_EXPIRES)
            {
                *capture = None;
            }
            if let Some((kept, _)) = capture.as_mut() {
                // While a request is heard, it grows; no new wake.
                let room = MAX_CAPTURE.saturating_sub(kept.samples.len());
                kept.samples
                    .extend_from_slice(&samples[..samples.len().min(room)]);
                return Ok(None);
            }
        }
        if self
            .last
            .lock()
            .map_err(|_| VoiceError::Busy)?
            .is_some_and(|at| gate.position() < at + COOLDOWN)
        {
            return Ok(None);
        }
        // A hit inside an utterance the gate checks anyway needs no second look.
        if spotted
            && !gate.hearing_since(gate.position().saturating_sub((CHECKED + PRE_ROLL) as u64))
        {
            candidates.push(gate.recent(CHECKED + PRE_ROLL));
        }
        for candidate in candidates {
            let heard = confirm(&candidate.samples)?;
            if let Some(rest) = after_name(&heard, sensitivity) {
                let kept = Capture {
                    samples: gate.since(candidate.from, MAX_CAPTURE),
                };
                *self.capture.lock().map_err(|_| VoiceError::Busy)? = Some((kept, Instant::now()));
                *self.last.lock().map_err(|_| VoiceError::Busy)? = Some(gate.position());
                return Ok(Some(Woken {
                    spoken: !rest.is_empty(),
                }));
            }
        }
        Ok(None)
    }

    /// The request heard since the wake, for Whisper; the capture ends.
    pub fn take_capture(&self) -> Option<Vec<f32>> {
        self.capture
            .lock()
            .ok()?
            .take()
            .map(|(kept, _)| kept.samples)
    }

    /// The quick box stopped listening without a request.
    pub fn drop_capture(&self) {
        if let Ok(mut capture) = self.capture.lock() {
            *capture = None;
        }
    }

    fn spot(&self, dir: &Path, samples: &[f32], sensitivity: &str) -> Result<bool, VoiceError> {
        let wanted = threshold(sensitivity);
        let mut guard = self.loaded.lock().map_err(|_| VoiceError::Busy)?;
        if guard.as_ref().is_none_or(|(_, t)| *t != wanted) {
            if !super::tts::is_installed_all(dir, &PACKAGES) {
                return Err(VoiceError::Model(
                    "The wake word isn't downloaded: Settings ▸ Voice.".into(),
                ));
            }
            let model_dir = dir.join(MODEL.folder);
            let tokens = std::fs::read_to_string(model_dir.join("tokens.txt"))?;
            *guard = Some((
                ffi::Spotter::load(
                    &dir.join(RUNTIME.folder).join("lib"),
                    &model_dir,
                    &keywords(&tokens),
                    wanted,
                )?,
                wanted,
            ));
        }
        let (spotter, _) = guard.as_ref().ok_or(VoiceError::Busy)?;
        Ok(spotter.feed(samples))
    }

    /// When the wake word is switched off or removed.
    pub fn unload(&self) {
        if let Ok(mut guard) = self.loaded.lock() {
            *guard = None;
        }
        if let Ok(mut gate) = self.gate.lock() {
            *gate = Gate::default();
        }
        self.drop_capture();
    }
}

#[cfg(windows)]
mod ffi {
    //! The sherpa-onnx keyword-spotter functions Kairos needs, from `c-api.h` of v1.13.8; the
    //! structs match that header field for field, zeroed fields meaning "default".

    use std::ffi::{CString, c_char, c_void};
    use std::path::Path;

    use super::super::VoiceError;
    use super::super::tts::ffi::{c, library, symbol};

    #[repr(C)]
    struct Feature {
        sample_rate: i32,
        feature_dim: i32,
    }

    #[repr(C)]
    struct Transducer {
        encoder: *const c_char,
        decoder: *const c_char,
        joiner: *const c_char,
    }

    #[repr(C)]
    struct TwoFiles {
        encoder: *const c_char,
        decoder: *const c_char,
    }

    #[repr(C)]
    struct OneFile {
        model: *const c_char,
    }

    #[repr(C)]
    struct OnlineModel {
        transducer: Transducer,
        paraformer: TwoFiles,
        zipformer2_ctc: OneFile,
        tokens: *const c_char,
        num_threads: i32,
        provider: *const c_char,
        debug: i32,
        model_type: *const c_char,
        modeling_unit: *const c_char,
        bpe_vocab: *const c_char,
        tokens_buf: *const c_char,
        tokens_buf_size: i32,
        nemo_ctc: OneFile,
        t_one_ctc: OneFile,
    }

    #[repr(C)]
    struct Config {
        feat_config: Feature,
        model_config: OnlineModel,
        max_active_paths: i32,
        num_trailing_blanks: i32,
        keywords_score: f32,
        keywords_threshold: f32,
        keywords_file: *const c_char,
        keywords_buf: *const c_char,
        keywords_buf_size: i32,
    }

    #[repr(C)]
    struct KeywordResult {
        keyword: *const c_char,
        // The rest of the struct is read by sherpa-onnx only.
    }

    type Create = unsafe extern "C" fn(*const Config) -> *const c_void;
    type CreateStream = unsafe extern "C" fn(*const c_void) -> *const c_void;
    type Accept = unsafe extern "C" fn(*const c_void, i32, *const f32, i32);
    type Ready = unsafe extern "C" fn(*const c_void, *const c_void) -> i32;
    type Decode = unsafe extern "C" fn(*const c_void, *const c_void);
    type Reset = unsafe extern "C" fn(*const c_void, *const c_void);
    type GetResult = unsafe extern "C" fn(*const c_void, *const c_void) -> *const KeywordResult;
    type DestroyResult = unsafe extern "C" fn(*const KeywordResult);
    type DestroyStream = unsafe extern "C" fn(*const c_void);
    type DestroySpotter = unsafe extern "C" fn(*const c_void);

    pub struct Spotter {
        spotter: *const c_void,
        stream: *const c_void,
        accept: Accept,
        ready: Ready,
        decode: Decode,
        reset: Reset,
        get_result: GetResult,
        destroy_result: DestroyResult,
        destroy_stream: DestroyStream,
        destroy_spotter: DestroySpotter,
    }

    // SAFETY: used only behind `Wake`'s mutex, one call at a time; no thread affinity.
    unsafe impl Send for Spotter {}

    impl Spotter {
        pub fn load(
            lib_dir: &Path,
            model_dir: &Path,
            keywords: &str,
            threshold: f32,
        ) -> Result<Self, VoiceError> {
            let module = library(lib_dir)?;
            let create: Create = symbol(module, "SherpaOnnxCreateKeywordSpotter")?;
            let create_stream: CreateStream = symbol(module, "SherpaOnnxCreateKeywordStream")?;
            let encoder = c(&model_dir.join("encoder-epoch-12-avg-2-chunk-16-left-64.onnx"))?;
            let decoder = c(&model_dir.join("decoder-epoch-12-avg-2-chunk-16-left-64.onnx"))?;
            let joiner = c(&model_dir.join("joiner-epoch-12-avg-2-chunk-16-left-64.onnx"))?;
            let tokens = c(&model_dir.join("tokens.txt"))?;
            let provider = CString::new("cpu").map_err(|_| VoiceError::Busy)?;
            let keywords = CString::new(keywords).map_err(|_| VoiceError::Busy)?;
            // SAFETY: plain pointers, integers and floats; all-zero is sherpa-onnx's "unset".
            let mut config: Config = unsafe { std::mem::zeroed() };
            config.feat_config.sample_rate = super::SAMPLE_RATE;
            config.feat_config.feature_dim = 80;
            config.model_config.transducer.encoder = encoder.as_ptr();
            config.model_config.transducer.decoder = decoder.as_ptr();
            config.model_config.transducer.joiner = joiner.as_ptr();
            config.model_config.tokens = tokens.as_ptr();
            // Listening all the time: one thread keeps it light.
            config.model_config.num_threads = 1;
            config.model_config.provider = provider.as_ptr();
            config.max_active_paths = 4;
            config.num_trailing_blanks = 1;
            config.keywords_score = 2.0;
            config.keywords_threshold = threshold;
            config.keywords_buf = keywords.as_ptr();
            config.keywords_buf_size = keywords.as_bytes().len() as i32;
            // SAFETY: `config` and its strings outlive the call; sherpa-onnx copies them.
            let spotter = unsafe { create(&config) };
            if spotter.is_null() {
                return Err(VoiceError::Model(
                    "the wake word didn't start (are its files complete?)".into(),
                ));
            }
            // SAFETY: a live spotter.
            let stream = unsafe { create_stream(spotter) };
            let this = Self {
                spotter,
                stream,
                accept: symbol(module, "SherpaOnnxOnlineStreamAcceptWaveform")?,
                ready: symbol(module, "SherpaOnnxIsKeywordStreamReady")?,
                decode: symbol(module, "SherpaOnnxDecodeKeywordStream")?,
                reset: symbol(module, "SherpaOnnxResetKeywordStream")?,
                get_result: symbol(module, "SherpaOnnxGetKeywordResult")?,
                destroy_result: symbol(module, "SherpaOnnxDestroyKeywordResult")?,
                destroy_stream: symbol(module, "SherpaOnnxDestroyOnlineStream")?,
                destroy_spotter: symbol(module, "SherpaOnnxDestroyKeywordSpotter")?,
            };
            if stream.is_null() {
                return Err(VoiceError::Model(
                    "the wake word's stream didn't start".into(),
                ));
            }
            Ok(this)
        }

        /// True when a keyword was found in what has been fed so far.
        pub fn feed(&self, samples: &[f32]) -> bool {
            let mut heard = false;
            // SAFETY: live spotter and stream; the samples outlive the call; each result is
            // read before it is freed.
            unsafe {
                (self.accept)(
                    self.stream,
                    super::SAMPLE_RATE,
                    samples.as_ptr(),
                    samples.len() as i32,
                );
                while (self.ready)(self.spotter, self.stream) == 1 {
                    (self.decode)(self.spotter, self.stream);
                    let result = (self.get_result)(self.spotter, self.stream);
                    if !result.is_null() {
                        let keyword = (*result).keyword;
                        if !keyword.is_null() && *keyword != 0 {
                            heard = true;
                            (self.reset)(self.spotter, self.stream);
                        }
                        (self.destroy_result)(result);
                    }
                }
            }
            heard
        }
    }

    impl Drop for Spotter {
        fn drop(&mut self) {
            // SAFETY: created by sherpa-onnx above, freed once, the stream before its spotter.
            unsafe {
                (self.destroy_stream)(self.stream);
                (self.destroy_spotter)(self.spotter);
            }
        }
    }
}

#[cfg(not(windows))]
mod ffi {
    use std::path::Path;

    use super::super::VoiceError;

    pub struct Spotter;

    impl Spotter {
        pub fn load(_: &Path, _: &Path, _: &str, _: f32) -> Result<Self, VoiceError> {
            Err(VoiceError::Model("the wake word needs Windows".into()))
        }

        pub fn feed(&self, _: &[f32]) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKENS: &str = "<blk> 0\n▁K 1\nA 2\nI 3\nRO 4\nS 5\nIR 6\nO 7\nR 8\n▁HE 9\nY 10\n";

    #[test]
    fn kairos_is_listed_in_every_spelling_the_model_has() {
        let tokens = TOKENS
            .lines()
            .map(|l| l.split(' ').next().unwrap().to_string())
            .collect();
        let mut found = spellings("KAIROS", &tokens);
        found.sort();
        assert_eq!(found, vec!["▁K A I R O S", "▁K A I RO S", "▁K A IR O S"]);
        assert!(spellings("ZEBRA", &tokens).is_empty());
    }

    #[test]
    fn each_spelling_is_also_listed_after_hey() {
        let list = keywords(TOKENS);
        assert!(list.lines().any(|l| l == "▁K A I RO S @KAIROS"));
        assert!(list.lines().any(|l| l == "▁HE Y ▁K A I RO S @KAIROS"));
        assert!(list.lines().all(|l| l.ends_with("@KAIROS")));
    }

    #[test]
    fn sensitivity_moves_the_threshold_and_defaults_to_normal() {
        assert!(threshold("strict") > threshold("normal"));
        assert!(threshold("eager") < threshold("normal"));
        assert_eq!(threshold("anything"), threshold("normal"));
    }

    /// Downloads the real wake-word model (17.6 MB) into %APPDATA%\kairos\voices (M9 Step 9).
    #[test]
    #[ignore]
    fn the_wake_word_installs() {
        let dir = super::super::tts::default_dir().unwrap();
        tauri::async_runtime::block_on(super::super::tts::install_packages(
            &dir,
            &PACKAGES,
            |_, _| {},
        ))
        .unwrap();
        assert!(super::super::tts::is_installed_all(&dir, &PACKAGES));
    }

    fn tone(seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * RATE as f32) as usize)
            .map(|i| amplitude * (i as f32 * 300.0 * std::f32::consts::TAU / RATE as f32).sin())
            .collect()
    }

    /// A quiet room: about -70 dBFS of noise.
    fn room(seconds: f32) -> Vec<f32> {
        (0..(seconds * RATE as f32) as usize)
            .map(|i| if i % 2 == 0 { 0.0003 } else { -0.0003 })
            .collect()
    }

    fn candidates(parts: &[Vec<f32>]) -> Vec<Candidate> {
        let mut gate = Gate::default();
        parts.iter().flat_map(|p| gate.push(p)).collect()
    }

    #[test]
    fn a_word_said_after_quiet_is_checked_once_with_a_little_before_it() {
        let found = candidates(&[room(1.0), tone(0.8, 0.3), room(1.0)]);
        assert_eq!(found.len(), 1);
        let start = RATE as u64;
        assert!(found[0].from <= start && found[0].from >= start - PRE_ROLL as u64 - FRAME as u64);
        assert!(found[0].samples.len() <= CHECKED + PRE_ROLL);
    }

    #[test]
    fn a_long_sentence_is_checked_once_at_its_start() {
        let found = candidates(&[room(1.0), tone(5.0, 0.3), room(1.0)]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].samples.len(), CHECKED + PRE_ROLL);
    }

    #[test]
    fn a_click_is_not_speech() {
        assert!(candidates(&[room(1.0), tone(0.06, 0.5), room(1.0)]).is_empty());
    }

    #[test]
    fn each_thing_said_after_a_pause_is_checked() {
        let found = candidates(&[
            room(1.0),
            tone(0.6, 0.3),
            room(0.6),
            tone(0.6, 0.3),
            room(1.0),
        ]);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn the_name_is_found_however_whisper_spelled_it() {
        assert_eq!(after_name("Kairos.", "normal"), Some(""));
        assert_eq!(
            after_name("Hey, Kairos. Kairos.", "normal"),
            Some("Kairos.")
        );
        assert_eq!(after_name("Cairo's", "normal"), Some(""));
        assert_eq!(
            after_name("Kyros, open Notepad.", "normal"),
            Some("open Notepad.")
        );
        assert_eq!(
            after_name("Okay Kairos what time is it", "normal"),
            Some("what time is it")
        );
    }

    #[test]
    fn other_words_are_not_the_name() {
        assert_eq!(after_name("I went to Cairo.", "normal"), None);
        assert_eq!(after_name("Chaos everywhere", "normal"), None);
        assert_eq!(after_name("What a nice day, Kairos", "normal"), None);
        assert_eq!(after_name("", "normal"), None);
        assert_eq!(after_name("Carros", "strict"), None);
    }

    #[test]
    fn eager_accepts_further_spellings() {
        assert_eq!(after_name("Guyros.", "normal"), None);
        assert_eq!(after_name("Guyros.", "eager"), Some(""));
    }

    /// Needs the wake word and the Small Whisper model installed under %APPDATA%\kairos, and clips
    /// in target\kws\tests: George's (made with Kokoro) and, when present, the product owner's
    /// own recording (`master-raw.wav`, never committed: it is their voice).
    #[test]
    #[ignore]
    fn saying_kairos_wakes_it_and_other_sentences_dont() {
        let dir = super::super::tts::default_dir().unwrap();
        let models = super::super::models::default_dir().unwrap();
        let model = super::super::models::path(&models, super::super::models::ModelId::Small);
        let engine = super::super::stt::Engine::default();
        let clips = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/kws/tests"
        ));
        let mut cases = vec![
            ("kairos-open", 1),
            ("hey-kairos", 1),
            ("no-carrots", 0),
            ("no-chaos", 0),
        ];
        if clips.join("master-raw.wav").exists() {
            // Seven times "Kairos", a few seconds apart (the product owner, 2026-10-27). Each wake
            // hides the next 4 s.
            cases.push(("master-raw", 2));
            // The same five one by one; Whisper wrote one of them "Queiras", which isn't the name.
            for u in ["u1", "u2", "u3", "u4", "u5"] {
                cases.push((u, 0));
            }
        }
        let mut each = 0;
        for (clip, expected) in cases {
            let wake = Wake::default();
            let samples =
                super::super::wav::read_mono_any(&clips.join(format!("{clip}.wav"))).unwrap();
            let mut wakes = 0;
            let mut fed = 0usize;
            let mut feed = |chunk: &[f32]| {
                fed += chunk.len();
                let at = fed as f32 / RATE as f32;
                let woken = wake
                    .feed(&dir, chunk, "normal", |s| {
                        let heard = engine.transcribe(&model, s, None)?;
                        println!("{clip} at {at:.1} s: Whisper heard {:?}", heard.text);
                        Ok(heard.text)
                    })
                    .unwrap();
                if woken.is_some() {
                    wakes += 1;
                    // The quick box collects the request; the next wake can come.
                    wake.drop_capture();
                }
            };
            for chunk in samples.chunks(3200) {
                feed(chunk);
            }
            // A second of quiet lets the last words finish.
            feed(&room(1.0));
            println!("{clip}: {wakes} wakes");
            if clip.starts_with('u') {
                each += wakes;
                continue;
            }
            assert!(
                if expected == 0 {
                    wakes == 0
                } else {
                    wakes >= expected
                },
                "{clip}: {wakes} wakes"
            );
        }
        if clips.join("u1.wav").exists() {
            assert!(each >= 4, "{each} of the product owner's five");
        }
    }
}
