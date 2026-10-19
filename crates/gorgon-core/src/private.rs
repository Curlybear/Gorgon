//! Files only the user can read: settings, history and tokens hold secrets
//! (deletion links embed upload tokens), so they don't follow the umask.

use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};

/// Creates `dir` and any missing parents as `0700`. Folders that already
/// exist keep their mode.
pub fn create_dir(dir: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

/// Writes `contents` to `path` as `0600`, creating its folder if needed.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        create_dir(dir)?;
    }
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // An existing file keeps its old mode; tighten it.
    f.set_permissions(fs::Permissions::from_mode(0o600))?;
    f.write_all(contents.as_ref())
}

/// Creates `dir` as `0700` and checks that it is a real folder owned by this
/// user that nobody else can enter. For folders in a shared place like
/// `/tmp`, where another user could have made it first.
pub fn ensure_own_dir(dir: &Path) -> std::io::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let meta = fs::symlink_metadata(dir)?;
    // SAFETY: getuid has no preconditions and can't fail.
    let uid = unsafe { libc::getuid() };
    if !meta.file_type().is_dir() || meta.uid() != uid || meta.mode() & 0o077 != 0 {
        return Err(std::io::Error::other(format!(
            "{} isn't a private folder of this user; remove it or set XDG_RUNTIME_DIR",
            dir.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn writes_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a/b/secret");
        write(&file, "x").unwrap();
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&dir.path().join("a/b")), 0o700);

        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        write(&file, "y").unwrap();
        assert_eq!(mode(&file), 0o600);
        assert_eq!(fs::read_to_string(&file).unwrap(), "y");
    }

    #[test]
    fn own_dir_must_be_private() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("ok");
        ensure_own_dir(&ok).unwrap();
        ensure_own_dir(&ok).unwrap();

        let open = dir.path().join("open");
        fs::create_dir(&open).unwrap();
        fs::set_permissions(&open, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(ensure_own_dir(&open).is_err());

        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&ok, &link).unwrap();
        assert!(ensure_own_dir(&link).is_err());
    }
}
