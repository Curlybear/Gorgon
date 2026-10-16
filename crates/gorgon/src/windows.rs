//! Models behind the History and Settings windows.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// The history as a JSON array, newest first.
        #[qproperty(QString, entries)]
        type HistoryModel = super::HistoryRust;

        /// Reads the history file again.
        #[qinvokable]
        fn reload(self: Pin<&mut HistoryModel>);

        /// Removes an entry from the history; the file stays.
        #[qinvokable]
        fn forget(self: Pin<&mut HistoryModel>, id: &QString);

        /// Removes an entry and deletes its file.
        #[qinvokable]
        #[cxx_name = "deleteFile"]
        fn delete_file(self: Pin<&mut HistoryModel>, id: &QString);

        /// Uploads (again) with the default uploader.
        #[qinvokable]
        fn upload(self: Pin<&mut HistoryModel>, id: &QString);

        #[qinvokable]
        fn edit(self: Pin<&mut HistoryModel>, id: &QString);

        /// Copies text; the History window has focus, so this works directly.
        #[qinvokable]
        fn copy(self: Pin<&mut HistoryModel>, text: &QString);
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, save_dir, cxx_name = "saveDir")]
        #[qproperty(QString, filename_pattern, cxx_name = "filenamePattern")]
        #[qproperty(bool, copy_image, cxx_name = "copyImage")]
        #[qproperty(bool, notify)]
        #[qproperty(bool, open_editor, cxx_name = "openEditor")]
        #[qproperty(bool, upload)]
        #[qproperty(bool, shorten)]
        #[qproperty(bool, copy_url, cxx_name = "copyUrl")]
        #[qproperty(bool, autostart)]
        /// Empty for none.
        #[qproperty(QString, image_uploader, cxx_name = "imageUploader")]
        #[qproperty(QString, url_shortener, cxx_name = "urlShortener")]
        /// Imported uploaders as JSON: [{name, uploads, shortens}].
        #[qproperty(QString, uploaders)]
        /// Feedback for the user after an action.
        #[qproperty(QString, message)]
        #[qproperty(bool, message_is_error, cxx_name = "messageIsError")]
        type SettingsModel = super::SettingsRust;

        /// Reads the settings file and the imported uploaders.
        #[qinvokable]
        fn load(self: Pin<&mut SettingsModel>);

        /// Writes the settings file. False (with `message`) if something is invalid.
        #[qinvokable]
        fn save(self: Pin<&mut SettingsModel>) -> bool;

        /// Imports a `.sxcu` file, from a path or a file:// URL.
        #[qinvokable]
        #[cxx_name = "importFile"]
        fn import_file(self: Pin<&mut SettingsModel>, path: &QString);

        /// Removes an imported uploader.
        #[qinvokable]
        #[cxx_name = "removeUploader"]
        fn remove_uploader(self: Pin<&mut SettingsModel>, name: &QString);

        /// The file name a capture taken now would get with this pattern.
        #[qinvokable]
        fn preview(self: &SettingsModel, pattern: &QString) -> QString;
    }

    impl cxx_qt::Initialize for HistoryModel {}
    impl cxx_qt::Initialize for SettingsModel {}
}

use std::{path::PathBuf, pin::Pin};

use cxx_qt_lib::QString;
use gorgon_core::{
    capture::file_uri_to_path,
    config::{self, Config},
    history, naming, uploaders,
};

#[derive(Default)]
pub struct HistoryRust {
    entries: QString,
}

impl cxx_qt::Initialize for qobject::HistoryModel {
    fn initialize(self: Pin<&mut Self>) {
        self.reload();
    }
}

fn parse_id(id: &QString) -> Option<u64> {
    id.to_string().parse().ok()
}

impl qobject::HistoryModel {
    pub fn reload(self: Pin<&mut Self>) {
        let entries: Vec<serde_json::Value> = history::load()
            .into_iter()
            .rev()
            .map(|e| {
                let ext = e
                    .path
                    .extension()
                    .map(|x| x.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                serde_json::json!({
                    "id": e.id.to_string(),
                    "time": e.time,
                    "path": e.path,
                    "name": e.path.file_name().map(|n| n.to_string_lossy()),
                    "fileUrl": format!("file://{}", e.path.display()),
                    "folderUrl": e.path.parent().map(|d| format!("file://{}", d.display())),
                    "exists": e.path.exists(),
                    "video": ext == "mp4" || ext == "webm",
                    "editable": ext == "png" || ext == "jpg" || ext == "jpeg",
                    "width": e.width,
                    "height": e.height,
                    "url": e.url,
                    "deletionUrl": e.deletion_url,
                    "uploader": e.uploader,
                    "error": e.error,
                })
            })
            .collect();
        let json = serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into());
        self.set_entries(QString::from(&json));
    }

