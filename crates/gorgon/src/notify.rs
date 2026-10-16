//! Desktop notifications.

use std::path::Path;

use notify_rust::{Notification, Timeout};

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

fn open(path: &Path) {
    if let Err(e) = std::process::Command::new("xdg-open").arg(path).spawn() {
        eprintln!("could not open {}: {e}", path.display());
    }
}
