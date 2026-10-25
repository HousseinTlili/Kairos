//! Voice (M9, D-022): local speech to text with Whisper, push-to-talk only. Speech out is the
//! webview's `speechSynthesis` and needs nothing here.

pub mod models;
pub mod stt;
#[cfg(test)]
mod wav;

use std::sync::atomic::AtomicBool;

use thiserror::Error;

pub const MODEL_SETTING: &str = "voice_model";
pub const LANGUAGE_SETTING: &str = "voice_language";
pub const READ_ALOUD_SETTING: &str = "voice_read_aloud";
/// Asked by voice, answered by voice: on unless switched off ("0").
pub const ANSWER_SPOKEN_SETTING: &str = "voice_answer_spoken";
pub const PTT_SETTING: &str = "voice_ptt_hotkey";
/// Next to the quick box's Ctrl+Alt+K; J for "just say it".
pub const DEFAULT_PTT: &str = "Ctrl+Alt+J";

#[derive(Debug, Error)]
pub enum VoiceError {
    #[error("{0}")]
    Model(String),
    #[error("{0}")]
    Audio(String),
    #[error("the voice engine is busy; try again")]
    Busy,
    #[error("couldn't write the voice model: {0}")]
    Io(#[from] std::io::Error),
}

/// Shared between the voice commands.
#[derive(Default)]
pub struct VoiceState {
    pub engine: stt::Engine,
    /// One model download at a time.
    pub downloading: AtomicBool,
}