    pub fn forget(self: Pin<&mut Self>, id: &QString) {
        if let Some(id) = parse_id(id) {
            if let Err(e) = history::remove(id) {
                crate::notify::error(&e.to_string());
            }
        }
        self.reload();
    }

    pub fn delete_file(self: Pin<&mut Self>, id: &QString) {
        if let Some(entry) = parse_id(id).and_then(history::get) {
            match std::fs::remove_file(&entry.path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    crate::notify::error(&format!(
                        "could not delete {}: {e}",
                        entry.path.display()
                    ));
                    return;
                }
            }
            let _ = history::remove(entry.id);
        }
        self.reload();
    }

    pub fn upload(self: Pin<&mut Self>, id: &QString) {
        let Some(entry) = parse_id(id).and_then(history::get) else {
            return;
        };
        let mut config = Config::load().unwrap_or_default();
        if config.image_uploader.is_none() {
            crate::notify::error("no uploader set: import one in Settings");
            return;
        }
        // Asked for explicitly, so upload even if automatic uploads are off.
        config.upload = true;
        std::thread::spawn(move || {
            crate::app::upload_saved(entry.path, config, false, Some(entry.id));
        });
    }

    pub fn edit(self: Pin<&mut Self>, id: &QString) {
        if let Some(entry) = parse_id(id).and_then(history::get) {
            if let Err(e) = crate::app::request_edit_file(entry.path) {
                crate::notify::error(&e);
            }
        }
    }

    pub fn copy(self: Pin<&mut Self>, text: &QString) {
        crate::app::copy_text_now(&text.to_string());
    }
}

#[derive(Default)]
pub struct SettingsRust {
    save_dir: QString,
    filename_pattern: QString,
    copy_image: bool,
    notify: bool,
    open_editor: bool,
    upload: bool,
    shorten: bool,
    copy_url: bool,
    autostart: bool,
    image_uploader: QString,
    url_shortener: QString,
    uploaders: QString,
    message: QString,
    message_is_error: bool,
}

impl cxx_qt::Initialize for qobject::SettingsModel {
    fn initialize(self: Pin<&mut Self>) {
        self.load();
    }
}

