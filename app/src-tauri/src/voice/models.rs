//! The Whisper models (M9 §3): downloaded once, on the user's click, into
//! `%APPDATA%\kairos\models`, and checked against the SHA-256 the whisper.cpp project publishes.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::VoiceError;

const SOURCE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
/// Generous: 190 MB on a slow line. The connection itself has its own, shorter timeout.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelId {
    Small,
    Base,
}

impl ModelId {
    pub const ALL: [ModelId; 2] = [ModelId::Small, ModelId::Base];

    pub fn file(self) -> &'static str {
        match self {
            ModelId::Small => "ggml-small-q5_1.bin",
            ModelId::Base => "ggml-base-q5_1.bin",
        }
    }

    pub fn size(self) -> u64 {
        match self {
            ModelId::Small => 190_085_487,
            ModelId::Base => 59_707_625,
        }
    }

    /// From the Hugging Face file list of ggerganov/whisper.cpp (M9 §3).
    pub fn sha256(self) -> &'static str {
        match self {
            ModelId::Small => "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
            ModelId::Base => "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ModelId::Small => "Small: best for French and Arabic",
            ModelId::Base => "Base: faster, weaker on Arabic",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ModelId::Small => "small",
            ModelId::Base => "base",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.as_str() == text)
    }
}

pub fn default_dir() -> Result<PathBuf, VoiceError> {
    std::env::var_os("APPDATA")
        .map(|appdata| PathBuf::from(appdata).join("kairos").join("models"))
        .ok_or_else(|| VoiceError::Model("APPDATA is not set".into()))
}

pub fn path(dir: &Path, id: ModelId) -> PathBuf {
    dir.join(id.file())
}

fn part_path(dir: &Path, id: ModelId) -> PathBuf {
    dir.join(format!("{}.part", id.file()))
}

/// A model is ready when its file is there at the right size; the hash was checked before the
/// file got its name, so it isn't re-read (190 MB) on every look.
pub fn is_ready(dir: &Path, id: ModelId) -> bool {
    std::fs::metadata(path(dir, id)).is_ok_and(|m| m.len() == id.size())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelInfo {
    pub id: ModelId,
    pub label: &'static str,
    pub size_bytes: u64,
    pub ready: bool,
}

pub fn list(dir: &Path) -> Vec<ModelInfo> {
    ModelId::ALL
        .into_iter()
        .map(|id| ModelInfo {
            id,
            label: id.label(),
            size_bytes: id.size(),
            ready: is_ready(dir, id),
        })
        .collect()
}

/// A file to fetch, with the size and SHA-256 it must have (models here, the voice packages in
/// `tts.rs`).
#[derive(Debug, Clone, PartialEq)]
pub struct Expected {
    pub url: String,
    pub file: String,
    pub size: u64,
    pub sha256: &'static str,
    /// Named in the "couldn't reach" message.
    pub host: &'static str,
}

impl Expected {
    pub fn model(id: ModelId) -> Self {
        Self {
            url: format!("{SOURCE}/{}", id.file()),
            file: id.file().to_string(),
            size: id.size(),
            sha256: id.sha256(),
            host: "huggingface.co",
        }
    }
}

/// Writes the download to `<file>.part`, hashing as it goes; only a file with the right size and
/// hash is renamed into place, so a broken download is never used.
pub struct PartFile {
    path: PathBuf,
    target: PathBuf,
    size: u64,
    sha256: &'static str,
    file: std::fs::File,
    hasher: Sha256,
    written: u64,
}

impl PartFile {
    /// Starts from scratch: an earlier, interrupted `.part` is replaced.
    pub fn create(dir: &Path, expected: &Expected) -> Result<Self, VoiceError> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{}.part", expected.file));
        let file = std::fs::File::create(&path)?;
        Ok(Self {
            path,
            target: dir.join(&expected.file),
            size: expected.size,
            sha256: expected.sha256,
            file,
            hasher: Sha256::new(),
            written: 0,
        })
    }

    pub fn write(&mut self, chunk: &[u8]) -> Result<u64, VoiceError> {
        self.file.write_all(chunk)?;
        self.hasher.update(chunk);
        self.written += chunk.len() as u64;
        Ok(self.written)
    }

    /// Checks size and hash, then renames; on any mismatch the `.part` is removed.
    pub fn finish(mut self) -> Result<PathBuf, VoiceError> {
        let result = self.check().and_then(|()| {
            self.file.flush()?;
            self.file.sync_all()?;
            Ok(())
        });
        drop(self.file);
        match result {
            Ok(()) => {
                std::fs::rename(&self.path, &self.target)?;
                Ok(self.target)
            }
            Err(err) => {
                let _ = std::fs::remove_file(&self.path);
                Err(err)
            }
        }
    }

    fn check(&mut self) -> Result<(), VoiceError> {
        if self.written != self.size {
            return Err(VoiceError::Model(format!(
                "The download stopped early ({} of {} MB). Try again.",
                self.written / 1_000_000,
                self.size / 1_000_000
            )));
        }
        let hash = hex(&std::mem::take(&mut self.hasher).finalize());
        if hash != self.sha256 {
            return Err(VoiceError::Model(
                "The download doesn't match its published checksum, so it was deleted. Try again."
                    .into(),
            ));
        }
        Ok(())
    }

    /// For a cancelled or failed download.
    pub fn abandon(self) {
        drop(self.file);
        let _ = std::fs::remove_file(&self.path);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Sent as `voice-model` events while a model downloads.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum DownloadProgress {
    Downloading {
        id: ModelId,
        received: u64,
        total: u64,
    },
    Done {
        id: ModelId,
    },
    Failed {
        id: ModelId,
        message: String,
    },
}

pub async fn download(
    dir: &Path,
    id: ModelId,
    mut progress: impl FnMut(DownloadProgress),
) -> Result<(), VoiceError> {
    fetch(dir, &Expected::model(id), |received| {
        progress(DownloadProgress::Downloading {
            id,
            received,
            total: id.size(),
        })
    })
    .await
    .map(|_| ())
}

/// Downloads `expected` into `dir`, calling `progress` with the bytes received about every 2 MB.
pub async fn fetch(
    dir: &Path,
    expected: &Expected,
    mut progress: impl FnMut(u64),
) -> Result<PathBuf, VoiceError> {
    let failed = |err: reqwest::Error| download_failed(err, expected.host);
    let http = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|err| VoiceError::Model(format!("couldn't start the download: {err}")))?;
    let mut response = http
        .get(&expected.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(failed)?;
    let mut part = PartFile::create(dir, expected)?;
    let mut last_reported = 0;
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let received = match part.write(&chunk) {
                    Ok(received) => received,
                    Err(err) => {
                        part.abandon();
                        return Err(err);
                    }
                };
                // About every 2 MB, so the page isn't flooded with events.
                if received - last_reported >= 2_000_000 {
                    last_reported = received;
                    progress(received);
                }
            }
            Ok(None) => break,
            Err(err) => {
                part.abandon();
                return Err(failed(err));
            }
        }
    }
    part.finish()
}

