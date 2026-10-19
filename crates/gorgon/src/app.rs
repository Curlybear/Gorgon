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
        fn set_clipboard_text(text: &str);
        fn keep_running_without_windows();
        fn set_app_identity(desktop_file: &str, icon_path: &str);
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QUrl, image_url, cxx_name = "imageUrl")]
        #[qproperty(i32, image_width, cxx_name = "imageWidth")]
        #[qproperty(i32, image_height, cxx_name = "imageHeight")]
        /// Set by QML: screenshot pixels per logical pixel, and the desktop's
        /// top-left corner, to place a recorded region on its monitor.
        #[qproperty(f64, image_scale, cxx_name = "imageScale")]
        #[qproperty(f64, origin_x, cxx_name = "originX")]
        #[qproperty(f64, origin_y, cxx_name = "originY")]
        /// True while a region is being picked for a recording.
        #[qproperty(bool, for_recording, cxx_name = "forRecording")]
        type App = super::AppRust;

        /// Show the frozen screenshot and let the user drag a region.
        #[qsignal]
        #[cxx_name = "selectRegion"]
        fn select_region(self: Pin<&mut App>);

        /// Flash the screens to confirm a full capture (and take focus for the clipboard).
        #[qsignal]
        fn flash(self: Pin<&mut App>);

        /// A capture is waiting for an editor window (see `take_pending_edit`).
        #[qsignal]
        #[cxx_name = "openEditor"]
        fn open_editor(self: Pin<&mut App>);

        /// The capture history changed on disk.
        #[qsignal]
        #[cxx_name = "historyUpdated"]
        fn history_updated(self: Pin<&mut App>);

        /// Asks QML to open the History or Settings window.
        #[qsignal]
        #[cxx_name = "showWindow"]
        fn show_window(self: Pin<&mut App>, name: &QString);

        /// Text is waiting to be copied: QML shows a tiny window to get the
        /// focus the clipboard needs, then calls `copyPending`.
        #[qsignal]
        #[cxx_name = "copyRequested"]
        fn copy_requested(self: Pin<&mut App>);

        /// Copies the waiting text. Call while a Gorgon window has focus.
        #[qinvokable]
        #[cxx_name = "copyPending"]
        fn copy_pending(self: Pin<&mut App>);

        /// No window got focus in time; the text wasn't copied.
        #[qinvokable]
        #[cxx_name = "copyFailed"]
        fn copy_failed(self: Pin<&mut App>);

        /// Starts a capture: "region", "screen" or "window".
        #[qinvokable]
        #[cxx_name = "startCapture"]
        fn start_capture(self: Pin<&mut App>, mode: &QString, edit: bool);

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

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{mpsc, Mutex, OnceLock},
    time::Duration,
};

use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};
use gorgon_core::{
    capture::{self, CaptureError},
    config::Config,
    history, naming,
    output::{self, Rect},
    record::Format,
    CaptureMode,
};
use image::RgbaImage;

use crate::notify;

/// The UI thread handle, set once the QML engine has created the App.
static QT: OnceLock<CxxQtThread<qobject::App>> = OnceLock::new();

/// Captures waiting for an editor window to pick them up.
static PENDING_EDITS: Mutex<VecDeque<(RgbaImage, Config)>> = Mutex::new(VecDeque::new());

fn queue_on_ui(f: impl FnOnce(Pin<&mut qobject::App>) + Send + 'static) -> Result<(), String> {
    let qt = QT.get().ok_or("Gorgon is still starting")?;
    qt.queue(f)
        .map_err(|e| format!("Gorgon's UI is not running: {e:?}"))
}

/// Asks the UI thread to start a capture. Safe to call from any thread.
/// Names Gorgon's desktop entry and icon for its windows (docks, Alt+Tab).
pub fn set_identity() {
    let icon = crate::icons::path().unwrap_or_default();
    qobject::set_app_identity(crate::icons::NAME, &icon.to_string_lossy());
}

pub fn request_capture(mode: CaptureMode, edit: bool) -> Result<(), String> {
    queue_on_ui(move |app| app.start(mode, edit))
}

/// Picks a region on a frozen screenshot, then records it.
pub fn request_record(format: Format) -> Result<(), String> {
    queue_on_ui(move |app| app.start_record(format))
}