/// Shows `~` for the home folder, the way people write paths.
fn tilde(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|h| path.strip_prefix(h).ok().map(PathBuf::from)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

impl qobject::SettingsModel {
    fn tell(mut self: Pin<&mut Self>, message: &str, error: bool) {
        self.as_mut().set_message_is_error(error);
        self.set_message(QString::from(message));
    }

    fn load_uploaders(mut self: Pin<&mut Self>) {
        let list: Vec<serde_json::Value> = uploaders::list()
            .map(|(ok, _)| ok)
            .unwrap_or_default()
            .iter()
            .map(|u| {
                serde_json::json!({
                    "name": u.name,
                    "uploads": u.uploads_files(),
                    "shortens": u.shortens_urls(),
                })
            })
            .collect();
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
        self.as_mut().set_uploaders(QString::from(&json));
    }

    pub fn load(mut self: Pin<&mut Self>) {
        let config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                self.as_mut().tell(&format!("{e}. Showing defaults."), true);
                Config::default()
            }
        };
        self.as_mut()
            .set_save_dir(QString::from(&tilde(&config.save_dir)));
        self.as_mut()
            .set_filename_pattern(QString::from(&config.filename_pattern));
        self.as_mut().set_copy_image(config.copy_image);
        self.as_mut().set_notify(config.notify);
        self.as_mut().set_open_editor(config.open_editor);
        self.as_mut().set_upload(config.upload);
        self.as_mut().set_shorten(config.shorten);
        self.as_mut().set_copy_url(config.copy_url);
        self.as_mut().set_autostart(crate::autostart::enabled());
        self.as_mut().set_image_uploader(QString::from(
            config.image_uploader.as_deref().unwrap_or(""),
        ));
        self.as_mut()
            .set_url_shortener(QString::from(config.url_shortener.as_deref().unwrap_or("")));
        self.load_uploaders();
    }

    pub fn save(self: Pin<&mut Self>) -> bool {
        let save_dir = self.save_dir.to_string().trim().to_owned();
        let pattern = self.filename_pattern.to_string().trim().to_owned();
        if save_dir.is_empty() {
            self.tell("Choose a folder for captures.", true);
            return false;
        }
        if pattern.is_empty() || pattern.contains('/') {
            self.tell("The file name pattern can't be empty or contain /.", true);
            return false;
        }
        let text = |s: &QString| {
            let s = s.to_string();
            (!s.is_empty()).then(|| toml_edit::Value::from(s))
        };
        let values = vec![
            ("save_dir".to_owned(), Some(save_dir.into())),
            ("filename_pattern".to_owned(), Some(pattern.into())),
            ("copy_image".to_owned(), Some(self.copy_image.into())),
            ("notify".to_owned(), Some(self.notify.into())),
            ("open_editor".to_owned(), Some(self.open_editor.into())),
            ("upload".to_owned(), Some(self.upload.into())),
            ("shorten".to_owned(), Some(self.shorten.into())),
            ("copy_url".to_owned(), Some(self.copy_url.into())),
            ("image_uploader".to_owned(), text(&self.image_uploader)),
            ("url_shortener".to_owned(), text(&self.url_shortener)),
        ];
        if self.autostart != crate::autostart::enabled() {
            if let Err(e) = crate::autostart::set(self.autostart) {
                self.tell(&format!("Couldn't change starting at login: {e}"), true);
                return false;
            }
        }
        match config::set_values(values) {
            Ok(path) => {
                self.tell(&format!("Saved to {}", tilde(&path)), false);
                true
            }
            Err(e) => {
                self.tell(&e.to_string(), true);
                false
            }
        }
    }

    pub fn import_file(mut self: Pin<&mut Self>, path: &QString) {
        let path = path.to_string();
        let path = path.trim();
        let path = if path.starts_with("file://") {
            file_uri_to_path(path)
        } else if let Some(rest) = path.strip_prefix("~/") {
            dirs::home_dir().map(|h| h.join(rest))
        } else {
            Some(PathBuf::from(path))
        };
        let Some(path) = path else {
            self.tell("That isn't a file path.", true);
            return;
        };
        match uploaders::import(&path) {
            Ok((u, _)) => {
                let mut picked = vec![];
                if u.uploads_files() && self.image_uploader.is_empty() {
                    self.as_mut().set_image_uploader(QString::from(&u.name));
                    picked.push("captures");
                }
                if u.shortens_urls() && self.url_shortener.is_empty() {
                    self.as_mut().set_url_shortener(QString::from(&u.name));
                    picked.push("links");
                }
                let mut msg = format!("Imported {}.", u.name);
                if !picked.is_empty() {
                    msg.push_str(&format!(
                        " Now used for {} (Save to keep).",
                        picked.join(" and ")
                    ));
                }
                if !u.uploads_files() && !u.shortens_urls() {
                    msg = format!(
                        "Imported {}, but it neither uploads files nor shortens links.",
                        u.name
                    );
                }
                self.as_mut().load_uploaders();
                self.tell(&msg, false);
            }
            Err(e) => self.tell(&e.to_string(), true),
        }
    }

    pub fn remove_uploader(mut self: Pin<&mut Self>, name: &QString) {
        let name = name.to_string();
        match uploaders::remove(&name) {
            Ok(()) => {
                if self.image_uploader.to_string() == name {
                    self.as_mut().set_image_uploader(QString::default());
                }
                if self.url_shortener.to_string() == name {
                    self.as_mut().set_url_shortener(QString::default());
                }
                self.as_mut().load_uploaders();
                self.tell(&format!("Removed {name}."), false);
            }
            Err(e) => self.tell(&e.to_string(), true),
        }
    }

    pub fn preview(&self, pattern: &QString) -> QString {
        let pattern = pattern.to_string();
        let path = naming::unique_path(std::path::Path::new(""), &pattern, "png", naming::now());
        QString::from(&path.display().to_string())
    }
}
