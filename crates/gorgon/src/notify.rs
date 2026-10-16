//! Desktop notifications.

use std::path::Path;

use gorgon_core::config::Config;
use notify_rust::{Notification, Timeout};

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
        let shown = Notification::new()
            .appname(APP_NAME)
            .summary("Screenshot captured")
            .body(&body)
            .icon("camera-photo")
            .image_path(&path.display().to_string())
            .action("default", "Open")
            .action("open", "Open")
            .action("edit", "Edit")
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
            .summary("Screenshot uploaded")
            .body(&body)
            .icon("camera-photo")
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
pub fn upload_failed(path: &Path, message: &str) {
    eprintln!("upload failed: {message}");
    let path = path.to_owned();
    let message = message.to_owned();
    std::thread::spawn(move || {
        let shown = Notification::new()
            .appname(APP_NAME)
            .summary("Upload failed")
            .body(&format!("{message}\nSaved to {}", path.display()))
            .icon("dialog-error")
            .action("retry", "Retry")
            .action("folder", "Show in folder")
            .timeout(Timeout::Milliseconds(10000))
            .show();
        match shown {
            Ok(handle) => handle.wait_for_action(|action| match action {
                "retry" => {
                    let config = Config::load().unwrap_or_default();
                    crate::app::upload_saved(path.clone(), config, false);
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

pub fn error(message: &str) {
    eprintln!("error: {message}");
    let message = message.to_owned();
    std::thread::spawn(move || {
        if let Err(e) = Notification::new()
            .appname(APP_NAME)
            .summary("Capture failed")
            .body(&message)
            .icon("dialog-error")
            .show()
        {
            eprintln!("notification failed: {e}");
        }
    });
}

fn open_url(url: &str) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(url).spawn() {
        eprintln!("could not open {url}: {e}");
    }
}

fn open(path: &Path) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
        eprintln!("could not open {}: {e}", path.display());
    }
}
