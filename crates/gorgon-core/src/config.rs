//! User settings, read from `~/.config/gorgon/config.toml`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Folder captures are saved in.
    pub save_dir: PathBuf,
    /// File name without extension, in strftime format.
    pub filename_pattern: String,
    /// Copy the captured image to the clipboard.
    pub copy_image: bool,
    /// Show a notification after each capture.
    pub notify: bool,
    /// Open the editor after every capture, not only for "and edit" captures.
    pub open_editor: bool,
    /// Upload captures with `image_uploader`.
    pub upload: bool,
    /// Name of the imported `.sxcu` uploader for captures. None: don't upload.
    pub image_uploader: Option<String>,
    /// Shorten uploaded links with `url_shortener`.
    pub shorten: bool,
    /// Name of the imported `.sxcu` URL shortener. None: keep the long link.
    pub url_shortener: Option<String>,
    /// Copy the final link to the clipboard (it replaces the copied image).
    pub copy_url: bool,
    /// Frames per second for MP4 recordings.
    pub video_fps: u32,
    /// Frames per second for GIF recordings.
    pub gif_fps: u32,
}

impl Default for Config {
    fn default() -> Self {
        let pictures = dirs::picture_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join("Pictures")))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            save_dir: pictures.join("Gorgon"),
            filename_pattern: "%Y-%m-%d_%H-%M-%S".into(),
            copy_image: true,
            notify: true,
            open_editor: false,
            upload: true,
            image_uploader: None,
            shorten: true,
            url_shortener: None,
            copy_url: true,
            video_fps: 30,
            gif_fps: 15,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid settings in {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

impl Config {
    /// Where the settings file lives.
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("gorgon").join("config.toml"))
    }

    /// Loads the settings file, or the defaults when there is none.
    pub fn load() -> Result<Self, ConfigError> {
        match Self::path() {
            Some(path) if path.exists() => Self::load_from(&path),
            _ => Ok(Self::default()),
        }
    }

    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_owned(),
            source,
        })?;
        let mut config: Self = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_owned(),
            source,
        })?;
        config.save_dir = expand_home(&config.save_dir);
        Ok(config)
    }
}

/// Sets one key in the settings file, keeping the rest of it (comments
/// included) as written. Creates the file if needed.
pub fn set_value(key: &str, value: impl Into<toml_edit::Value>) -> Result<PathBuf, ConfigError> {
    set_values(vec![(key.to_owned(), Some(value.into()))])
}

/// Sets several keys at once; `None` removes a key (back to its default).
pub fn set_values(values: Vec<(String, Option<toml_edit::Value>)>) -> Result<PathBuf, ConfigError> {
    let path = Config::path().ok_or_else(|| ConfigError::Read {
        path: PathBuf::from("~/.config/gorgon/config.toml"),
        source: std::io::Error::other("no config folder"),
    })?;
    set_values_in(&path, values)?;
    Ok(path)
}

pub fn set_value_in(
    path: &Path,
    key: &str,
    value: impl Into<toml_edit::Value>,
) -> Result<(), ConfigError> {
    set_values_in(path, vec![(key.to_owned(), Some(value.into()))])
}

pub fn set_values_in(
    path: &Path,
    values: Vec<(String, Option<toml_edit::Value>)>,
) -> Result<(), ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_owned(),
                source,
            })
        }
    };
    let mut doc: toml_edit::DocumentMut =
        text.parse()
            .map_err(|e: toml_edit::TomlError| ConfigError::Read {
                path: path.to_owned(),
                source: std::io::Error::other(e.to_string()),
            })?;
    for (key, value) in values {
        match value {
            Some(v) => doc[key.as_str()] = toml_edit::value(v),
            None => {
                doc.remove(&key);
            }
        }
    }
    let write = || {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, doc.to_string())
    };
    write().map_err(|source| ConfigError::Read {
        path: path.to_owned(),
        source,
    })
}

/// Expands a leading `~/` so settings can be written the way people type paths.
fn expand_home(path: &Path) -> PathBuf {
    match (path.strip_prefix("~"), dirs::home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_use_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "notify = false\n").unwrap();
        let config = Config::load_from(&path).unwrap();
        assert!(!config.notify);
        assert_eq!(config.filename_pattern, Config::default().filename_pattern);
    }

    #[test]
    fn tilde_is_expanded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "save_dir = \"~/Shots\"\n").unwrap();
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.save_dir, dirs::home_dir().unwrap().join("Shots"));
    }

    #[test]
    fn set_value_keeps_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# mine\nnotify = false\n").unwrap();
        set_value_in(&path, "image_uploader", "XBackBone").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# mine\nnotify = false\n"));
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.image_uploader.as_deref(), Some("XBackBone"));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "save_folder = \"/tmp\"\n").unwrap();
        assert!(matches!(
            Config::load_from(&path),
            Err(ConfigError::Parse { .. })
        ));
    }
}
