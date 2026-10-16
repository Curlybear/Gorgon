//! Global shortcuts through the portal, where the desktop supports it.

use ashpd::desktop::{
    global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
    CreateSessionOptions,
};
use futures_util::StreamExt;
use gorgon_core::CaptureMode;

struct Binding {
    id: &'static str,
    description: &'static str,
    trigger: &'static str,
    mode: CaptureMode,
    edit: bool,
}

/// ShareX's defaults, plus Ctrl+Shift+Print to capture a region and edit it.
const SHORTCUTS: &[Binding] = &[
    Binding {
        id: "capture-region",
        description: "Capture region",
        trigger: "CTRL+Print",
        mode: CaptureMode::Region,
        edit: false,
    },
    Binding {
        id: "capture-screen",
        description: "Capture screen",
        trigger: "Print",
        mode: CaptureMode::Screen,
        edit: false,
    },
    Binding {
        id: "capture-window",
        description: "Capture window",
        trigger: "ALT+Print",
        mode: CaptureMode::Window,
        edit: false,
    },
    Binding {
        id: "capture-region-edit",
        description: "Capture region and edit",
        trigger: "CTRL+SHIFT+Print",
        mode: CaptureMode::Region,
        edit: true,
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
            if let Err(e) = crate::app::request_capture(b.mode, b.edit) {
                eprintln!("{e}");
            }
        }
    }
    // The session ends when it is dropped, so keep it until the stream ends.
    drop(session);
    Ok(())
}
