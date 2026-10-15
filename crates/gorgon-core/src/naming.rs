//! File names for saved captures.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};

/// A local timestamp for file names.
pub type Time = DateTime<Local>;

pub fn now() -> Time {
    Local::now()
}

/// Builds `dir/<pattern>.<ext>` for `time`, adding `-2`, `-3`... if the name is taken.
pub fn unique_path(dir: &Path, pattern: &str, ext: &str, time: DateTime<Local>) -> PathBuf {
    let stem = format_stem(pattern, time);
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem}-{n}.{ext}")))
        .find(|p| !p.exists())
        .expect("an unused file name exists")
}

/// Formats the pattern, falling back to the default when it isn't valid strftime.
fn format_stem(pattern: &str, time: DateTime<Local>) -> String {
    use std::fmt::Write;

    let mut out = String::new();
    if write!(out, "{}", time.format(pattern)).is_err() || out.is_empty() {
        out = time.format("%Y-%m-%d_%H-%M-%S").to_string();
    }
    // A pattern must not be able to escape the save folder.
    out.replace('/', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn time() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 15, 20, 30, 5).unwrap()
    }

    #[test]
    fn formats_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let path = unique_path(dir.path(), "%Y-%m-%d_%H-%M-%S", "png", time());
        assert_eq!(path, dir.path().join("2026-10-15_20-30-05.png"));
    }

    #[test]
    fn avoids_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shot.png"), b"").unwrap();
        std::fs::write(dir.path().join("shot-2.png"), b"").unwrap();
        let path = unique_path(dir.path(), "shot", "png", time());
        assert_eq!(path, dir.path().join("shot-3.png"));
    }

    #[test]
    fn invalid_pattern_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = unique_path(dir.path(), "%Q", "png", time());
        assert_eq!(path, dir.path().join("2026-10-15_20-30-05.png"));
    }

    #[test]
    fn slashes_stay_in_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = unique_path(dir.path(), "%Y/%m", "png", time());
        assert_eq!(path, dir.path().join("2026-10.png"));
    }
}
