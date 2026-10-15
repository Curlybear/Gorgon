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
