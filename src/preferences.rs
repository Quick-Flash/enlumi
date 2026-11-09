//! Palette persistence and platform configuration directory.
use crate::theme::{CustomTheme, Palette};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    rc::Rc,
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
    /// Built-in palettes followed by the user's custom themes, by name.
    pub fn themes(&self) -> Vec<Palette> {
        let mut custom: Vec<Palette> = self
            .themes_directory()
            .and_then(|directory| std::fs::read_dir(directory).ok())
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.extension()? != "css" {
                    return None;
                }
                let name = path
                    .file_stem()?
                    .to_str()
                    .filter(|name| valid_theme_name(name))?;
                let css = std::fs::read_to_string(&path).ok()?;
                Some(Palette::Custom(Rc::new(CustomTheme::new(name, &css))))
            })
            .collect();
        custom.sort_by_key(|theme| theme.name().to_lowercase());
        Palette::ALL.into_iter().chain(custom).collect()
    }
    pub fn load(&self) -> Palette {
        let Some(id) = self
            .0
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
        else {
            return Palette::default();
        };
        self.themes()
            .into_iter()
            .find(|palette| palette.id() == id.trim())
            .unwrap_or_default()
    }
    pub fn save(&self, palette: &Palette) -> io::Result<()> {
        let Some(path) = &self.0 else {
            return Ok(());
        };
        write_atomic(path, &format!("{}\n", palette.id()))
    }
    /// Writes `<config>/themes/<name>.css` and returns the theme as it will load.
    pub fn save_custom(&self, name: &str, tokens: &[(String, String)]) -> io::Result<Palette> {
        let name = name.trim();
        if !valid_theme_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Theme names use 1 to 64 letters, digits, spaces, '-' or '_'",
            ));
        }
        let css: String = tokens
            .iter()
            .map(|(token, value)| format!("{token}: {value};\n"))
            .collect();
        if let Some(directory) = self.themes_directory() {
            write_atomic(&directory.join(format!("{name}.css")), &css)?;
        }
        Ok(Palette::Custom(Rc::new(CustomTheme::new(name, &css))))
    }
    pub fn delete_custom(&self, name: &str) -> io::Result<()> {
        match self.themes_directory() {
            Some(directory) if valid_theme_name(name) => {
                std::fs::remove_file(directory.join(format!("{name}.css")))
            }
            _ => Ok(()),
        }
    }
    fn themes_directory(&self) -> Option<PathBuf> {
        Some(self.0.as_ref()?.parent()?.join("themes"))
    }
}
pub fn valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name == name.trim()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
}
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(text.as_bytes())?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}
/// Folder of the last opened log, kept so the file dialog starts there on platforms whose
/// native dialog does not remember it.
#[derive(Clone, Default)]
pub struct LastLogDirectory(Option<PathBuf>);
impl LastLogDirectory {
    #[cfg(test)]
    pub fn at(path: PathBuf) -> Self {
        Self(Some(path))
    }
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
/// Writable per-user cache location, separate from portable workspace recipes.
pub(crate) fn cache_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let directory = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|p| p.join("Enlumi"));
    #[cfg(target_os = "macos")]
    let directory = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|p| p.join("Library/Caches/Enlumi"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let directory = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(".cache"))
        })
        .map(|p| p.join("enlumi"));
    directory.or_else(|| config_directory().map(|p| p.join("cache")))
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
        std::fs::write(
            dir.path().join("config/last-log-directory"),
            "relative/dir\n",
        )
        .unwrap();
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
            preferences.save(&palette).unwrap();
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
    #[test]
    fn custom_themes_roundtrip_list_and_delete() {
        let directory = tempfile::tempdir().unwrap();
        let preferences = ThemePreference(Some(directory.path().join("theme")));
        let tokens = vec![
            ("--app-accent".to_owned(), "#ff7a59".to_owned()),
            ("--app-measurement-fill".to_owned(), "#11223344".to_owned()),
        ];
        let saved = preferences.save_custom("Night Owl", &tokens).unwrap();
        assert_eq!(saved.id(), "custom:Night Owl");
        let themes = preferences.themes();
        assert_eq!(themes.len(), Palette::ALL.len() + 1);
        assert_eq!(themes.last(), Some(&saved));
        assert_eq!(saved.measurement_fill(), [0x11, 0x22, 0x33, 0x44]);
        // Tokens the theme leaves out keep the default palette's value.
        assert_eq!(saved.plot_background(), Palette::Slate.plot_background());

        preferences.save(&saved).unwrap();
        assert_eq!(preferences.load(), saved);

        assert!(preferences.save_custom("../escape", &tokens).is_err());
        assert!(preferences.save_custom("", &tokens).is_err());

        preferences.delete_custom("Night Owl").unwrap();
        assert_eq!(preferences.themes().len(), Palette::ALL.len());
        assert_eq!(preferences.load(), Palette::Slate);
    }
}
