//! Wayland resolves app_id through desktop entries, not window icon pixels.
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn register() -> io::Result<()> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".local/share"))
        })
        .ok_or_else(|| io::Error::other("Cannot locate user desktop data directory"))?;
    let png = include_bytes!(concat!(env!("OUT_DIR"), "/enlumi.png"));
    use sha2::{Digest, Sha256};
    // New filename on artwork changes prevents stale desktop icon caches.
    let digest = format!("{:x}", Sha256::digest(png));
    let icon = data
        .join("icons/hicolor/256x256/apps")
        .join(format!("com.tealdrones.enlumi-{digest}.png"));
    write_changed(&icon, png)?;
    // Shells may resolve app_id directly instead of desktop entry's absolute Icon.
    let canonical = data.join("icons/hicolor/256x256/apps/com.tealdrones.enlumi.png");
    let icon_changed = fs::read(&canonical).map_or(true, |existing| existing != png);
    write_changed(&canonical, png)?;
    if icon_changed {
        let theme = data.join("icons/hicolor");
        // Invalidate directory-based icon caches without requiring GTK tools.
        fs::File::open(&theme)?.set_modified(std::time::SystemTime::now())?;
    }

    let executable = std::env::current_exe()?;
    let executable = executable
        .to_str()
        .ok_or_else(|| io::Error::other("Executable path is not UTF-8"))?;
    let icon = icon
        .to_str()
        .ok_or_else(|| io::Error::other("Desktop icon path is not UTF-8"))?;
    let quoted = executable
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$")
        .replace('%', "%%");
    let entry = include_str!("../assets/com.tealdrones.enlumi.desktop")
        .replace(
            "Exec=enlumi %f",
            &format!("Exec={} %f", value(&format!("\"{quoted}\""))),
        )
        .replace(
            "Icon=com.tealdrones.enlumi",
            &format!("Icon={}", value(icon)),
        );
    write_changed(
        &data.join("applications/com.tealdrones.enlumi.desktop"),
        entry.as_bytes(),
    )?;
    Ok(())
}

fn value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn write_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(());
    }
    let directory = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid desktop resource path"))?;
    fs::create_dir_all(directory)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(bytes)?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
