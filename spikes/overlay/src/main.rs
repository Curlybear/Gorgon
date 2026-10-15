//! M0 spike: freeze the screen, select a region over it, save the crop.
//!
//! Checks three things on each desktop:
//! - fullscreen per-monitor windows showing a frozen portal screenshot line up
//!   with the real screens (multi-monitor, fractional scaling);
//! - a `QQuickPaintedItem` implemented in Rust draws correctly (the editor
//!   will use the same approach);
//! - a tray icon shows up (run with `--tray`).

mod capture;
mod selection;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/dev/gorgon/spike/qml/main.qml"));
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
