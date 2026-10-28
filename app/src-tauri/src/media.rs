//! Pictures in answers (M13 Step 4, D-028). Hermes attaches a file to an answer with a
//! `MEDIA:<path>` line; the finished answer carries the picture itself, but a conversation opened
//! later only has the path. Kairos reads such a path only inside Hermes's own image folders, so a
//! line the model wrote can't make it show any other file. Saving goes to the user's Downloads.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// As Hermes's own inline limit (`_MEDIA_DATA_URL_MAX_BYTES`).
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;
const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];
/// What Kairos itself saves: the same pictures, plus the model's SVG sketches.
const SAVE_EXTENSIONS: [&str; 5] = ["png", "jpg", "gif", "webp", "svg"];
/// Hermes's folders that hold pictures: generated images, and its cache (screenshots, the browser
/// workspace).
const FOLDERS: [&str; 2] = ["image_cache", "cache"];

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("That picture isn't one of Hermes's: Kairos only shows images from Hermes's folders.")]
    NotHermes,
    #[error("That file isn't a picture.")]
    NotPicture,
    #[error("That picture is too large to show (over 5 MB).")]
    TooLarge,
    #[error("The picture is no longer there.")]
    Missing,
    #[error("Couldn't find your Downloads folder.")]
    NoDownloads,
    #[error("Couldn't save the picture: {0}")]
    Io(#[from] std::io::Error),
}

/// The picture at `path` if it is an image file of at most 5 MB inside Hermes's image folders
/// (after resolving `..` and links).
pub fn allowed(path: &str, hermes_home: &Path) -> Result<PathBuf, MediaError> {
    let file = Path::new(path.trim());
    let extension = file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !EXTENSIONS.contains(&extension.as_str()) {
        return Err(MediaError::NotPicture);
    }
    let real = file.canonicalize().map_err(|_| MediaError::Missing)?;
    let inside = FOLDERS.iter().any(|folder| {
        hermes_home
            .join(folder)
            .canonicalize()
            .is_ok_and(|root| real.starts_with(root))
    });
    if !inside {
        return Err(MediaError::NotHermes);
    }
    let meta = std::fs::metadata(&real).map_err(|_| MediaError::Missing)?;
    if !meta.is_file() {
        return Err(MediaError::NotPicture);
    }
    if meta.len() > MAX_BYTES {
        return Err(MediaError::TooLarge);
    }
    Ok(real)
}

pub fn read(path: &str, hermes_home: &Path) -> Result<Vec<u8>, MediaError> {
    Ok(std::fs::read(allowed(path, hermes_home)?)?)
}

/// `%USERPROFILE%\Downloads`.
pub fn downloads() -> Result<PathBuf, MediaError> {
    std::env::var_os("USERPROFILE")
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join("Downloads"))
        .filter(|dir| dir.is_dir())
        .ok_or(MediaError::NoDownloads)
}

/// A file name in `dir` that doesn't exist yet: `kairos-image.png`, `kairos-image (2).png`…
/// Only the name's letters, digits, `-` and `_` are kept, and only a picture extension.
pub fn target(dir: &Path, name: &str) -> Result<PathBuf, MediaError> {
    let name = Path::new(name);
    let extension = name
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|e| SAVE_EXTENSIONS.contains(&e.as_str()))
        .ok_or(MediaError::NotPicture)?;
    let stem: String = name
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(60)
        .collect();
    let stem = if stem.is_empty() {
        "kairos-image".into()
    } else {
        stem
    };
    let mut candidate = dir.join(format!("{stem}.{extension}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({n}).{extension}"));
        n += 1;
    }
    Ok(candidate)
}

pub fn save(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf, MediaError> {
    if bytes.len() as u64 > 4 * MAX_BYTES {
        return Err(MediaError::TooLarge);
    }
    let path = target(dir, name)?;
    std::fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Home {
        dir: PathBuf,
    }

    impl Home {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("kairos-media-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("hermes/image_cache")).unwrap();
            std::fs::create_dir_all(dir.join("hermes/cache/screenshots")).unwrap();
            std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
            Self { dir }
        }
        fn hermes(&self) -> PathBuf {
            self.dir.join("hermes")
        }
        fn file(&self, rel: &str, bytes: usize) -> String {
            let path = self.dir.join(rel);
            std::fs::write(&path, vec![0u8; bytes]).unwrap();
            path.display().to_string()
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn a_picture_in_hermess_folders_is_shown() {
        let home = Home::new("ok");
        let generated = home.file("hermes/image_cache/apple.png", 10);
        let shot = home.file("hermes/cache/screenshots/page.JPG", 10);
        assert_eq!(read(&generated, &home.hermes()).unwrap().len(), 10);
        assert!(allowed(&shot, &home.hermes()).is_ok());
    }

    #[test]
    fn a_picture_anywhere_else_is_refused() {
        let home = Home::new("outside");
        let outside = home.file("elsewhere/secret.png", 10);
        assert!(matches!(
            allowed(&outside, &home.hermes()),
            Err(MediaError::NotHermes)
        ));
        let climbing = home
            .hermes()
            .join("image_cache/../../elsewhere/secret.png")
            .display()
            .to_string();
        assert!(matches!(
            allowed(&climbing, &home.hermes()),
            Err(MediaError::NotHermes)
        ));
    }

    #[test]
    fn only_pictures_of_at_most_5_mb() {
        let home = Home::new("kinds");
        let notes = home.file("hermes/image_cache/notes.txt", 10);
        assert!(matches!(
            allowed(&notes, &home.hermes()),
            Err(MediaError::NotPicture)
        ));
        let huge = home.file("hermes/image_cache/huge.png", 6 * 1024 * 1024);
        assert!(matches!(
            allowed(&huge, &home.hermes()),
            Err(MediaError::TooLarge)
        ));
        let gone = home
            .hermes()
            .join("image_cache/gone.png")
            .display()
            .to_string();
        assert!(matches!(
            allowed(&gone, &home.hermes()),
            Err(MediaError::Missing)
        ));
    }

    #[test]
    fn saving_never_overwrites_and_keeps_a_plain_name() {
        let home = Home::new("save");
        let dir = home.dir.join("elsewhere");
        let first = save(&dir, "apple.png", b"one").unwrap();
        let second = save(&dir, "apple.png", b"two").unwrap();
        assert_eq!(first.file_name().unwrap(), "apple.png");
        assert_eq!(second.file_name().unwrap(), "apple (2).png");
        assert_eq!(std::fs::read(first).unwrap(), b"one");
        let odd = target(&dir, "..\\..\\evil name.svg").unwrap();
        assert_eq!(odd.file_name().unwrap(), "evilname.svg");
        assert!(matches!(
            target(&dir, "run.exe"),
            Err(MediaError::NotPicture)
        ));
    }
}
