//! Global shortcuts through the portal, where the desktop supports it.

use ashpd::desktop::{
    global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut},
    CreateSessionOptions,
};
use futures_util::StreamExt;
use gorgon_core::CaptureMode;

/// ShareX's defaults.
const SHORTCUTS: &[(&str, &str, &str, CaptureMode)] = &[
    (
        "capture-region",
        "Capture region",
        "CTRL+Print",
        CaptureMode::Region,
    ),
    (
        "capture-screen",
        "Capture screen",
        "Print",
        CaptureMode::Screen,
    ),
    (
        "capture-window",
        "Capture window",
        "ALT+Print",
        CaptureMode::Window,
    ),
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
        .map(|(id, description, trigger, _)| {
            NewShortcut::new(*id, *description).preferred_trigger(*trigger)
        })
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
        if let Some((.., mode)) = SHORTCUTS.iter().find(|(id, ..)| *id == event.shortcut_id()) {
            if let Err(e) = crate::app::request_capture(*mode) {
                eprintln!("{e}");
            }
        }
    }
    // The session ends when it is dropped, so keep it until the stream ends.
    drop(session);
    Ok(())
}
