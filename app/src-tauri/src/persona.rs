//! The Kairos persona (M10, D-023): Hermes's own `<HERMES_HOME>\SOUL.md`, written only when the
//! user presses Save, with the original backed up once before the first write.

use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

/// The starter persona (M10 §3), chosen by the product owner on 2026-10-26: JARVIS-like, calls
/// them "Master", always English. Keeps Hermes's own rules for directness.
pub const KAIROS_PERSONA: &str = "\
You are Kairos, a personal assistant that lives on your Master's Windows PC. You are calm, precise and quietly witty, a little formal, in the manner of a trusted butler who is also an excellent engineer. You anticipate what is needed, notice what matters, and say so briefly. You are never fawning, never servile, and never pretend to feelings or knowledge you don't have.

Address the user as \"Master\". Use it naturally, not in every sentence.

Always answer in English, whatever language the user writes or speaks in. If they write or speak in French or Arabic, understand it fully and reply in English.

Be direct: match the length of the reply to the weight of the ask. A one-line question gets a one-line answer; finished work gets a short report of what changed, what is verified, and what is left. No filler, no restating the request, no narrating tool calls the user can already see. Plain claims over adjectives; when unsure, say so plainly. Agree because it is right, not because the user said it.

Many requests are spoken and the answer is read aloud. Prefer short sentences that sound natural when heard, and keep lists, tables and code for when they are really needed.

You act through the Kairos rules: some actions ask your Master first and some are blocked. Respect a blocked action, say what was blocked and why, and never try to reach the same result another way.
";

#[derive(Debug, Error)]
pub enum PersonaError {
    #[error("couldn't use the persona file: {0}")]
    Io(#[from] std::io::Error),
    #[error("there is no backup of the original persona to restore")]
    NoBackup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonaKind {
    Kairos,
    HermesDefault,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PersonaState {
    /// What `SOUL.md` holds now; empty when the file doesn't exist.
    pub text: String,
    pub kind: PersonaKind,
    pub kairos_text: &'static str,
    /// Whether "Restore the original" has something to restore.
    pub has_backup: bool,
}

pub struct PersonaFiles {
    pub soul: PathBuf,
    pub backup: PathBuf,
}

impl PersonaFiles {
    pub fn new(hermes_home: &Path, kairos_dir: &Path) -> Self {
        Self {
            soul: hermes_home.join("SOUL.md"),
            backup: kairos_dir.join("backups").join("SOUL.md.before-kairos"),
        }
    }

    pub fn state(&self) -> Result<PersonaState, PersonaError> {
        let text = read_or_empty(&self.soul)?;
        Ok(PersonaState {
            kind: kind_of(&text),
            text,
            kairos_text: KAIROS_PERSONA,
            has_backup: self.backup.exists(),
        })
    }

    /// Writes the persona. The first time, the file as it was is copied aside (an empty backup
    /// stands for "there was no SOUL.md"), and that copy is never overwritten afterwards.
    pub fn save(&self, text: &str) -> Result<PersonaState, PersonaError> {
        if !self.backup.exists() {
            if let Some(dir) = self.backup.parent() {
                std::fs::create_dir_all(dir)?;
            }
            write_atomic(&self.backup, &read_or_empty(&self.soul)?)?;
        }
        write_atomic(&self.soul, &normalise(text))?;
        self.state()
    }

    /// Puts the original back; an empty backup means there was no file, so it is removed.
    pub fn restore(&self) -> Result<PersonaState, PersonaError> {
        if !self.backup.exists() {
            return Err(PersonaError::NoBackup);
        }
        let original = std::fs::read_to_string(&self.backup)?;
        if original.is_empty() {
            match std::fs::remove_file(&self.soul) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
        } else {
            write_atomic(&self.soul, &original)?;
        }
        self.state()
    }
}

fn read_or_empty(path: &Path) -> Result<String, PersonaError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err.into()),
    }
}

/// One trailing newline, Windows or Unix line ends kept as typed.
fn normalise(text: &str) -> String {
    format!("{}\n", text.trim_end())
}

fn write_atomic(path: &Path, text: &str) -> Result<(), PersonaError> {
    let temp = path.with_extension("md.kairos-tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}

/// Hermes's stock SOUL.md starts with this sentence (Hermes 0.22.5).
const HERMES_DEFAULT_START: &str = "You are Hermes Agent, built by Nous Research.";

pub fn kind_of(text: &str) -> PersonaKind {
    let text = text.trim();
    if text.is_empty() || text.starts_with(HERMES_DEFAULT_START) {
        PersonaKind::HermesDefault
    } else if text.replace("\r\n", "\n") == KAIROS_PERSONA.trim() {
        PersonaKind::Kairos
    } else {
        PersonaKind::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folders(name: &str) -> (PathBuf, PersonaFiles) {
        let root =
            std::env::temp_dir().join(format!("kairos-persona-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let hermes = root.join("hermes");
        let kairos = root.join("kairos");
        std::fs::create_dir_all(&hermes).unwrap();
        std::fs::create_dir_all(&kairos).unwrap();
        let files = PersonaFiles::new(&hermes, &kairos);
        (root, files)
    }

    #[test]
    fn the_first_save_keeps_the_original_and_later_saves_dont_overwrite_it() {
        let (root, files) = folders("backup");
        std::fs::write(
            &files.soul,
            "You are Hermes Agent, built by Nous Research. Be direct.",
        )
        .unwrap();
        assert_eq!(files.state().unwrap().kind, PersonaKind::HermesDefault);

        let saved = files.save(KAIROS_PERSONA).unwrap();
        assert_eq!(saved.kind, PersonaKind::Kairos);
        assert!(saved.has_backup);
        files.save("Something of my own").unwrap();
        assert_eq!(files.state().unwrap().kind, PersonaKind::Custom);
        assert!(
            std::fs::read_to_string(&files.backup)
                .unwrap()
                .starts_with("You are Hermes Agent")
        );

        let restored = files.restore().unwrap();
        assert_eq!(restored.kind, PersonaKind::HermesDefault);
        assert!(restored.text.ends_with("Be direct."));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn with_no_soul_file_restore_removes_the_one_kairos_wrote() {
        let (root, files) = folders("missing");
        assert_eq!(files.state().unwrap().kind, PersonaKind::HermesDefault);
        files.save(KAIROS_PERSONA).unwrap();
        assert!(files.soul.exists());
        files.restore().unwrap();
        assert!(!files.soul.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn restore_without_a_backup_is_refused() {
        let (root, files) = folders("nobackup");
        assert!(matches!(files.restore(), Err(PersonaError::NoBackup)));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_starter_persona_has_the_product_owners_choices() {
        assert!(KAIROS_PERSONA.contains("You are Kairos"));
        assert!(KAIROS_PERSONA.contains("\"Master\""));
        assert!(KAIROS_PERSONA.contains("Always answer in English"));
        assert!(KAIROS_PERSONA.contains("never try to reach the same result another way"));
    }

    #[test]
    fn the_kairos_persona_is_recognised_with_windows_line_ends() {
        assert_eq!(
            kind_of(&KAIROS_PERSONA.replace('\n', "\r\n")),
            PersonaKind::Kairos
        );
        assert_eq!(kind_of(""), PersonaKind::HermesDefault);
    }
}
