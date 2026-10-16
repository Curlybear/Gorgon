//! Tray icon (StatusNotifierItem over D-Bus).

use gorgon_core::{config::Config, CaptureMode};
use ksni::{menu::StandardItem, MenuItem, TrayMethods};

struct GorgonTray;

fn capture(mode: CaptureMode) {
    request(mode, false);
}

fn request(mode: CaptureMode, edit: bool) {
    if let Err(e) = crate::app::request_capture(mode, edit) {
        eprintln!("{e}");
    }
}

impl ksni::Tray for GorgonTray {
    fn id(&self) -> String {
        "gorgon".into()
    }

    fn title(&self) -> String {
        "Gorgon".into()
    }

    fn icon_name(&self) -> String {
        "camera-photo".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        capture(CaptureMode::Region);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let item = |label: &str, action: fn()| -> MenuItem<Self> {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |_: &mut Self| action()),
                ..Default::default()
            }
            .into()
        };
        vec![
            item("Capture region", || capture(CaptureMode::Region)),
            item("Capture region and edit", || {
                request(CaptureMode::Region, true)
            }),
            item("Capture screen", || capture(CaptureMode::Screen)),
            item("Capture window", || capture(CaptureMode::Window)),
            MenuItem::Separator,
            item("History", || window("history")),
            item("Settings", || window("settings")),
            item("Open captures folder", open_captures_folder),
            item("Quit", || std::process::exit(0)),
        ]
    }
}

fn window(name: &'static str) {
    if let Err(e) = crate::app::request_window(name) {
        eprintln!("{e}");
    }
}

fn open_captures_folder() {
    let dir = Config::load().unwrap_or_default().save_dir;
    let _ = std::fs::create_dir_all(&dir);
    if let Err(e) = std::process::Command::new("xdg-open").arg(&dir).spawn() {
        eprintln!("could not open {}: {e}", dir.display());
    }
}

/// Shows the tray icon for as long as the daemon runs.
pub async fn run() {
    match GorgonTray.spawn().await {
        Ok(handle) => {
            std::future::pending::<()>().await;
            drop(handle);
        }
        Err(e) => eprintln!(
            "no tray icon ({e}). On GNOME, enable the AppIndicator extension; \
             captures still work from the gorgon command."
        ),
    }
}
