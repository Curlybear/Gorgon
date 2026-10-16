//! Starting Gorgon at login through an XDG autostart entry
//! (`~/.config/autostart/gorgon.desktop`), which GNOME, KDE and most other
//! desktops run when the session starts.

use std::io;
use std::path::PathBuf;

fn entry_path() -> io::Result<PathBuf> {
    dirs::config_dir()
        .map(|d| d.join("autostart").join("gorgon.desktop"))
        .ok_or_else(|| io::Error::other("no config folder"))
}

/// Whether Gorgon starts at login: the entry exists and isn't switched off.
pub fn enabled() -> bool {
    let Ok(text) = entry_path().and_then(std::fs::read_to_string) else {
        return false;
    };
    !text.lines().any(|l| {
        let l = l.replace(' ', "");
        l == "Hidden=true" || l == "X-GNOME-Autostart-enabled=false"
    })
}

/// Adds or removes the entry. It runs this very binary, so it keeps
/// working wherever Gorgon was installed.
pub fn set(on: bool) -> io::Result<PathBuf> {
    let path = entry_path()?;
    if !on {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => return Ok(path),
        }
    }
    let exe = std::env::current_exe()?;
    let exe = exe
        .to_str()
        .ok_or_else(|| io::Error::other("Gorgon's path isn't UTF-8"))?;
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Gorgon\n\
         Comment=Screenshots, recordings and uploads\n\
         Exec={} daemon\n\
         Icon=camera-photo\n\
         Terminal=false\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n",
        quote(exe)
    );
    std::fs::create_dir_all(path.parent().unwrap_or(&path))?;
    std::fs::write(&path, entry)?;
    Ok(path)
}

/// Quotes an Exec argument per the desktop entry spec.
fn quote(arg: &str) -> String {
    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c))
    {
        return arg.to_owned();
    }
    let mut out = String::from("\"");
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    // `%` is a field code in Exec, even inside quotes.
    out.replace('%', "%%")
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn quotes_exec_paths() {
        assert_eq!(
            quote("/home/me/.cargo/bin/gorgon"),
            "/home/me/.cargo/bin/gorgon"
        );
        assert_eq!(quote("/opt/my apps/gorgon"), "\"/opt/my apps/gorgon\"");
        assert_eq!(quote("/a/$b%"), "\"/a/\\$b%%\"");
    }
}
