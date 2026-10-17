//! Imported `.sxcu` files, kept in `~/.config/gorgon/uploaders/`.
//!
//! They often hold API tokens, so the folder and files are owner-only.

use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

use crate::sxcu::{SxcuError, Uploader};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Invalid { path: PathBuf, source: SxcuError },
    #[error("no uploader named {0:?} (see `gorgon uploader list`)")]
    Missing(String),
    #[error("no config folder")]
    NoConfigDir,
}

pub fn dir() -> Result<PathBuf, StoreError> {
    dirs::config_dir()
        .map(|d| d.join("gorgon").join("uploaders"))
        .ok_or(StoreError::NoConfigDir)
}

/// Validates a `.sxcu` file and copies it into the store. Returns the
/// uploader and where it was stored. A file with the same name is replaced.
pub fn import(source: &Path) -> Result<(Uploader, PathBuf), StoreError> {
    import_into(&dir()?, source)
}

pub fn import_into(dir: &Path, source: &Path) -> Result<(Uploader, PathBuf), StoreError> {
    let text = fs::read_to_string(source).map_err(|e| StoreError::Read {
        path: source.to_owned(),
        source: e,
    })?;
    let uploader = Uploader::parse(&text).map_err(|e| StoreError::Invalid {
        path: source.to_owned(),
        source: e,
    })?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| StoreError::Write {
            path: dir.to_owned(),
            source: e,
        })?;
    let target = dir.join(format!("{}.sxcu", file_stem(&uploader.name)));
    let write = || -> std::io::Result<()> {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&target)?;
        f.write_all(text.as_bytes())?;
        // An existing file keeps its old mode; tighten it.
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
    };
    write().map_err(|e| StoreError::Write {
        path: target.clone(),
        source: e,
    })?;
    Ok((uploader, target))
}

/// A file name safe for any uploader name.
fn file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "-_. ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = stem.trim().trim_start_matches('.').to_owned();
    if stem.is_empty() {
        "uploader".into()
    } else {
        stem
    }
}

/// Every stored uploader that parses, with the files that don't.
pub fn list() -> Result<(Vec<Uploader>, Vec<StoreError>), StoreError> {
    list_in(&dir()?)
}

pub fn list_in(dir: &Path) -> Result<(Vec<Uploader>, Vec<StoreError>), StoreError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((vec![], vec![])),
        Err(e) => {
            return Err(StoreError::Read {
                path: dir.to_owned(),
                source: e,
            })
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "sxcu"))
        .collect();
    paths.sort();
    let (mut ok, mut bad) = (vec![], vec![]);
    for path in paths {
        match read(&path) {
            Ok(u) => ok.push(u),
            Err(e) => bad.push(e),
        }
    }
    Ok((ok, bad))
}

fn read(path: &Path) -> Result<Uploader, StoreError> {
    let text = fs::read_to_string(path).map_err(|e| StoreError::Read {
        path: path.to_owned(),
        source: e,
    })?;
    Uploader::parse(&text).map_err(|e| StoreError::Invalid {
        path: path.to_owned(),
        source: e,
    })
}

/// Finds a stored uploader by its name (case-insensitive).
pub fn find(name: &str) -> Result<Uploader, StoreError> {
    find_in(&dir()?, name)
}

pub fn find_in(dir: &Path, name: &str) -> Result<Uploader, StoreError> {
    let direct = dir.join(format!("{}.sxcu", file_stem(name)));
    if direct.exists() {
        return read(&direct);
    }
    list_in(dir)?
        .0
        .into_iter()
        .find(|u| u.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| StoreError::Missing(name.to_owned()))
}

/// Deletes a stored uploader by name.
pub fn remove(name: &str) -> Result<(), StoreError> {
    let dir = dir()?;
    let (ok, _) = list_in(&dir)?;
    let uploader = ok
        .iter()
        .find(|u| u.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| StoreError::Missing(name.to_owned()))?;
    let path = dir.join(format!("{}.sxcu", file_stem(&uploader.name)));
    fs::remove_file(&path).map_err(|e| StoreError::Write { path, source: e })
}

/// The `URL` template of examples/xbackbone.sxcu, which turns XBackBone's
/// preview link (host/user/CODE.ext) into the raw file's (host/user/CODE/raw.ext).
fn xbackbone_raw_url() -> String {
    let example: serde_json::Value =
        serde_json::from_str(include_str!("../../../examples/xbackbone.sxcu"))
            .expect("examples/xbackbone.sxcu is valid JSON");
    example["URL"].as_str().unwrap_or_default().to_owned()
}

