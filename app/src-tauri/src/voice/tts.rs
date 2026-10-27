//! The natural voice (M9 Step 8, D-025): Kokoro through sherpa-onnx's C library, downloaded on the
//! user's click and kept loaded, so each sentence costs only its synthesis (about a quarter of its
//! spoken length on the product owner's PC).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::VoiceError;
use super::models::{Expected, fetch};

const RELEASES: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";

/// One downloadable archive and the file that shows it is unpacked.
pub struct Package {
    pub expected: fn() -> Expected,
    /// The folder the archive unpacks to.
    pub folder: &'static str,
    /// Relative to `folder`.
    pub marker: &'static str,
}

pub const RUNTIME: Package = Package {
    expected: || Expected {
        url: format!("{RELEASES}/v1.13.8/sherpa-onnx-v1.13.8-win-x64-shared-MT-Release.tar.bz2"),
        file: "sherpa-onnx-v1.13.8-win-x64-shared-MT-Release.tar.bz2".into(),
        size: 24_805_859,
        sha256: "6dffdc715a4465b989446a6105265d2cb345e7101591a17d35534b6758f6e8df",
        host: "github.com",
    },
    folder: "sherpa-onnx-v1.13.8-win-x64-shared-MT-Release",
    marker: "lib/sherpa-onnx-c-api.dll",
};

pub const KOKORO: Package = Package {
    expected: || Expected {
        url: format!("{RELEASES}/tts-models/kokoro-en-v0_19.tar.bz2"),
        file: "kokoro-en-v0_19.tar.bz2".into(),
        size: 319_625_534,
        sha256: "912804855a04745fa77a30be545b3f9a5d15c4d66db00b88cbcd4921df605ac7",
        host: "github.com",
    },
    folder: "kokoro-en-v0_19",
    marker: "model.onnx",
};

pub const PACKAGES: [&Package; 2] = [&RUNTIME, &KOKORO];

pub fn total_size() -> u64 {
    PACKAGES.iter().map(|p| (p.expected)().size).sum()
}

/// Kokoro v0.19's eleven voices, in its speaker-id order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    Heart,
    Bella,
    Nicole,
    Sarah,
    Sky,
    Adam,
    Michael,
    Emma,
    Isabella,
    George,
    Lewis,
}

impl Speaker {
    pub const ALL: [Speaker; 11] = [
        Speaker::Heart,
        Speaker::Bella,
        Speaker::Nicole,
        Speaker::Sarah,
        Speaker::Sky,
        Speaker::Adam,
        Speaker::Michael,
        Speaker::Emma,
        Speaker::Isabella,
        Speaker::George,
        Speaker::Lewis,
    ];

    pub fn id(self) -> i32 {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(9) as i32
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Speaker::Heart => "heart",
            Speaker::Bella => "bella",
            Speaker::Nicole => "nicole",
            Speaker::Sarah => "sarah",
            Speaker::Sky => "sky",
            Speaker::Adam => "adam",
            Speaker::Michael => "michael",
            Speaker::Emma => "emma",
            Speaker::Isabella => "isabella",
            Speaker::George => "george",
            Speaker::Lewis => "lewis",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == text)
    }
}

pub fn default_dir() -> Result<PathBuf, VoiceError> {
    std::env::var_os("APPDATA")
        .map(|appdata| PathBuf::from(appdata).join("kairos").join("voices"))
        .ok_or_else(|| VoiceError::Model("APPDATA is not set".into()))
}

pub fn is_installed(dir: &Path) -> bool {
    is_installed_all(dir, &PACKAGES)
}

pub fn is_installed_all(dir: &Path, packages: &[&Package]) -> bool {
    packages
        .iter()
        .all(|p| dir.join(p.folder).join(p.marker).is_file())
}

/// Downloads and unpacks whatever is missing; `progress` gets (bytes so far, total bytes).
pub async fn install(dir: &Path, progress: impl FnMut(u64, u64)) -> Result<(), VoiceError> {
    install_packages(dir, &PACKAGES, progress).await
}