fn download_failed(err: reqwest::Error, host: &str) -> VoiceError {
    VoiceError::Model(if err.is_connect() || err.is_timeout() {
        format!("Couldn't reach {host} for the download. Are you online?")
    } else {
        format!("The download failed: {err}")
    })
}

pub fn delete(dir: &Path, id: ModelId) -> Result<(), VoiceError> {
    for file in [path(dir, id), part_path(dir, id)] {
        match std::fs::remove_file(&file) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kairos-voice-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_download_with_the_wrong_bytes_leaves_no_model_and_no_part_file() {
        let dir = temp_dir("wrong");
        let mut part = PartFile::create(&dir, &Expected::model(ModelId::Base)).unwrap();
        part.write(b"not a model").unwrap();
        let err = part.finish().unwrap_err();
        assert!(err.to_string().contains("stopped early"), "{err}");
        assert!(!path(&dir, ModelId::Base).exists());
        assert!(!part_path(&dir, ModelId::Base).exists());
        assert!(!is_ready(&dir, ModelId::Base));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_right_size_with_the_wrong_hash_is_refused_too() {
        let dir = temp_dir("hash");
        let mut part = PartFile::create(&dir, &Expected::model(ModelId::Base)).unwrap();
        // Zeros of the right length: the size check passes, the hash check must not.
        let block = vec![0u8; 1 << 20];
        let mut left = ModelId::Base.size();
        while left > 0 {
            let n = left.min(block.len() as u64) as usize;
            part.write(&block[..n]).unwrap();
            left -= n as u64;
        }
        let err = part.finish().unwrap_err();
        assert!(err.to_string().contains("checksum"), "{err}");
        assert!(!path(&dir, ModelId::Base).exists());
        assert!(!part_path(&dir, ModelId::Base).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_interrupted_download_starts_again_from_scratch() {
        let dir = temp_dir("restart");
        let mut first = PartFile::create(&dir, &Expected::model(ModelId::Small)).unwrap();
        first.write(b"half of something").unwrap();
        drop(first);
        let mut second = PartFile::create(&dir, &Expected::model(ModelId::Small)).unwrap();
        assert_eq!(second.write(b"abc").unwrap(), 3);
        second.abandon();
        assert!(!part_path(&dir, ModelId::Small).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_list_says_which_models_are_ready() {
        let dir = temp_dir("list");
        std::fs::create_dir_all(&dir).unwrap();
        // A file of the wrong size (say, someone else's) doesn't count as ready.
        std::fs::write(path(&dir, ModelId::Small), b"short").unwrap();
        let list = list(&dir);
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|m| !m.ready));
        assert_eq!(list[0].id, ModelId::Small);
        delete(&dir, ModelId::Small).unwrap();
        delete(&dir, ModelId::Small).unwrap();
        assert!(!path(&dir, ModelId::Small).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Downloads the real Small model (190 MB) into %APPDATA%\kairos\models (M9 Step 2).
    #[test]
    #[ignore]
    fn the_small_model_downloads_and_checks_out() {
        let dir = default_dir().unwrap();
        let mut events = 0;
        tauri::async_runtime::block_on(download(&dir, ModelId::Small, |_| events += 1)).unwrap();
        assert!(is_ready(&dir, ModelId::Small));
        assert!(events > 10, "progress was reported");
    }

    #[test]
    fn model_ids_survive_the_settings_round_trip() {
        for id in ModelId::ALL {
            assert_eq!(ModelId::parse(id.as_str()), Some(id));
        }
        assert_eq!(ModelId::parse("large"), None);
    }

    #[test]
    fn hashes_are_lowercase_hex() {
        assert_eq!(hex(&[0x0a, 0xff]), "0aff");
        for id in ModelId::ALL {
            assert_eq!(id.sha256().len(), 64);
        }
    }
}
