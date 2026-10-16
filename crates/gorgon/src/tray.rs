//! Tray icon (StatusNotifierItem over D-Bus).

use gorgon_core::{config::Config, CaptureMode};
use std::sync::OnceLock;

use gorgon_core::record::Format;
use ksni::{menu::StandardItem, Handle, MenuItem, TrayMethods};

static HANDLE: OnceLock<Handle<GorgonTray>> = OnceLock::new();

/// Redraws the icon and menu, after recording starts or stops.
pub fn refresh() {
    if let Some(handle) = HANDLE.get() {
        let handle = handle.clone();
        crate::runtime().spawn(async move {
            handle.update(|_| {}).await;
        });
    }
}

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
        if crate::recorder::is_recording() {
            "media-record".into()
        } else {
            "camera-photo".into()
        }
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
        let mut items = vec![];
        if crate::recorder::is_recording() {
            items.push(item("Stop recording", crate::recorder::stop));
            items.push(MenuItem::Separator);
        }
        items.extend([
            item("Capture region", || capture(CaptureMode::Region)),
            item("Capture region and edit", || {
                request(CaptureMode::Region, true)
            }),
            item("Capture screen", || capture(CaptureMode::Screen)),
            item("Capture window", || capture(CaptureMode::Window)),
            MenuItem::Separator,
            item("Record region (video)", || record(Format::Mp4)),
            item("Record region (GIF)", || record(Format::Gif)),
            MenuItem::Separator,
            item("History", || window("history")),
            item("Settings", || window("settings")),
            item("Open captures folder", open_captures_folder),
            item("Quit", || std::process::exit(0)),
        ]);
        items
    }
}

fn record(format: Format) {
    if let Err(e) = crate::recorder::toggle(format) {
        eprintln!("{e}");
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
            let _ = HANDLE.set(handle);
            std::future::pending::<()>().await;
        }
        Err(e) => eprintln!(
            "no tray icon ({e}). On GNOME, enable the AppIndicator extension; \
             captures still work from the gorgon command."
        ),
    }
}
