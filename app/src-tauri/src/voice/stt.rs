//! Speech to text with whisper.cpp (M9 §3, D-022). The samples come from the webview, already
//! 16 kHz mono; they stay in memory and are dropped after the call.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::VoiceError;

pub const SAMPLE_RATE: usize = 16_000;
/// The recorder stops at 60 s; a little slack for the last buffer.
pub const MAX_SECONDS: usize = 62;
/// Shorter than this is a key tap, not speech.
pub const MIN_SECONDS: f32 = 0.3;
/// Root-mean-square level below which a recording is treated as silence. Whisper invents text
/// on silence ("Thank you."), so silence never reaches it.
pub const SILENCE_RMS: f32 = 0.004;
/// The model holds a few hundred MB; it goes after this long without speech.
pub const IDLE_UNLOAD: Duration = Duration::from_secs(10 * 60);

/// Languages the user can fix in Settings ▸ Voice; anything else means "detect".
pub const LANGUAGES: [&str; 3] = ["en", "fr", "ar"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Transcript {
    pub text: String,
    /// Whisper's code for the language it heard ("en", "fr", "ar"…); `None` for silence.
    pub language: Option<String>,
    /// How long transcription took, for the "under 2 s" check.
    pub elapsed_ms: u64,
}

/// Signed 16-bit little-endian samples, as the webview sends them, to Whisper's floats.
pub fn pcm16_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|s| f32::from(i16::from_le_bytes(*s)) / 32768.0)
        .collect()
}

/// Refuses recordings that can't be speech before any model work.
pub fn check_samples(samples: &[f32]) -> Result<(), VoiceError> {
    if samples.len() > MAX_SECONDS * SAMPLE_RATE {
        return Err(VoiceError::Audio(
            "Recordings are limited to 60 seconds.".into(),
        ));
    }
    if samples.iter().any(|s| !s.is_finite()) {
        return Err(VoiceError::Audio("The recording was damaged.".into()));
    }
    Ok(())
}

pub fn is_silence(samples: &[f32]) -> bool {
    if (samples.len() as f32) < MIN_SECONDS * SAMPLE_RATE as f32 {
        return true;
    }
    let energy: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    ((energy / samples.len() as f64).sqrt() as f32) < SILENCE_RMS
}

/// Whisper's own markers for non-speech ("[BLANK_AUDIO]", "(music)") are dropped; what is left is
/// joined with single spaces.
pub fn clean_text(segments: &[String]) -> String {
    let mut words = Vec::new();
    for segment in segments {
        let mut text = segment.as_str();
        let mut kept = String::new();
        while let Some(start) = text.find(['[', '(']) {
            let close = if text[start..].starts_with('[') {
                ']'
            } else {
                ')'
            };
            kept.push_str(&text[..start]);
            match text[start..].find(close) {
                Some(end) => text = &text[start + end + 1..],
                None => {
                    text = &text[start..];
                    break;
                }
            }
        }
        kept.push_str(text);
        words.extend(kept.split_whitespace().map(str::to_string));
    }
    words.join(" ")
}

pub fn language_setting(text: Option<&str>) -> Option<&'static str> {
    text.and_then(|t| LANGUAGES.into_iter().find(|l| *l == t))
}

struct Loaded {
    path: PathBuf,
    context: WhisperContext,
    last_used: Instant,
}

/// One model loaded at a time, on first use.
#[derive(Default)]
pub struct Engine {
    loaded: Mutex<Option<Loaded>>,
}