/// Downloads and unpacks whichever of `packages` is missing (the wake word shares the runtime).
pub async fn install_packages(
    dir: &Path,
    packages: &[&Package],
    mut progress: impl FnMut(u64, u64),
) -> Result<(), VoiceError> {
    let total: u64 = packages
        .iter()
        .filter(|p| !dir.join(p.folder).join(p.marker).is_file())
        .map(|p| (p.expected)().size)
        .sum();
    let mut done = 0;
    for package in packages.iter().copied() {
        let expected = (package.expected)();
        if !dir.join(package.folder).join(package.marker).is_file() {
            let base = done;
            let archive =
                fetch(dir, &expected, |received| progress(base + received, total)).await?;
            let unpacked = unpack(&archive, dir);
            let _ = std::fs::remove_file(&archive);
            unpacked?;
            if !dir.join(package.folder).join(package.marker).is_file() {
                return Err(VoiceError::Model(format!(
                    "{} unpacked, but {} is missing from it.",
                    expected.file, package.marker
                )));
            }
            done += expected.size;
        }
        progress(done, total);
    }
    Ok(())
}

/// Windows's own `tar.exe` (bsdtar, with bzip2), so unpacking needs no new crate.
fn unpack(archive: &Path, into: &Path) -> Result<(), VoiceError> {
    let tar = std::env::var_os("SystemRoot")
        .map(|root| PathBuf::from(root).join("System32").join("tar.exe"))
        .filter(|p| p.is_file())
        .ok_or_else(|| {
            VoiceError::Model("Windows's tar.exe wasn't found to unpack the voice.".into())
        })?;
    let mut command = std::process::Command::new(tar);
    command.arg("-xjf").arg(archive).arg("-C").arg(into);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(VoiceError::Model(format!(
            "Unpacking the voice failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

pub fn remove(dir: &Path) -> Result<(), VoiceError> {
    for package in PACKAGES {
        match std::fs::remove_dir_all(dir.join(package.folder)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }
    Ok(())
}

/// 16-bit PCM WAV, mono, for the page to play.
pub fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// One Kokoro engine, loaded on first use and kept.
#[derive(Default)]
pub struct Voice {
    loaded: Mutex<Option<(ffi::Engine, std::time::Instant)>>,
}

/// Loading takes about 6 s on the product owner's PC (a sentence then takes under 1 s), so it is
/// done while the user is still speaking (`warm`), and undone after this long unused.
pub const IDLE_UNLOAD: std::time::Duration = std::time::Duration::from_secs(15 * 60);

impl Voice {
    /// Blocking: call it from a worker thread. Returns (samples, sample rate).
    pub fn speak(
        &self,
        dir: &Path,
        text: &str,
        speaker: Speaker,
        speed: f32,
    ) -> Result<(Vec<f32>, u32), VoiceError> {
        let text = text.trim();
        if text.is_empty() {
            return Ok((Vec::new(), 24_000));
        }
        let mut guard = self.loaded.lock().map_err(|_| VoiceError::Busy)?;
        let (engine, last_used) = Self::load_into(&mut guard, dir)?;
        *last_used = std::time::Instant::now();
        engine.generate(text, speaker.id(), speed)
    }

    /// Loads the engine now, so the first answer doesn't wait for it. Blocking.
    pub fn warm(&self, dir: &Path) -> Result<(), VoiceError> {
        let mut guard = self.loaded.lock().map_err(|_| VoiceError::Busy)?;
        let (_, last_used) = Self::load_into(&mut guard, dir)?;
        *last_used = std::time::Instant::now();
        Ok(())
    }

    fn load_into<'a>(
        slot: &'a mut Option<(ffi::Engine, std::time::Instant)>,
        dir: &Path,
    ) -> Result<&'a mut (ffi::Engine, std::time::Instant), VoiceError> {
        if slot.is_none() {
            if !is_installed(dir) {
                return Err(VoiceError::Model(
                    "The natural voice isn't downloaded: Settings ▸ Voice.".into(),
                ));
            }
            let engine = ffi::Engine::load(
                &dir.join(RUNTIME.folder).join("lib"),
                &dir.join(KOKORO.folder),
            )?;
            *slot = Some((engine, std::time::Instant::now()));
        }
        slot.as_mut().ok_or(VoiceError::Busy)
    }

    /// Frees the engine after `IDLE_UNLOAD` unused; called from a timer.
    pub fn unload_if_idle(&self) {
        if let Ok(mut guard) = self.loaded.try_lock()
            && guard
                .as_ref()
                .is_some_and(|(_, used)| used.elapsed() > IDLE_UNLOAD)
        {
            *guard = None;
        }
    }

    /// Before the files are deleted. The DLL stays mapped (unloading ONNX Runtime isn't safe), but
    /// its engine is freed.
    pub fn unload(&self) {
        if let Ok(mut guard) = self.loaded.lock() {
            *guard = None;
        }
    }
}

#[cfg(windows)]
pub(crate) mod ffi {
    //! The few sherpa-onnx C functions Kairos needs, from `c-api.h` of v1.13.8. The structs must
    //! match that header field for field; zeroed fields mean "default" to sherpa-onnx.

    use std::ffi::{CString, c_char, c_void};
    use std::path::Path;

    use super::VoiceError;

    #[repr(C)]
    struct Vits {
        model: *const c_char,
        lexicon: *const c_char,
        tokens: *const c_char,
        data_dir: *const c_char,
        noise_scale: f32,
        noise_scale_w: f32,
        length_scale: f32,
        dict_dir: *const c_char,
    }

    #[repr(C)]
    struct Matcha {
        acoustic_model: *const c_char,
        vocoder: *const c_char,
        lexicon: *const c_char,
        tokens: *const c_char,
        data_dir: *const c_char,
        noise_scale: f32,
        length_scale: f32,
        dict_dir: *const c_char,
    }

    #[repr(C)]
    struct Kokoro {
        model: *const c_char,
        voices: *const c_char,
        tokens: *const c_char,
        data_dir: *const c_char,
        length_scale: f32,
        dict_dir: *const c_char,
        lexicon: *const c_char,
        lang: *const c_char,
    }

    #[repr(C)]
    struct Kitten {
        model: *const c_char,
        voices: *const c_char,
        tokens: *const c_char,
        data_dir: *const c_char,
        length_scale: f32,
    }

    #[repr(C)]
    struct Zipvoice {
        tokens: *const c_char,
        encoder: *const c_char,
        decoder: *const c_char,
        vocoder: *const c_char,
        data_dir: *const c_char,
        lexicon: *const c_char,
        feat_scale: f32,
        t_shift: f32,
        target_rms: f32,
        guidance_scale: f32,
    }

    #[repr(C)]
    struct Pocket {
        lm_flow: *const c_char,
        lm_main: *const c_char,
        encoder: *const c_char,
        decoder: *const c_char,
        text_conditioner: *const c_char,
        vocab_json: *const c_char,
        token_scores_json: *const c_char,
        voice_embedding_cache_capacity: i32,
    }

    #[repr(C)]
    struct Supertonic {
        duration_predictor: *const c_char,
        text_encoder: *const c_char,
        vector_estimator: *const c_char,
        vocoder: *const c_char,
        tts_json: *const c_char,
        unicode_indexer: *const c_char,
        voice_style: *const c_char,
    }

    #[repr(C)]
    struct ModelConfig {
        vits: Vits,
        num_threads: i32,
        debug: i32,
        provider: *const c_char,
        matcha: Matcha,
        kokoro: Kokoro,
        kitten: Kitten,
        zipvoice: Zipvoice,
        pocket: Pocket,
        supertonic: Supertonic,
    }

    #[repr(C)]
    struct Config {
        model: ModelConfig,
        rule_fsts: *const c_char,
        max_num_sentences: i32,
        rule_fars: *const c_char,
        silence_scale: f32,
    }

    #[repr(C)]
    struct GeneratedAudio {
        samples: *const f32,
        n: i32,
        sample_rate: i32,
    }

    type Create = unsafe extern "C" fn(*const Config) -> *const c_void;
    type Destroy = unsafe extern "C" fn(*const c_void);
    type Generate =
        unsafe extern "C" fn(*const c_void, *const c_char, i32, f32) -> *const GeneratedAudio;
    type DestroyAudio = unsafe extern "C" fn(*const GeneratedAudio);

    pub struct Engine {
        tts: *const c_void,
        destroy: Destroy,
        generate: Generate,
        destroy_audio: DestroyAudio,
    }

    // SAFETY: the engine is only used behind `Voice`'s mutex, one call at a time; sherpa-onnx's
    // offline TTS has no thread affinity.
    unsafe impl Send for Engine {}

    pub(crate) fn c(path: &Path) -> Result<CString, VoiceError> {
        CString::new(path.to_string_lossy().into_owned())
            .map_err(|_| VoiceError::Model("a voice file path contains a NUL".into()))
    }

    pub(crate) fn symbol<T>(module: *mut c_void, name: &str) -> Result<T, VoiceError> {
        use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
        let cname = CString::new(name).map_err(|_| VoiceError::Model("bad symbol".into()))?;
        // SAFETY: `module` is a loaded sherpa-onnx-c-api.dll; the name is NUL-terminated.
        let proc = unsafe { GetProcAddress(module, cname.as_ptr().cast()) }.ok_or_else(|| {
            VoiceError::Model(format!("sherpa-onnx has no {name}: wrong version?"))
        })?;
        // SAFETY: the signatures above are copied from the v1.13.8 header for these names.
        Ok(unsafe { std::mem::transmute_copy(&proc) })
    }

    /// sherpa-onnx-c-api.dll, shared by the voice and the wake word (Windows hands back the same
    /// module when it is already loaded). Never freed: unloading ONNX Runtime isn't safe.
    pub(crate) fn library(lib_dir: &Path) -> Result<*mut c_void, VoiceError> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::LibraryLoader::{
            LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LoadLibraryExW,
        };
        let dll: Vec<u16> = lib_dir
            .join("sherpa-onnx-c-api.dll")
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // Its own folder is searched for onnxruntime.dll; PATH isn't.
        // SAFETY: a NUL-terminated wide path.
        let module = unsafe {
            LoadLibraryExW(
                dll.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        };
        if module.is_null() {
            return Err(VoiceError::Model(format!(
                "couldn't load the voice engine: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(module)
    }

    impl Engine {
        pub fn load(lib_dir: &Path, model_dir: &Path) -> Result<Self, VoiceError> {
            let module = library(lib_dir)?;
            let create: Create = symbol(module, "SherpaOnnxCreateOfflineTts")?;
            let destroy: Destroy = symbol(module, "SherpaOnnxDestroyOfflineTts")?;
            let generate: Generate = symbol(module, "SherpaOnnxOfflineTtsGenerate")?;
            let destroy_audio: DestroyAudio =
                symbol(module, "SherpaOnnxDestroyOfflineTtsGeneratedAudio")?;

            let model = c(&model_dir.join("model.onnx"))?;
            let voices = c(&model_dir.join("voices.bin"))?;
            let tokens = c(&model_dir.join("tokens.txt"))?;
            let data_dir = c(&model_dir.join("espeak-ng-data"))?;
            let provider = CString::new("cpu").map_err(|_| VoiceError::Busy)?;
            // SAFETY: every field is a plain pointer, integer or float; all-zero is sherpa-onnx's
            // "unset", and the fields Kairos uses are filled in below.
            let mut config: Config = unsafe { std::mem::zeroed() };
            config.model.kokoro.model = model.as_ptr();
            config.model.kokoro.voices = voices.as_ptr();
            config.model.kokoro.tokens = tokens.as_ptr();
            config.model.kokoro.data_dir = data_dir.as_ptr();
            config.model.kokoro.length_scale = 1.0;
            // 8 threads measured fastest (M9 Step 8); fewer on smaller PCs.
            config.model.num_threads = std::thread::available_parallelism()
                .map_or(4, |n| n.get().saturating_sub(2))
                .clamp(1, 8) as i32;
            config.model.provider = provider.as_ptr();
            config.max_num_sentences = 1;
            // SAFETY: `config` and the strings it points to outlive the call; sherpa-onnx copies them.
            let tts = unsafe { create(&config) };
            if tts.is_null() {
                return Err(VoiceError::Model(
                    "the voice engine didn't start (are the voice files complete?)".into(),
                ));
            }
            Ok(Self {
                tts,
                destroy,
                generate,
                destroy_audio,
            })
        }

        pub fn generate(
            &self,
            text: &str,
            speaker: i32,
            speed: f32,
        ) -> Result<(Vec<f32>, u32), VoiceError> {
            let text = CString::new(text.replace('\0', " ")).map_err(|_| VoiceError::Busy)?;
            // SAFETY: `tts` is alive; the text is NUL-terminated and outlives the call.
            let audio = unsafe { (self.generate)(self.tts, text.as_ptr(), speaker, speed) };
            if audio.is_null() {
                return Err(VoiceError::Audio(
                    "the voice engine produced nothing".into(),
                ));
            }
            // SAFETY: sherpa-onnx returned `n` samples at `samples`; copied before it is freed.
            let result = unsafe {
                let a = &*audio;
                let samples = if a.samples.is_null() || a.n <= 0 {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(a.samples, a.n as usize).to_vec()
                };
                let rate = a.sample_rate.max(1) as u32;
                (self.destroy_audio)(audio);
                (samples, rate)
            };
            Ok(result)
        }
    }

    impl Drop for Engine {
        fn drop(&mut self) {
            // SAFETY: created by SherpaOnnxCreateOfflineTts and freed once.
            unsafe { (self.destroy)(self.tts) };
        }
    }
}

#[cfg(not(windows))]
mod ffi {
    use std::path::Path;

    use super::VoiceError;

    pub struct Engine;

    impl Engine {
        pub fn load(_lib_dir: &Path, _model_dir: &Path) -> Result<Self, VoiceError> {
            Err(VoiceError::Model("the natural voice needs Windows".into()))
        }

        pub fn generate(
            &self,
            _text: &str,
            _speaker: i32,
            _speed: f32,
        ) -> Result<(Vec<f32>, u32), VoiceError> {
            Err(VoiceError::Model("the natural voice needs Windows".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn george_is_speaker_nine_and_names_round_trip() {
        assert_eq!(Speaker::George.id(), 9);
        assert_eq!(Speaker::Lewis.id(), 10);
        for speaker in Speaker::ALL {
            assert_eq!(Speaker::parse(speaker.as_str()), Some(speaker));
        }
        assert_eq!(Speaker::parse("jarvis"), None);
    }

    #[test]
    fn the_packages_are_pinned_to_their_published_hashes() {
        for package in PACKAGES {
            let expected = (package.expected)();
            assert_eq!(expected.sha256.len(), 64);
            assert!(
                expected
                    .url
                    .starts_with("https://github.com/k2-fsa/sherpa-onnx/")
            );
            assert!(expected.url.ends_with(&expected.file));
        }
        assert_eq!(total_size(), 24_805_859 + 319_625_534);
    }

    #[test]
    fn nothing_is_installed_in_an_empty_folder() {
        let dir = std::env::temp_dir().join(format!("kairos-tts-empty-{}", std::process::id()));
        assert!(!is_installed(&dir));
        assert!(remove(&dir).is_ok(), "removing what isn't there is fine");
    }

    #[test]
    fn the_wav_header_says_16_bit_mono_at_the_given_rate() {
        let bytes = wav(&[0.0, 1.0, -1.0, 2.0], 24_000);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1, "mono");
        assert_eq!(
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            24_000
        );
        assert_eq!(
            u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]),
            8
        );
        assert_eq!(i16::from_le_bytes([bytes[46], bytes[47]]), 32767);
        assert_eq!(i16::from_le_bytes([bytes[50], bytes[51]]), 32767, "clipped");
    }

    /// Needs the voice in %APPDATA%\kairos\voices (Settings ▸ Voice ▸ Download), M9 Step 8.
    #[test]
    #[ignore]
    fn the_natural_voice_installs() {
        let dir = default_dir().unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let mut last = (0, 0);
        tauri::async_runtime::block_on(install(&dir, |received, total| last = (received, total)))
            .unwrap();
        assert!(is_installed(&dir));
        assert_eq!(last.0, last.1, "progress reached the total");
    }

    #[test]
    #[ignore]
    fn george_speaks_a_sentence_quickly() {
        let dir = default_dir().unwrap();
        let voice = Voice::default();
        let started = std::time::Instant::now();
        let (samples, rate) = voice
            .speak(
                &dir,
                "Good evening, Master. The build finished without errors.",
                Speaker::George,
                1.0,
            )
            .unwrap();
        let first = started.elapsed();
        let seconds = samples.len() as f32 / rate as f32;
        eprintln!(
            "first sentence (with loading): {first:?} for {seconds:.1} s of speech at {rate} Hz"
        );
        assert!(seconds > 1.5, "{seconds}");
        let again = std::time::Instant::now();
        voice
            .speak(&dir, "Your notes are ready.", Speaker::George, 1.0)
            .unwrap();
        eprintln!("next sentence (kept loaded): {:?}", again.elapsed());
        assert!(again.elapsed() < std::time::Duration::from_secs(3));
    }
}
