//! The list of past captures, in `~/.local/share/gorgon/history.json`.
//!
//! Small enough to rewrite whole on every change; writes go through a temp
//! file and a rename so a crash never leaves half a file.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

/// One capture or uploaded file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Unique, increasing: milliseconds since 1970 when it was added.
    pub id: u64,
    /// Seconds since 1970.
    pub time: u64,
    pub path: PathBuf,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletion_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uploader: Option<String>,
    /// Why the last upload failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Entry {
    pub fn new(path: &Path, width: u32, height: u32) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            id: now.as_millis() as u64,
            time: now.as_secs(),
            path: path.to_owned(),
            width,
            height,
            url: None,
            deletion_url: None,
            uploader: None,
            error: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("no data folder")]
    NoDataDir,
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Entries kept; older ones are dropped (their files stay on disk).
const MAX_ENTRIES: usize = 2000;

/// Serializes changes within this process.
static LOCK: Mutex<()> = Mutex::new(());

pub fn path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("gorgon").join("history.json"))
}

/// Every entry, oldest first. A missing or unreadable file is an empty history.
pub fn load() -> Vec<Entry> {
    path().map(|p| load_from(&p)).unwrap_or_default()
}

pub fn load_from(path: &Path) -> Vec<Entry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_to(path: &Path, entries: &[Entry]) -> Result<(), HistoryError> {
    let err = |source| HistoryError::Write {
        path: path.to_owned(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(entries).expect("history serializes");
    std::fs::write(&tmp, text).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}

/// Loads, changes and saves the history under the lock.
fn modify<T>(path: &Path, f: impl FnOnce(&mut Vec<Entry>) -> T) -> Result<T, HistoryError> {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut entries = load_from(path);
    let out = f(&mut entries);
    if entries.len() > MAX_ENTRIES {
        let extra = entries.len() - MAX_ENTRIES;
        entries.drain(..extra);
    }
    save_to(path, &entries)?;
    Ok(out)
}

fn default_path() -> Result<PathBuf, HistoryError> {
    path().ok_or(HistoryError::NoDataDir)
}

/// Adds an entry and returns its id.
pub fn add(entry: Entry) -> Result<u64, HistoryError> {
    add_to(&default_path()?, entry)
}

pub fn add_to(path: &Path, mut entry: Entry) -> Result<u64, HistoryError> {
    modify(path, |entries| {
        // Two captures in the same millisecond still get distinct ids.
        if let Some(last) = entries.iter().map(|e| e.id).max() {
            entry.id = entry.id.max(last + 1);
        }
        entries.push(entry.clone());
        entry.id
    })
}

/// Changes one entry. Does nothing if it was removed meanwhile.
pub fn update(id: u64, f: impl FnOnce(&mut Entry)) -> Result<(), HistoryError> {
    update_in(&default_path()?, id, f)
}

pub fn update_in(path: &Path, id: u64, f: impl FnOnce(&mut Entry)) -> Result<(), HistoryError> {
    modify(path, |entries| {
        if let Some(e) = entries.iter_mut().find(|e| e.id == id) {
            f(e);
        }
    })
}

pub fn remove(id: u64) -> Result<(), HistoryError> {
    remove_from(&default_path()?, id)
}

pub fn remove_from(path: &Path, id: u64) -> Result<(), HistoryError> {
    modify(path, |entries| entries.retain(|e| e.id != id))
}

pub fn get(id: u64) -> Option<Entry> {
    load().into_iter().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_update_remove() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub").join("history.json");
        let a = add_to(&file, Entry::new(Path::new("/a.png"), 10, 20)).unwrap();
        let b = add_to(&file, Entry::new(Path::new("/b.png"), 1, 1)).unwrap();
        assert!(b > a);
        update_in(&file, a, |e| e.url = Some("https://x/a".into())).unwrap();
        let entries = load_from(&file);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url.as_deref(), Some("https://x/a"));
        remove_from(&file, a).unwrap();
        let entries = load_from(&file);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, Path::new("/b.png"));
    }

    #[test]
    fn unreadable_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("history.json");
        std::fs::write(&file, "garbage").unwrap();
        assert!(load_from(&file).is_empty());
    }
}
