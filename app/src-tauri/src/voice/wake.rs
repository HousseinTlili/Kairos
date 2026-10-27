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

/// After a wake, the next this long is ignored: one "Kairos" is one wake.
pub const COOLDOWN: Duration = Duration::from_secs(4);

pub const SAMPLE_RATE: i32 = 16_000;

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

/// The spotter, loaded on first use and kept while the wake word is on.
#[derive(Default)]
pub struct Wake {
    loaded: Mutex<Option<(ffi::Spotter, f32)>>,
    last: Mutex<Option<Instant>>,
}

impl Wake {
    /// Feeds 16 kHz mono samples; true once when "Kairos" was heard. Blocking.
    pub fn feed(&self, dir: &Path, samples: &[f32], sensitivity: &str) -> Result<bool, VoiceError> {
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
        if !spotter.feed(samples) {
            return Ok(false);
        }
        let mut last = self.last.lock().map_err(|_| VoiceError::Busy)?;
        if last.is_some_and(|at| at.elapsed() < COOLDOWN) {
            return Ok(false);
        }
        *last = Some(Instant::now());
        Ok(true)
    }

    /// When the wake word is switched off or removed.
    pub fn unload(&self) {
        if let Ok(mut guard) = self.loaded.lock() {
            *guard = None;
        }
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

    /// Needs the wake word installed in %APPDATA%\kairos\voices and the test clips George made in
    /// target\kws\tests (M9 Step 9).
    #[test]
    #[ignore]
    fn george_saying_kairos_wakes_it_and_other_sentences_dont() {
        let dir = super::super::tts::default_dir().unwrap();
        let clips = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/kws/tests"
        ));
        for (clip, expected) in [
            ("kairos-open", true),
            ("hey-kairos", true),
            ("no-carrots", false),
            ("no-chaos", false),
        ] {
            let wake = Wake::default();
            let samples =
                super::super::wav::read_mono_any(&clips.join(format!("{clip}.wav"))).unwrap();
            let mut heard = false;
            for chunk in samples.chunks(1600) {
                heard |= wake.feed(&dir, chunk, "normal").unwrap();
            }
            // A second of silence lets the last words finish decoding.
            heard |= wake.feed(&dir, &vec![0.0; 16_000], "normal").unwrap();
            assert_eq!(heard, expected, "{clip}");
        }
    }
}
