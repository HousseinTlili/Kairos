//! Listing wallpaper candidates in a folder, and next/previous navigation.
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Formats Windows can set as a wallpaper without extra codecs.
const EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp"];
/// Keep the grid and IPC payload bounded.
pub const MAX_IMAGES: usize = 300;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Image {
    pub path: String,
    pub name: String,
}

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Images directly inside `dir` (not recursive), sorted by name, capped at MAX_IMAGES.
pub fn list(dir: &Path) -> Result<Vec<Image>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read '{}': {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_image(p))
        .collect();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
    files.truncate(MAX_IMAGES);
    Ok(files
        .into_iter()
        .map(|p| Image {
            name: p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: p.to_string_lossy().into_owned(),
        })
        .collect())
}

/// Windows paths are case-insensitive and may use either slash.
pub fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('/', "\\").to_lowercase();
    norm(a) == norm(b)
}

pub fn position_of(images: &[Image], current: &str) -> Option<usize> {
    images.iter().position(|i| same_path(&i.path, current))
}

/// Index to apply for next (+1) / previous (-1), wrapping. If the current wallpaper is not in
/// the folder, next starts at the first image and previous at the last.
pub fn step(current: Option<usize>, len: usize, forward: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match (current, forward) {
        (Some(i), true) => (i + 1) % len,
        (Some(i), false) => (i + len - 1) % len,
        (None, true) => 0,
        (None, false) => len - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imgs(n: usize) -> Vec<Image> {
        (0..n)
            .map(|i| Image {
                path: format!("C:\\w\\{i}.jpg"),
                name: i.to_string(),
            })
            .collect()
    }

    #[test]
    fn step_wraps_both_ways() {
        assert_eq!(step(Some(2), 3, true), Some(0));
        assert_eq!(step(Some(0), 3, false), Some(2));
        assert_eq!(step(Some(1), 3, true), Some(2));
        assert_eq!(step(Some(1), 3, false), Some(0));
    }

    #[test]
    fn step_from_unknown_current_and_empty_folder() {
        assert_eq!(step(None, 4, true), Some(0));
        assert_eq!(step(None, 4, false), Some(3));
        assert_eq!(step(None, 0, true), None);
        assert_eq!(step(Some(0), 1, true), Some(0));
    }

    #[test]
    fn position_ignores_case_and_slash_style() {
        let v = imgs(3);
        assert_eq!(position_of(&v, "c:/W/1.JPG"), Some(1));
        assert_eq!(position_of(&v, "C:\\elsewhere\\1.jpg"), None);
    }

    #[test]
    fn only_settable_formats_count() {
        assert!(is_image(Path::new("a.JPG")));
        assert!(is_image(Path::new("a.png")));
        assert!(!is_image(Path::new("a.webp")));
        assert!(!is_image(Path::new("a.txt")));
        assert!(!is_image(Path::new("noext")));
    }

    #[test]
    fn list_filters_sorts_and_reports_missing_dir() {
        let dir = std::env::temp_dir().join(format!("elissa-wp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["b.png", "A.jpg", "notes.txt", "c.webp"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        std::fs::create_dir_all(dir.join("sub.jpg")).unwrap(); // a directory named like an image
        let names: Vec<_> = list(&dir).unwrap().into_iter().map(|i| i.name).collect();
        assert_eq!(names, vec!["A", "b"]);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(list(&dir).is_err());
    }
}
