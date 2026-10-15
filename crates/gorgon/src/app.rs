//! The QObject behind the overlays: runs the capture flow on the Qt thread.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    #[namespace = "gorgon"]
    unsafe extern "C++" {
        include!("gorgon/qt_helpers.h");
        fn set_clipboard_image(rgba: &[u8], width: i32, height: i32) -> bool;
        fn keep_running_without_windows();
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QUrl, image_url, cxx_name = "imageUrl")]
        #[qproperty(i32, image_width, cxx_name = "imageWidth")]
        #[qproperty(i32, image_height, cxx_name = "imageHeight")]
        type App = super::AppRust;

        /// Show the frozen screenshot and let the user drag a region.
        #[qsignal]
        #[cxx_name = "selectRegion"]
        fn select_region(self: Pin<&mut App>);

        /// Flash the screens to confirm a full capture (and take focus for the clipboard).
        #[qsignal]
        fn flash(self: Pin<&mut App>);

        /// Starts a capture: "region", "screen" or "window".
        #[qinvokable]
        #[cxx_name = "startCapture"]
        fn start_capture(self: Pin<&mut App>, mode: &QString);

        /// Finishes a region capture; the rectangle is in screenshot pixels.
        #[qinvokable]
        #[cxx_name = "finishRegion"]
        fn finish_region(self: Pin<&mut App>, x: f64, y: f64, width: f64, height: f64);

        /// Finishes a screen or window capture.
        #[qinvokable]
        #[cxx_name = "finishFull"]
        fn finish_full(self: Pin<&mut App>);

        #[qinvokable]
        fn cancel(self: Pin<&mut App>);
    }

    impl cxx_qt::Threading for App {}
    impl cxx_qt::Initialize for App {}
}

use std::{path::PathBuf, pin::Pin, sync::OnceLock};

use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};
use gorgon_core::{
    capture::{self, CaptureError},
    config::Config,
    naming,
    output::{self, Rect},
    CaptureMode,
};

use crate::notify;

/// The UI thread handle, set once the QML engine has created the App.
static QT: OnceLock<CxxQtThread<qobject::App>> = OnceLock::new();

/// Asks the UI thread to start a capture. Safe to call from any thread.
pub fn request_capture(mode: CaptureMode) -> Result<(), String> {
    let qt = QT.get().ok_or("Gorgon is still starting")?;
    qt.queue(move |app| app.start(mode))
        .map_err(|e| format!("Gorgon's UI is not running: {e:?}"))
}

#[derive(Default)]
pub struct AppRust {
    image_url: QUrl,
    image_width: i32,
    image_height: i32,
    busy: bool,
    config: Config,
    /// Screenshot being worked on, in Gorgon's runtime folder.
    current: Option<PathBuf>,
}

impl cxx_qt::Initialize for qobject::App {
    fn initialize(self: Pin<&mut Self>) {
        qobject::keep_running_without_windows();
        let _ = QT.set(self.qt_thread());
    }
}

impl qobject::App {
    pub fn start_capture(self: Pin<&mut Self>, mode: &QString) {
        match mode.to_string().parse() {
            Ok(mode) => self.start(mode),
            Err(e) => eprintln!("{e}"),
        }
    }

    fn start(mut self: Pin<&mut Self>, mode: CaptureMode) {
        if self.busy {
            eprintln!("a capture is already in progress");
            return;
        }
        let config = Config::load().unwrap_or_else(|e| {
            notify::error(&format!("{e}. Using default settings."));
            Config::default()
        });
        {
            let mut rust = self.as_mut().rust_mut();
            rust.busy = true;
            rust.config = config;
        }
        let qt = self.qt_thread();
        crate::runtime().spawn(async move {
            let result = capture::screenshot(mode == CaptureMode::Window).await;
            let _ = qt.queue(move |app| app.captured(mode, result));
        });
    }

    fn captured(
        mut self: Pin<&mut Self>,
        mode: CaptureMode,
        result: Result<PathBuf, CaptureError>,
    ) {
        let path = match result {
            Ok(path) => path,
            Err(e) => {
                notify::error(&e.to_string());
                self.as_mut().rust_mut().busy = false;
                return;
            }
        };
        let (w, h) = match image::image_dimensions(&path) {
            Ok(dims) => dims,
            Err(e) => {
                notify::error(&format!("unreadable screenshot: {e}"));
                let _ = std::fs::remove_file(&path);
                self.as_mut().rust_mut().busy = false;
                return;
            }
        };
        self.as_mut().set_image_width(w as i32);
        self.as_mut().set_image_height(h as i32);
        self.as_mut()
            .set_image_url(QUrl::from(&format!("file://{}", path.display())));
        self.as_mut().rust_mut().current = Some(path);
        match mode {
            CaptureMode::Region => self.select_region(),
            CaptureMode::Screen | CaptureMode::Window => self.flash(),
        }
    }

    pub fn finish_region(self: Pin<&mut Self>, x: f64, y: f64, width: f64, height: f64) {
        self.finish(Some(Rect {
            x,
            y,
            width,
            height,
        }));
    }

    pub fn finish_full(self: Pin<&mut Self>) {
        self.finish(None);
    }

    pub fn cancel(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        if let Some(path) = rust.current.take() {
            let _ = std::fs::remove_file(path);
        }
        rust.busy = false;
    }

    fn finish(mut self: Pin<&mut Self>, rect: Option<Rect>) {
        let (source, config) = {
            let mut rust = self.as_mut().rust_mut();
            rust.busy = false;
            (rust.current.take(), rust.config.clone())
        };
        let Some(source) = source else {
            return;
        };
        let image = output::load(&source, rect);
        let _ = std::fs::remove_file(&source);
        let image = match image {
            Ok(image) => image,
            Err(e) => {
                notify::error(&format!("could not read the screenshot: {e}"));
                return;
            }
        };

        // Must happen now, while an overlay still has keyboard focus.
        let copied = config.copy_image
            && qobject::set_clipboard_image(
                image.as_raw(),
                image.width() as i32,
                image.height() as i32,
            );

        std::thread::spawn(move || {
            let path = naming::unique_path(
                &config.save_dir,
                &config.filename_pattern,
                "png",
                naming::now(),
            );
            match output::save_png(&image, &path) {
                Ok(()) => {
                    println!("saved {}", path.display());
                    if config.notify {
                        notify::saved(&path, copied);
                    }
                }
                Err(e) => notify::error(&format!("could not save to {}: {e}", path.display())),
            }
        });
    }
}
