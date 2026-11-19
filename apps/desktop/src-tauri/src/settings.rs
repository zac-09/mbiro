//! What the user can change in Settings, and its safe defaults.
//!
//! Stored as one JSON value beside the job list. Anything missing or out of
//! range falls back to the default, so an old or hand-edited value never
//! stops the app from starting.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::engine::{DEFAULT_CONNECTIONS, MAX_CONNECTIONS};

/// The most downloads that may run at once.
pub const MAX_CONCURRENT: usize = 10;

/// A kind of file, recognised by how its name ends, and where to save it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub name: String,
    /// Lower case, without the dot. `tar.gz` is allowed.
    pub extensions: Vec<String>,
    /// `None` saves into the default folder.
    #[serde(default)]
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `None` is the Mac's Downloads folder.
    pub default_dir: Option<PathBuf>,
    pub max_concurrent: usize,
    pub default_connections: u8,
    /// Total speed cap for file downloads, in KB/s. `None` is no cap.
    pub speed_limit_kbps: Option<u64>,
    pub watch_clipboard: bool,
    pub notify_on_finish: bool,
    pub categories: Vec<Category>,
}

fn category(name: &str, extensions: &[&str]) -> Category {
    Category {
        name: name.to_owned(),
        extensions: extensions.iter().map(|ext| (*ext).to_owned()).collect(),
        dir: None,
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_dir: None,
            max_concurrent: 3,
            default_connections: DEFAULT_CONNECTIONS,
            speed_limit_kbps: None,
            watch_clipboard: true,
            notify_on_finish: true,
            categories: vec![
                category(
                    "Archives",
                    &["zip", "rar", "7z", "tar", "gz", "tar.gz", "bz2", "xz"],
                ),
                category("Apps", &["dmg", "pkg", "app", "exe", "msi", "deb", "iso"]),
                category(
                    "Documents",
                    &[
                        "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "epub",
                    ],
                ),
                category("Video", &["mp4", "mkv", "mov", "webm", "avi", "m4v"]),
                category(
                    "Audio",
                    &["mp3", "m4a", "flac", "wav", "aac", "ogg", "opus"],
                ),
            ],
        }
    }
}

/// A file type as typed, tidied: `.ZIP` becomes `zip`. Nothing if it is not one.
fn clean_extension(text: &str) -> Option<String> {
    let ext = text.trim().trim_start_matches('.').to_ascii_lowercase();
    let plain = !ext.is_empty()
        && ext.len() <= 12
        && ext.split('.').count() <= 2
        && ext
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()));
    plain.then_some(ext)
}

/// A folder is only kept if it is an absolute path to a folder that exists.
fn clean_dir(dir: Option<PathBuf>) -> Option<PathBuf> {
    dir.filter(|path| path.is_absolute() && path.is_dir())
}

impl Settings {
    /// The same settings with every value brought into range.
    pub fn normalised(mut self) -> Self {
        self.default_dir = clean_dir(self.default_dir);
        self.max_concurrent = self.max_concurrent.clamp(1, MAX_CONCURRENT);
        self.default_connections = self.default_connections.clamp(1, MAX_CONNECTIONS);
        self.speed_limit_kbps = self
            .speed_limit_kbps
            .filter(|limit| *limit > 0)
            .map(|l| l.min(10_000_000));
        self.categories = self
            .categories
            .into_iter()
            .filter_map(|category| {
                let name: String = category
                    .name
                    .trim()
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(40)
                    .collect();
                if name.is_empty() {
                    return None;
                }
                let mut extensions: Vec<String> = category
                    .extensions
                    .iter()
                    .filter_map(|ext| clean_extension(ext))
                    .collect();
                extensions.dedup();
                Some(Category {
                    name,
                    extensions,
                    dir: clean_dir(category.dir),
                })
            })
            .take(20)
            .collect();
        self
    }

    /// The category a file name belongs to. A longer ending wins, so
    /// `backup.tar.gz` is matched by `tar.gz` before `gz`.
    pub fn category_of(&self, file_name: &str) -> Option<&Category> {
        let name = file_name.to_ascii_lowercase();
        self.categories
            .iter()
            .flat_map(|category| category.extensions.iter().map(move |ext| (category, ext)))
            .filter(|(_, ext)| name.ends_with(&format!(".{ext}")))
            .max_by_key(|(_, ext)| ext.len())
            .map(|(category, _)| category)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brings_every_value_into_range() {
        let wild = Settings {
            default_dir: Some("relative/folder".into()),
            max_concurrent: 0,
            default_connections: 99,
            speed_limit_kbps: Some(0),
            categories: vec![
                Category {
                    name: "  Mine ".into(),
                    extensions: vec![".ZIP".into(), "bad type".into()],
                    dir: Some("/no/such/folder".into()),
                },
                Category {
                    name: "".into(),
                    extensions: vec!["x".into()],
                    dir: None,
                },
            ],
            ..Settings::default()
        };
        let tidy = wild.normalised();
        assert_eq!(tidy.default_dir, None);
        assert_eq!(tidy.max_concurrent, 1);
        assert_eq!(tidy.default_connections, MAX_CONNECTIONS);
        assert_eq!(tidy.speed_limit_kbps, None);
        assert_eq!(
            tidy.categories,
            vec![Category {
                name: "Mine".into(),
                extensions: vec!["zip".into()],
                dir: None
            }]
        );
        assert_eq!(Settings::default().normalised(), Settings::default());
    }

    #[test]
    fn reads_what_is_stored_and_fills_in_the_rest() {
        let stored: Settings =
            serde_json::from_str(r#"{"maxConcurrent":5,"watchClipboard":false}"#).unwrap();
        assert_eq!(stored.max_concurrent, 5);
        assert!(!stored.watch_clipboard);
        assert_eq!(stored.default_connections, DEFAULT_CONNECTIONS);
        assert_eq!(stored.categories.len(), 5);
    }

    #[test]
    fn finds_the_category_by_how_the_name_ends() {
        let settings = Settings::default();
        let name = |file| settings.category_of(file).map(|c| c.name.as_str());
        assert_eq!(name("design-assets-2026.ZIP"), Some("Archives"));
        assert_eq!(name("backup.tar.gz"), Some("Archives"));
        assert_eq!(name("node-v22.11.0.pkg"), Some("Apps"));
        assert_eq!(name("quarterly-report.pdf"), Some("Documents"));
        assert_eq!(name("clip.mp4"), Some("Video"));
        assert_eq!(name("notes"), None);
        assert_eq!(name("zip"), None);
    }
}
