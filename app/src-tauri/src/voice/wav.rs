//! Reads the 16 kHz, 16-bit mono WAV fixtures for the transcription test (M9 Step 3).

use std::path::Path;

pub fn read_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let mut at = 12;
    let mut format = None;
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]])
            as usize;
        let body = &bytes[at + 8..(at + 8 + size).min(bytes.len())];
        if id == b"fmt " && body.len() >= 16 {
            let channels = u16::from_le_bytes([body[2], body[3]]);
            let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            let bits = u16::from_le_bytes([body[14], body[15]]);
            format = Some((channels, rate, bits));
        } else if id == b"data" {
            if format != Some((1, 16_000, 16)) {
                return Err(format!("expected 16 kHz 16-bit mono, got {format:?}"));
            }
            return Ok(super::stt::pcm16_to_f32(body));
        }
        at += 8 + size + (size % 2);
    }
    Err("no audio data in the WAV file".into())
}
