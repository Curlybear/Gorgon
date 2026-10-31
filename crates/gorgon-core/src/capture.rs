//! Screenshots through the xdg-desktop-portal Screenshot interface.

use std::path::{Path, PathBuf};

use ashpd::desktop::{screenshot::Screenshot, ResponseError};

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("the screenshot portal failed: {0}")]
    Portal(#[from] ashpd::Error),
    #[error(
        "the desktop refused the screenshot. See Troubleshooting in Gorgon's README \
         to allow it"
    )]
    Refused,
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
    let response = match request(interactive).await {
        // GNOME refuses a silent screenshot, without asking, to an app it has
        // no stored permission for. Gorgon started at login is such an app
        // (it is named after how it was launched). The desktop's own
        // screenshot dialog needs no permission, so offer that instead.
        Err(ashpd::Error::Response(ResponseError::Other)) if !interactive => request(true).await,
        other => other,
    };
    let response = match response {
        Err(ashpd::Error::Response(ResponseError::Other)) => return Err(CaptureError::Refused),
        other => other?,
    };
    let uri = response.uri().as_str();
    let source = file_uri_to_path(uri).ok_or_else(|| CaptureError::NotLocal(uri.to_owned()))?;
    let dest = runtime_dir()?.join(format!(
        "capture-{}.png",
        chrono::Local::now().format("%Y%m%d-%H%M%S-%f")
    ));
    move_file(&source, &dest)?;
    Ok(dest)
}

async fn request(interactive: bool) -> ashpd::Result<Screenshot> {
    Screenshot::request()
        .interactive(interactive)
        .modal(false)
        .send()
        .await?
        .response()
}

/// Gorgon's private scratch folder, emptied by the session on logout.
/// Without `XDG_RUNTIME_DIR` (cron, ssh) it falls back to a per-user folder
/// in `/tmp`, which must not be one another user prepared.
pub fn runtime_dir() -> std::io::Result<PathBuf> {
    match dirs::runtime_dir() {
        Some(dir) => {
            let dir = dir.join("gorgon");
            crate::private::create_dir(&dir)?;
            Ok(dir)
        }
        None => {
            // SAFETY: getuid has no preconditions and can't fail.
            let uid = unsafe { libc::getuid() };
            let dir = std::env::temp_dir().join(format!("gorgon-{uid}"));
            crate::private::ensure_own_dir(&dir)?;
            Ok(dir)
        }
    }
}

/// Turns `/a b#1.png` into `file:///a%20b%231.png`, for Qt and other URL
/// readers. Without the encoding, `#`, `%` or `?` in a name break the URL.
pub fn path_to_file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut out = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
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
        let odd = Path::new("/home/you/Shots #1/100% é?.png");
        let uri = path_to_file_uri(odd);
        assert_eq!(uri, "file:///home/you/Shots%20%231/100%25%20%C3%A9%3F.png");
        assert_eq!(file_uri_to_path(&uri).as_deref(), Some(odd));
    }
}
