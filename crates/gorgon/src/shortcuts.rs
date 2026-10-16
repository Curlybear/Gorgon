//! Global shortcuts through the portal, where the desktop supports it.

use ashpd::desktop::{
    global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
    CreateSessionOptions,
};
use futures_util::StreamExt;
use gorgon_core::{record::Format, CaptureMode};

struct Binding {
    id: &'static str,
    description: &'static str,
    trigger: &'static str,
    action: Action,
}

#[derive(Clone, Copy)]
enum Action {
    Capture(CaptureMode, bool),
    /// Starts a recording, or stops the running one.
    Record(Format),
}

/// ShareX's defaults, plus Ctrl+Shift+Print to capture a region and edit it,
/// and Ctrl+Alt+Print / Ctrl+Alt+Shift+Print to record. GNOME takes Shift+Print
/// for its own screenshot and wins most presses, so recording avoids it. The ids
/// changed with the keys because desktops keep the key stored for an id.
const SHORTCUTS: &[Binding] = &[
    Binding {
        id: "capture-region",
        description: "Capture region",
        trigger: "CTRL+Print",
        action: Action::Capture(CaptureMode::Region, false),
    },
    Binding {
        id: "capture-screen",
        description: "Capture screen",
        trigger: "Print",
        action: Action::Capture(CaptureMode::Screen, false),
    },
    Binding {
        id: "capture-window",
        description: "Capture window",
        trigger: "ALT+Print",
        action: Action::Capture(CaptureMode::Window, false),
    },
    Binding {
        id: "capture-region-edit",
        description: "Capture region and edit",
        trigger: "CTRL+SHIFT+Print",
        action: Action::Capture(CaptureMode::Region, true),
    },
    Binding {
        id: "record-mp4",
        description: "Record region as video (again to stop)",
        trigger: "CTRL+ALT+Print",
        action: Action::Record(Format::Mp4),
    },
    Binding {
        id: "record-gif-region",
        description: "Record region as GIF (again to stop)",
        trigger: "CTRL+ALT+SHIFT+Print",
        action: Action::Record(Format::Gif),
    },
];

pub async fn run() {
    if let Err(e) = register().await {
        eprintln!(
            "no global shortcuts ({e}). Bind `gorgon capture region` (or screen, window) \
             in your desktop's keyboard settings instead."
        );
    }
}

async fn register() -> ashpd::Result<()> {
    let portal = GlobalShortcuts::new().await?;
    let session = portal
        .create_session(CreateSessionOptions::default())
        .await?;
    let wanted: Vec<NewShortcut> = SHORTCUTS
        .iter()
        .map(|b| NewShortcut::new(b.id, b.description).preferred_trigger(b.trigger))
        .collect();
    let bound = portal
        .bind_shortcuts(&session, &wanted, None, BindShortcutsOptions::default())
        .await?
        .response()?;
    for s in bound.shortcuts() {
        println!("shortcut {} bound to {}", s.id(), s.trigger_description());
    }

    let mut activated = portal.receive_activated().await?;
    while let Some(event) = activated.next().await {
        if let Some(b) = SHORTCUTS.iter().find(|b| b.id == event.shortcut_id()) {
            let result = match b.action {
                Action::Capture(mode, edit) => crate::app::request_capture(mode, edit),
                Action::Record(format) => crate::recorder::toggle(format),
            };
            if let Err(e) = result {
                eprintln!("{e}");
            }
        }
    }
    // The session ends when it is dropped, so keep it until the stream ends.
    drop(session);
    Ok(())
}