/// Opens the "history" or "settings" window. Safe to call from any thread.
pub fn request_window(name: &'static str) -> Result<(), String> {
    queue_on_ui(move |app| app.show_window(&QString::from(name)))
}

/// Opens an existing image in the editor. Safe to call from any thread.
pub fn request_edit_file(path: PathBuf) -> Result<(), String> {
    let image = image::open(&path)
        .map_err(|e| format!("could not open {}: {e}", path.display()))?
        .into_rgba8();
    let config = Config::load().unwrap_or_default();
    queue_on_ui(move |app| app.edit(image, config))
}

/// The next capture waiting for an editor, if any.
pub fn take_pending_edit() -> Option<(RgbaImage, Config)> {
    PENDING_EDITS.lock().ok()?.pop_front()
}

/// Copies to the clipboard. Only works while a Gorgon window has focus.
pub fn copy_to_clipboard(image: &RgbaImage) -> bool {
    qobject::set_clipboard_image(image.as_raw(), image.width() as i32, image.height() as i32)
}

/// Copies text right away. Only works while a Gorgon window has focus.
pub fn copy_text_now(text: &str) {
    qobject::set_clipboard_text(text);
}

/// Text waiting for a focused window to be copied, and who wants to know.
static PENDING_COPY: Mutex<Option<(String, mpsc::Sender<bool>)>> = Mutex::new(None);

/// Copies text from any thread, briefly showing a window to get focus.
/// Blocks until it's done or has failed.
pub fn copy_text(text: &str) -> bool {
    let (tx, rx) = mpsc::channel();
    if let Ok(mut pending) = PENDING_COPY.lock() {
        if let Some((_, old)) = pending.replace((text.to_owned(), tx)) {
            let _ = old.send(false);
        }
    }
    if queue_on_ui(|app| app.copy_requested()).is_err() {
        return false;
    }
    rx.recv_timeout(Duration::from_secs(3)).unwrap_or(false)
}

fn finish_copy(copy: bool) {
    let Some((text, tx)) = PENDING_COPY.lock().ok().and_then(|mut p| p.take()) else {
        return;
    };
    if copy {
        qobject::set_clipboard_text(&text);
    }
    let _ = tx.send(copy);
}

/// Saves the final image, uploads it if set up, and tells the user. Runs
/// off the UI thread.
pub fn deliver(image: RgbaImage, config: Config, copied: bool) {
    std::thread::spawn(move || {
        let path = naming::unique_path(
            &config.save_dir,
            &config.filename_pattern,
            "png",
            naming::now(),
        );
        if let Err(e) = output::save_png(&image, &path) {
            notify::error(&format!("could not save to {}: {e}", path.display()));
            return;
        }
        println!("saved {}", path.display());
        let entry = history::Entry::new(&path, image.width(), image.height());
        let id = history::add(entry)
            .inspect_err(|e| eprintln!("history: {e}"))
            .ok();
        history_changed();
        upload_saved(path, config, copied, id);
    });
}

/// Uploads a saved capture per the settings and notifies. Blocks.
pub fn upload_saved(path: PathBuf, config: Config, copied_image: bool, id: Option<u64>) {
    let result = crate::pipeline::upload(&path, &config);
    if let (Some(id), Some(result)) = (id, &result) {
        let uploader = config.image_uploader.clone();
        let recorded = history::update(id, |e| match result {
            Ok(links) => {
                e.url = Some(links.url.clone());
                e.deletion_url = links.deletion_url.clone();
                e.uploader = uploader;
                e.error = None;
            }
            Err(err) => e.error = Some(err.clone()),
        });
        if let Err(e) = recorded {
            eprintln!("history: {e}");
        }
        history_changed();
    }
    match result {
        None => {
            if config.notify {
                notify::saved(&path, copied_image);
            }
        }
        Some(Ok(links)) => {
            println!("uploaded {}", links.url);
            if let Some(e) = &links.shorten_error {
                eprintln!("shortening failed: {e}");
            }
            let copied = config.copy_url && copy_text(&links.url);
            if config.copy_url && !copied {
                eprintln!("could not copy the link: no Gorgon window got focus");
            }
            if config.notify {
                notify::uploaded(&path, &links, copied);
            }
        }
        Some(Err(e)) => notify::upload_failed(&path, &e, id),
    }
}

