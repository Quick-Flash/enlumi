//! Palette persistence and platform configuration directory.
use crate::theme::Palette;
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Default)]
pub struct ThemePreference(Option<PathBuf>);
impl ThemePreference {
    pub fn user() -> Self {
        Self(config_directory().map(|path| path.join("theme")))
    }
    #[cfg(test)]
    pub fn disabled() -> Self {
        Self(None)
    }
    pub fn load(&self) -> Palette {
        self.0
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| Palette::from_id(text.trim()))
            .unwrap_or_default()
    }
    pub fn save(&self, palette: Palette) -> io::Result<()> {
        let Some(path) = &self.0 else {
            return Ok(());
        };
        let parent = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        writeln!(file, "{}", palette.id())?;
        file.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}
/// Folder of the last opened log, kept so the file dialog starts there on platforms whose
/// native dialog does not remember it.
#[derive(Clone, Default)]
pub struct LastLogDirectory(Option<PathBuf>);
impl LastLogDirectory {
    pub fn user() -> Self {
        Self(config_directory().map(|path| path.join("last-log-directory")))
    }
    pub fn load(&self) -> Option<PathBuf> {
        let text = std::fs::read_to_string(self.0.as_ref()?).ok()?;
        let path = PathBuf::from(text.trim_end_matches(['\r', '\n']));
        (path.is_absolute() && path.is_dir()).then_some(path)
    }
    pub fn save(&self, log: &Path) -> io::Result<()> {
        let (Some(path), Some(folder)) = (&self.0, log.parent()) else {
            return Ok(());
        };
        let Some(folder) = folder.to_str().filter(|f| !f.contains(['\r', '\n'])) else {
            return Ok(());
        };
        let parent = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        writeln!(file, "{folder}")?;
        file.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}
pub(crate) fn config_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| path.join("Enlumi"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| path.join("Library/Application Support/Enlumi"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|path| path.join(".config"))
            })
            .map(|path| path.join("enlumi"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn last_log_directory_roundtrips_and_rejects_stale_or_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let store = LastLogDirectory(Some(dir.path().join("config/last-log-directory")));
        assert_eq!(store.load(), None);
        let logs = dir.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        store.save(&logs.join("a.bbl")).unwrap();
        assert_eq!(store.load(), Some(logs.clone()));
        std::fs::remove_dir(&logs).unwrap();
        assert_eq!(store.load(), None);
        std::fs::write(dir.path().join("config/last-log-directory"), "relative/dir\n").unwrap();
        assert_eq!(store.load(), None);
        assert!(LastLogDirectory(None).save(Path::new("/x/a.bbl")).is_ok());
    }
    #[test]
    fn only_theme_id_roundtrips_and_invalid_preferences_fall_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config/theme");
        let preferences = ThemePreference(Some(path.clone()));
        assert_eq!(preferences.load(), Palette::Slate);
        for palette in Palette::ALL {
            preferences.save(palette).unwrap();
            assert_eq!(ThemePreference(Some(path.clone())).load(), palette);
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                format!("{}\n", palette.id())
            );
        }
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        std::fs::write(path, "unknown-theme").unwrap();
        assert_eq!(preferences.load(), Palette::Slate);
    }
}
