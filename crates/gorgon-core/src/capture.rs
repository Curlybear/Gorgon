//! Screenshots through the xdg-desktop-portal Screenshot interface.

use std::path::{Path, PathBuf};

use ashpd::desktop::screenshot::Screenshot;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("the screenshot portal failed: {0}")]
    Portal(#[from] ashpd::Error),
    #[error("the portal returned {0}, which isn't a local file")]
    NotLocal(String),
    #[error("could not move the screenshot: {0}")]
    Io(#[from] std::io::Error),
}

/// Takes a screenshot of the whole desktop, or lets the desktop's own picker
/// choose (for window capture) when `interactive` is set.
///
/// Portals save the image wherever they like (GNOME uses `~/Pictures`), so the
/// file is moved into Gorgon's runtime folder; the caller owns it from then on.
pub async fn screenshot(interactive: bool) -> Result<PathBuf, CaptureError> {
    let response = Screenshot::request()
        .interactive(interactive)
        .modal(false)
        .send()
        .await?
        .response()?;
    let uri = response.uri().as_str();
    let source = file_uri_to_path(uri).ok_or_else(|| CaptureError::NotLocal(uri.to_owned()))?;
    let dest = runtime_dir()?.join(format!(
        "capture-{}.png",
        chrono::Local::now().format("%Y%m%d-%H%M%S-%f")
    ));
    move_file(&source, &dest)?;
    Ok(dest)
}

/// Gorgon's private scratch folder, emptied by the session on logout.
pub fn runtime_dir() -> std::io::Result<PathBuf> {
    let dir = dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("gorgon");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Turns `file:///a%20b.png` into `/a b.png`.
pub fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
}

fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Different filesystems: copy, then remove the original.
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_file_uris() {
        assert_eq!(
            file_uri_to_path("file:///home/you/Pictures/Screenshot%20from%20today.png"),
            Some(PathBuf::from(
                "/home/you/Pictures/Screenshot from today.png"
            ))
        );
        assert_eq!(file_uri_to_path("https://example.com/a.png"), None);
        assert_eq!(file_uri_to_path("file:///bad%2"), None);
    }
}
