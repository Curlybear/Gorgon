//! The app icon: installed into the user's icon theme and a launcher entry
//! at startup, so the tray, notifications, windows and app grid can find it
//! by name ("gorgon") wherever Gorgon was installed from.

use std::path::{Path, PathBuf};

pub const NAME: &str = "gorgon";

const SIZES: &[(u32, &[u8])] = &[
    (16, include_bytes!("../../../assets/icons/gorgon-16.png")),
    (22, include_bytes!("../../../assets/icons/gorgon-22.png")),
    (24, include_bytes!("../../../assets/icons/gorgon-24.png")),
    (32, include_bytes!("../../../assets/icons/gorgon-32.png")),
    (48, include_bytes!("../../../assets/icons/gorgon-48.png")),
    (64, include_bytes!("../../../assets/icons/gorgon-64.png")),
    (128, include_bytes!("../../../assets/icons/gorgon-128.png")),
    (256, include_bytes!("../../../assets/icons/gorgon-256.png")),
    (512, include_bytes!("../../../assets/icons/gorgon-512.png")),
];

fn data_dir() -> Option<PathBuf> {
    dirs::data_dir()
}

/// The installed 256 px icon, for places that want a file.
pub fn path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("icons/hicolor/256x256/apps/gorgon.png"))
}

/// Writes `bytes` unless the file already holds them.
fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, bytes)?;
    Ok(true)
}

/// Installs the icons and `~/.local/share/applications/gorgon.desktop`.
/// The launcher entry opens History and names the windows' app id, so
/// docks show the icon too. Packages (installed under /usr) ship both
/// themselves, so this only runs for `cargo install` and AppImages.
pub fn install() {
    let Ok(exe) = crate::self_exe() else { return };
    if exe.starts_with("/usr/") {
        return;
    }
    let Some(data) = data_dir() else { return };
    let hicolor = data.join("icons/hicolor");
    let mut changed = false;
    for (size, bytes) in SIZES {
        let path = hicolor.join(format!("{size}x{size}/apps/gorgon.png"));
        match write_if_changed(&path, bytes) {
            Ok(c) => changed |= c,
            Err(e) => eprintln!("could not install the icon to {}: {e}", path.display()),
        }
    }
    if changed {
        // Theme caches notice new icons by the folder's modification time.
        let _ = std::fs::File::open(&hicolor)
            .and_then(|f| f.set_modified(std::time::SystemTime::now()));
    }

    let Some(exe) = exe.to_str() else { return };
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Gorgon\n\
         GenericName=Screenshot tool\n\
         Comment=Screenshots, recordings and uploads\n\
         Exec={} history\n\
         Icon={NAME}\n\
         Terminal=false\n\
         Categories=Utility;Graphics;\n\
         Keywords=screenshot;capture;record;upload;\n\
         StartupWMClass=gorgon\n\
         Actions=capture-region;settings;\n\
         \n\
         [Desktop Action capture-region]\n\
         Name=Capture region\n\
         Exec={0} capture region\n\
         \n\
         [Desktop Action settings]\n\
         Name=Settings\n\
         Exec={0} settings\n",
        crate::autostart::quote(exe)
    );
    let path = data.join("applications/gorgon.desktop");
    if let Err(e) = write_if_changed(&path, entry.as_bytes()) {
        eprintln!("could not write {}: {e}", path.display());
    }
}

/// ARGB32 pixmaps for the tray, which then needs no icon theme lookup.
pub fn tray_pixmaps() -> Vec<ksni::Icon> {
    SIZES
        .iter()
        .filter(|(size, _)| [22, 32, 48, 64].contains(size))
        .filter_map(|(_, bytes)| {
            let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .ok()?
                .into_rgba8();
            let (width, height) = img.dimensions();
            let data = img
                .pixels()
                .flat_map(|p| [p[3], p[0], p[1], p[2]])
                .collect();
            Some(ksni::Icon {
                width: width as i32,
                height: height as i32,
                data,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn tray_pixmaps_decode() {
        let icons = super::tray_pixmaps();
        assert_eq!(icons.len(), 4);
        for icon in icons {
            assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
        }
    }
}