/// Tells open History windows to reload.
pub fn history_changed() {
    let _ = queue_on_ui(|app| app.history_updated());
}

/// Removes a file, ignoring "already gone".
fn discard(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[derive(Default)]
pub struct AppRust {
    image_url: QUrl,
    image_width: i32,
    image_height: i32,
    image_scale: f64,
    origin_x: f64,
    origin_y: f64,
    for_recording: bool,
    /// Record the picked region instead of saving a screenshot.
    record: Option<Format>,
    busy: bool,
    config: Config,
    /// Screenshot being worked on, in Gorgon's runtime folder.
    current: Option<PathBuf>,
    /// Open the editor once the current capture is taken.
    edit: bool,
}

impl cxx_qt::Initialize for qobject::App {
    fn initialize(self: Pin<&mut Self>) {
        qobject::keep_running_without_windows();
        let _ = QT.set(self.qt_thread());
    }
}

impl qobject::App {
    pub fn start_capture(self: Pin<&mut Self>, mode: &QString, edit: bool) {
        match mode.to_string().parse() {
            Ok(mode) => self.start(mode, edit),
            Err(e) => eprintln!("{e}"),
        }
    }

    fn edit(self: Pin<&mut Self>, image: RgbaImage, config: Config) {
        if let Ok(mut pending) = PENDING_EDITS.lock() {
            pending.push_back((image, config));
        }
        self.open_editor();
    }

    fn start_record(mut self: Pin<&mut Self>, format: Format) {
        if self.busy {
            eprintln!("a capture is already in progress");
            return;
        }
        self.as_mut().rust_mut().record = Some(format);
        self.as_mut().set_for_recording(true);
        self.start(CaptureMode::Region, false);
    }

    fn start(mut self: Pin<&mut Self>, mode: CaptureMode, edit: bool) {
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
            rust.edit = edit || config.open_editor;
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
                self.as_mut().rust_mut().record = None;
                self.as_mut().set_for_recording(false);
                return;
            }
        };
        let (w, h) = match image::image_dimensions(&path) {
            Ok(dims) => dims,
            Err(e) => {
                notify::error(&format!("unreadable screenshot: {e}"));
                discard(&path);
                self.as_mut().rust_mut().busy = false;
                return;
            }
        };
        self.as_mut().set_image_width(w as i32);
        self.as_mut().set_image_height(h as i32);
        self.as_mut()
            .set_image_url(QUrl::from(&gorgon_core::capture::path_to_file_uri(&path)));
        self.as_mut().rust_mut().current = Some(path);
        match mode {
            CaptureMode::Region => self.select_region(),
            CaptureMode::Screen | CaptureMode::Window => self.flash(),
        }
    }

    pub fn copy_pending(self: Pin<&mut Self>) {
        finish_copy(true);
    }

    pub fn copy_failed(self: Pin<&mut Self>) {
        finish_copy(false);
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
        {
            let mut rust = self.as_mut().rust_mut();
            if let Some(path) = rust.current.take() {
                discard(&path);
            }
            rust.busy = false;
            rust.record = None;
        }
        self.set_for_recording(false);
    }

    fn finish(mut self: Pin<&mut Self>, rect: Option<Rect>) {
        let (source, config, edit) = {
            let mut rust = self.as_mut().rust_mut();
            rust.busy = false;
            (rust.current.take(), rust.config.clone(), rust.edit)
        };
        let Some(source) = source else {
            return;
        };
        if let Some(format) = self.as_mut().rust_mut().record.take() {
            self.as_mut().set_for_recording(false);
            discard(&source);
            let Some(region) = rect else { return };
            let desktop = crate::recorder::Desktop {
                scale: if self.image_scale > 0.0 {
                    self.image_scale
                } else {
                    1.0
                },
                origin: (self.origin_x, self.origin_y),
            };
            crate::recorder::start(format, region, desktop, config);
            return;
        }
        let image = output::load(&source, rect);
        discard(&source);
        let image = match image {
            Ok(image) => image,
            Err(e) => {
                notify::error(&format!("could not read the screenshot: {e}"));
                return;
            }
        };
        if edit {
            self.edit(image, config);
            return;
        }
        // Must happen now, while an overlay still has keyboard focus.
        let copied = config.copy_image && copy_to_clipboard(&image);
        deliver(image, config, copied);
    }
}