impl Engine {
    /// Blocking: call it from a worker thread.
    pub fn transcribe(
        &self,
        model: &Path,
        samples: &[f32],
        language: Option<&str>,
    ) -> Result<Transcript, VoiceError> {
        let started = Instant::now();
        check_samples(samples)?;
        if is_silence(samples) {
            return Ok(Transcript {
                text: String::new(),
                language: None,
                elapsed_ms: 0,
            });
        }
        let mut guard = self.loaded.lock().map_err(|_| VoiceError::Busy)?;
        if guard.as_ref().is_none_or(|l| l.path != model) {
            // Quieten whisper.cpp's own console output (it would otherwise print every load).
            whisper_rs::install_logging_hooks();
            let context =
                WhisperContext::new_with_params(model, WhisperContextParameters::default())
                    .map_err(|err| {
                        VoiceError::Model(format!("couldn't load the voice model: {err}"))
                    })?;
            *guard = Some(Loaded {
                path: model.to_path_buf(),
                context,
                last_used: Instant::now(),
            });
        }
        let loaded = guard.as_mut().ok_or(VoiceError::Busy)?;
        loaded.last_used = Instant::now();

        let mut state = loaded
            .context
            .create_state()
            .map_err(|err| VoiceError::Model(format!("couldn't start transcribing: {err}")))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(language_setting(language).unwrap_or("auto")));
        params.set_n_threads(threads());
        params.set_audio_ctx(audio_ctx(samples.len()));
        params.set_no_context(true);
        params.set_no_timestamps(true);
        params.set_suppress_blank(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        state
            .full(params, samples)
            .map_err(|err| VoiceError::Model(format!("transcription failed: {err}")))?;

        let segments: Vec<String> = state
            .as_iter()
            .map(|segment| {
                segment
                    .to_str_lossy()
                    .map(|t| t.into_owned())
                    .unwrap_or_default()
            })
            .collect();
        let language =
            whisper_rs::get_lang_str(state.full_lang_id_from_state()).map(str::to_string);
        Ok(Transcript {
            text: clean_text(&segments),
            language,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Frees the model after `IDLE_UNLOAD` without use; called from a timer.
    pub fn unload_if_idle(&self) {
        if let Ok(mut guard) = self.loaded.try_lock()
            && guard
                .as_ref()
                .is_some_and(|l| l.last_used.elapsed() > IDLE_UNLOAD)
        {
            *guard = None;
        }
    }

    /// Before a model file is deleted.
    pub fn unload(&self) {
        if let Ok(mut guard) = self.loaded.lock() {
            *guard = None;
        }
    }
}

/// Whisper encodes a 30 s window however short the recording; a window cut to the recording's
/// length (plus a margin) makes a short sentence about twice as fast (M9 Step 3: 4.2 s → 2.3 s on
/// the product owner's PC). Not below 256 frames (≈ 5 s), where the last word started to suffer.
pub fn audio_ctx(samples: usize) -> i32 {
    const FULL: usize = 1500; // frames in 30 s
    let seconds = samples as f32 / SAMPLE_RATE as f32;
    if seconds >= 25.0 {
        return 0; // the whole window: whisper.cpp's default
    }
    (((seconds + 2.0) / 30.0 * FULL as f32).ceil() as i32).max(256)
}

/// Leaves cores for the rest of the PC; past 12 threads whisper.cpp hardly gets faster.
fn threads() -> i32 {
    std::thread::available_parallelism()
        .map_or(4, |n| n.get().saturating_sub(2))
        .clamp(1, 12) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f32, amplitude: f32) -> Vec<f32> {
        (0..(seconds * SAMPLE_RATE as f32) as usize)
            .map(|i| {
                amplitude * (i as f32 * 440.0 * std::f32::consts::TAU / SAMPLE_RATE as f32).sin()
            })
            .collect()
    }

    #[test]
    fn the_webviews_16_bit_samples_become_floats() {
        let bytes = [0x00, 0x80, 0x00, 0x00, 0xff, 0x7f, 0x01];
        let samples = pcm16_to_f32(&bytes);
        assert_eq!(samples.len(), 3, "a stray odd byte is ignored");
        assert_eq!(samples[0], -1.0);
        assert_eq!(samples[1], 0.0);
        assert!((samples[2] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn a_quiet_or_very_short_recording_is_silence() {
        assert!(is_silence(&[]));
        assert!(is_silence(&tone(0.1, 0.5)), "a key tap");
        assert!(is_silence(&tone(2.0, 0.001)), "room noise");
        assert!(!is_silence(&tone(2.0, 0.1)), "someone speaking");
    }

    #[test]
    fn recordings_over_a_minute_or_damaged_are_refused() {
        assert!(check_samples(&tone(1.0, 0.1)).is_ok());
        assert!(check_samples(&vec![0.0; (MAX_SECONDS + 1) * SAMPLE_RATE]).is_err());
        assert!(check_samples(&[0.1, f32::NAN]).is_err());
    }

    #[test]
    fn silence_never_reaches_the_model() {
        // No model file exists at this path: silence must return before loading anything.
        let engine = Engine::default();
        let transcript = engine
            .transcribe(Path::new("no-such-model.bin"), &tone(2.0, 0.0005), None)
            .unwrap();
        assert_eq!(transcript.text, "");
        assert_eq!(transcript.language, None);
    }

    #[test]
    fn whispers_non_speech_markers_are_dropped() {
        let segments = vec![
            " [BLANK_AUDIO]".to_string(),
            " Open Notepad (music) please. ".to_string(),
            " Merci  beaucoup".to_string(),
        ];
        assert_eq!(clean_text(&segments), "Open Notepad please. Merci beaucoup");
        assert_eq!(clean_text(&["(unclosed".to_string()]), "(unclosed");
        assert_eq!(clean_text(&["افتح المفكرة".to_string()]), "افتح المفكرة");
    }

    #[test]
    fn the_audio_window_follows_the_recording_length() {
        assert_eq!(
            audio_ctx(2 * SAMPLE_RATE),
            256,
            "short sentences keep a floor"
        );
        assert_eq!(audio_ctx(10 * SAMPLE_RATE), 600);
        assert_eq!(
            audio_ctx(30 * SAMPLE_RATE),
            0,
            "long ones use whisper.cpp's full window"
        );
    }

    #[test]
    fn only_the_offered_languages_can_be_fixed() {
        assert_eq!(language_setting(Some("fr")), Some("fr"));
        assert_eq!(language_setting(Some("de")), None);
        assert_eq!(language_setting(None), None);
    }

    /// Needs the Small model in %APPDATA%\kairos\models and the fixture WAV (M9 Step 3).
    #[test]
    #[ignore]
    fn the_fixture_sentence_is_transcribed_quickly() {
        let dir = super::super::models::default_dir().unwrap();
        let model = super::super::models::path(&dir, super::super::models::ModelId::Small);
        let samples = super::super::wav::read_16k_mono(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/voice/open-notepad.wav"
        )))
        .unwrap();
        let transcript = Engine::default()
            .transcribe(&model, &samples, None)
            .unwrap();
        let text = transcript.text.to_lowercase();
        assert!(text.contains("notepad"), "{transcript:?}");
        assert_eq!(transcript.language.as_deref(), Some("en"));
        assert!(transcript.elapsed_ms < 4_000, "{transcript:?}");
    }
}