/// Rewrites an XBackBone `.sxcu` so it links to raw files. Files in the
/// legacy `$json:url$` syntax are moved to the current `{json:url}` one,
/// since the new template uses it.
pub fn with_raw_links(text: &str) -> Result<String, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let url = value
        .get("URL")
        .and_then(|u| u.as_str())
        .unwrap_or_default();
    if url == xbackbone_raw_url() {
        return Err("it already links straight to the file".into());
    }
    if url != "{json:url}" && url != "$json:url$" {
        return Err("its URL isn't XBackBone's plain {json:url}, so edit it by hand".into());
    }
    let major = value
        .get("Version")
        .and_then(|v| v.as_str())
        .and_then(|v| v.split('.').next()?.parse::<u32>().ok())
        .unwrap_or(0);
    if major < 13 {
        let legacy = regex::Regex::new(r"\$(\w+(?::[^$]*)?)\$").unwrap();
        fn modernize(v: &mut serde_json::Value, re: &regex::Regex) {
            match v {
                serde_json::Value::String(s) => *s = re.replace_all(s, "{$1}").into_owned(),
                serde_json::Value::Object(map) => map.values_mut().for_each(|v| modernize(v, re)),
                _ => {}
            }
        }
        modernize(&mut value, &legacy);
        value["Version"] = "16.0.0".into();
    }
    value["URL"] = xbackbone_raw_url().into();
    let out = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())? + "\n";
    Uploader::parse(&out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Applies [`with_raw_links`] to a stored uploader.
pub fn make_raw_links(name: &str) -> Result<PathBuf, String> {
    let dir = dir().map_err(|e| e.to_string())?;
    let uploader = find_in(&dir, name).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.sxcu", file_stem(&uploader.name)));
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let out = with_raw_links(&text).map_err(|e| format!("{:?}: {e}", uploader.name))?;
    let tmp = path.with_extension("sxcu.tmp");
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .and_then(|mut f| f.write_all(out.as_bytes()))
        .and_then(|_| fs::rename(&tmp, &path))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {

    #[test]
    fn raw_links_from_legacy_xbackbone_export() {
        let legacy = r#"{"Name":"my-server","DestinationType":"ImageUploader, FileUploader","RequestType":"POST","RequestURL":"https://x.example.eu/upload","FileFormName":"upload","Arguments":{"file":"$filename$","token":"tok"},"URL":"$json:url$","DeletionURL":"$json:url$/delete/tok","ErrorMessage":"$json:message$"}"#;
        let out = super::with_raw_links(legacy).unwrap();
        let up = crate::sxcu::Uploader::parse(&out).unwrap();
        let ctx = crate::sxcu::Context {
            response: Some(crate::sxcu::Response {
                body: r#"{"url":"https:\/\/x.example.eu\/me\/AbC.png"}"#.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let render = |t: &crate::sxcu::Template| t.render(&ctx, crate::sxcu::Escape::None).unwrap();
        assert_eq!(
            render(up.url.as_ref().unwrap()),
            "https://x.example.eu/me/AbC/raw.png"
        );
        assert_eq!(
            render(up.deletion_url.as_ref().unwrap()),
            "https://x.example.eu/me/AbC.png/delete/tok"
        );
        assert!(super::with_raw_links(&out).is_err(), "already patched");
    }
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn import_list_find() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("kutt.sxcu");
        fs::write(
            &src,
            r#"{"Version":"16.0.0","Name":"My/Kutt","DestinationType":"URLShortener","RequestURL":"https://k.example.com/api/v2/links","Body":"JSON","Data":"{\"target\":\"{input}\"}","URL":"{json:link}"}"#,
        )
        .unwrap();
        let store = tmp.path().join("store");
        let (up, path) = import_into(&store, &src).unwrap();
        assert_eq!(up.name, "My/Kutt");
        assert_eq!(path.file_name().unwrap(), "My_Kutt.sxcu");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(find_in(&store, "my/kutt").unwrap().name, "My/Kutt");
        assert!(matches!(
            find_in(&store, "nope"),
            Err(StoreError::Missing(_))
        ));
        assert_eq!(list_in(&store).unwrap().0.len(), 1);
    }

    #[test]
    fn invalid_files_are_not_imported() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("bad.sxcu");
        fs::write(&src, "{ not json").unwrap();
        let store = tmp.path().join("store");
        assert!(matches!(
            import_into(&store, &src),
            Err(StoreError::Invalid { .. })
        ));
        assert!(!store.exists());
    }
}
