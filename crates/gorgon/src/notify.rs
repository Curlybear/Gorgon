//! Desktop notifications.

use std::path::Path;

use gorgon_core::{config::Config, record::Format};
use notify_rust::{Hint, Notification, Timeout};

use crate::pipeline::Links;

const APP_NAME: &str = "Gorgon";

/// Tells the user where the capture went, with buttons to open it.
pub fn saved(path: &Path, copied: bool) {
    let path = path.to_owned();
    std::thread::spawn(move || {
        let body = if copied {
            format!("Copied to the clipboard and saved to {}", path.display())
        } else {
            format!("Saved to {}", path.display())
        };
        let recording = is_recording_file(&path);
        let mut note = Notification::new();
        note.appname(APP_NAME)
            .hint(Hint::DesktopEntry(crate::icons::NAME.into()))
            .summary(if recording {
                "Recording saved"
            } else {
                "Screenshot captured"
            })
            .body(&escape(&body))
            .icon(crate::icons::NAME)
            .action("default", "Open")
            .action("open", "Open");
        if !recording {
            note.image_path(&path.display().to_string())
                .action("edit", "Edit");
        }
        let shown = note
            .action("folder", "Show in folder")
            .timeout(Timeout::Milliseconds(6000))
            .show();
        match shown {
            Ok(handle) => handle.wait_for_action(|action| match action {
                "default" | "open" => open(&path),
                "edit" => {
                    if let Err(e) = crate::app::request_edit_file(path.clone()) {
                        error(&e);
                    }
                }
                "folder" => {
                    if let Some(dir) = path.parent() {
                        open(dir);
                    }
                }
                _ => {}
            }),
            Err(e) => eprintln!("notification failed: {e}"),
        }
    });
}

/// The capture is online: shows the link, with buttons to open or copy it.
pub fn uploaded(path: &Path, links: &Links, copied: bool) {
    let path = path.to_owned();
    let links = links.clone();
    std::thread::spawn(move || {
        let mut body = if copied {
            format!("{} (copied)", links.url)
        } else {
            links.url.clone()
        };
        if let Some(e) = &links.shorten_error {
            body.push_str(&format!("\nShortening failed: {e}"));
        }
        let shown = Notification::new()
            .appname(APP_NAME)
            .hint(Hint::DesktopEntry(crate::icons::NAME.into()))
            .summary(if is_recording_file(&path) {
                "Recording uploaded"
            } else {
                "Screenshot uploaded"
            })
            .body(&escape(&body))
            .icon(crate::icons::NAME)
            .image_path(&path.display().to_string())
            .action("default", "Open link")
            .action("copy", "Copy link")
            .action("file", "Open file")
            .action("folder", "Show in folder")
            .timeout(Timeout::Milliseconds(8000))
            .show();
        match shown {
            Ok(handle) => handle.wait_for_action(|action| match action {
                "default" => open_url(&links.url),
                "copy" => {
                    if !crate::app::copy_text(&links.url) {
                        error(&format!("could not copy the link: {}", links.url));
                    }
                }
                "file" => open(&path),
                "folder" => {
                    if let Some(dir) = path.parent() {
                        open(dir);
                    }
                }
                _ => {}
            }),
            Err(e) => eprintln!("notification failed: {e}"),
        }
    });
}

/// The capture was saved but the upload failed: offers to retry.
pub fn upload_failed(path: &Path, message: &str, id: Option<u64>) {
    eprintln!("upload failed: {message}");
    let path = path.to_owned();
    let message = message.to_owned();
    std::thread::spawn(move || {
        let shown = Notification::new()
            .appname(APP_NAME)
            .hint(Hint::DesktopEntry(crate::icons::NAME.into()))
            .summary("Upload failed")
            .body(&escape(&format!("{message}\nSaved to {}", path.display())))
            .icon("dialog-error")
            .action("retry", "Retry")
            .action("folder", "Show in folder")
            .timeout(Timeout::Milliseconds(10000))
            .show();
        match shown {
            Ok(handle) => handle.wait_for_action(|action| match action {
                "retry" => {
                    let config = Config::load().unwrap_or_default();
                    crate::app::upload_saved(path.clone(), config, false, id);
                }
                "folder" => {
                    if let Some(dir) = path.parent() {
                        open(dir);
                    }
                }
                _ => {}
            }),
            Err(e) => eprintln!("notification failed: {e}"),
        }
    });
}

static RECORDING_NOTE: std::sync::Mutex<Option<u32>> = std::sync::Mutex::new(None);

/// Stays up while recording, with a Stop button.
pub fn recording(format: Format) {
    std::thread::spawn(move || {
        let what = match format {
            Format::Mp4 => "video",
            Format::Gif => "GIF",
        };
        let shown = Notification::new()
            .appname(APP_NAME)
            .hint(Hint::DesktopEntry(crate::icons::NAME.into()))
            .summary(&format!("Recording {what}"))
            .body("Press the same shortcut again, or Stop, to finish.")
            .icon("media-record")
            .action("stop", "Stop")
            .timeout(Timeout::Never)
            .show();
        match shown {
            Ok(handle) => {
                if let Ok(mut id) = RECORDING_NOTE.lock() {
                    *id = Some(handle.id());
                }
                handle.wait_for_action(|action| {
                    if action == "stop" {
                        crate::recorder::stop();
                    }
                });
            }
            Err(e) => eprintln!("notification failed: {e}"),
        }
    });
}

/// Replaces the "Recording" notification once it has stopped.
pub fn recording_stopped() {
    let Some(id) = RECORDING_NOTE.lock().ok().and_then(|mut id| id.take()) else {
        return;
    };
    std::thread::spawn(move || {
        let _ = Notification::new()
            .appname(APP_NAME)
            .hint(Hint::DesktopEntry(crate::icons::NAME.into()))
            .id(id)
            .summary("Recording stopped")
            .body("Saving…")
            .icon("media-record")
            .timeout(Timeout::Milliseconds(1500))
            .show();
    });
}

pub fn error(message: &str) {
    eprintln!("error: {message}");
    let message = message.to_owned();
    std::thread::spawn(move || {
        if let Err(e) = Notification::new()
            .appname(APP_NAME)
            .hint(Hint::DesktopEntry(crate::icons::NAME.into()))
            .summary("Capture failed")
            .body(&escape(&message))
            .icon("dialog-error")
            .show()
        {
            eprintln!("notification failed: {e}");
        }
    });
}

fn is_recording_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e == "mp4" || e == "webm" || e == "gif")
}

/// Notification servers may read bodies as markup (`<b>`, `<a>`, `<img>`),
/// and bodies carry server replies and file names.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn open_url(url: &str) {
    if !gorgon_core::upload::is_web_link(url) {
        eprintln!("not opening {url}: not a web link");
        return;
    }
    if let Err(e) = std::process::Command::new("xdg-open").arg(url).spawn() {
        eprintln!("could not open {url}: {e}");
    }
}

fn open(path: &Path) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
        eprintln!("could not open {}: {e}", path.display());
    }
}
