//! Local Auto overrides never modify shared recipes. Explicit override wins over recipe default.
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};
pub type AutoChoices = BTreeMap<PathBuf, bool>;
fn path() -> Result<PathBuf, String> {
    crate::preferences::config_directory()
        .map(|p| p.join("profile-auto.json"))
        .ok_or("Cannot find configuration directory".into())
}
pub fn load() -> Result<AutoChoices, String> {
    load_from(&path()?)
}
fn load_from(path: &Path) -> Result<AutoChoices, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1_048_576 {
        return Err("Profile Auto preferences exceed 1 MiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("Profile Auto preferences: {e}"))
}
pub fn save_choice(recipe: &Path, enabled: bool) -> Result<AutoChoices, String> {
    if enabled && crate::profile::load(recipe)?.requires_settings.is_empty() {
        return Err(
            "Auto selection needs firmware/settings conditions in requires_settings".into(),
        );
    }
    save_to(&path()?, recipe, enabled)
}
fn save_to(path: &Path, recipe: &Path, enabled: bool) -> Result<AutoChoices, String> {
    let mut choices = load_from(path)?;
    choices.insert(recipe.canonicalize().map_err(|e| e.to_string())?, enabled);
    let parent = path.parent().ok_or("Invalid preference path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, &choices).map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(choices)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_overrides_keep_read_only_recipe_bytes_and_saved_false() {
        let dir = tempfile::tempdir().unwrap();
        let recipe = dir.path().join("recipe.json");
        let prefs = dir.path().join("config/auto.json");
        let original = b"{\"version\":1,\"name\":\"Read only\",\"auto_apply\":true}\n";
        std::fs::write(&recipe, original).unwrap();
        let mut permissions = std::fs::metadata(&recipe).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&recipe, permissions).unwrap();
        save_to(&prefs, &recipe, false).unwrap();
        let choices = load_from(&prefs).unwrap();
        assert_eq!(choices.get(&recipe.canonicalize().unwrap()), Some(&false));
        assert_eq!(std::fs::read(&recipe).unwrap(), original);
        save_to(&prefs, &recipe, true).unwrap();
        assert_eq!(load_from(&prefs).unwrap().len(), 1);
        assert_eq!(std::fs::read(&recipe).unwrap(), original);
    }
    #[test]
    fn malformed_preferences_do_not_get_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = dir.path().join("auto.json");
        let recipe = dir.path().join("recipe.json");
        std::fs::write(&prefs, b"bad JSON").unwrap();
        std::fs::write(&recipe, b"{}").unwrap();
        assert!(save_to(&prefs, &recipe, true).is_err());
        assert_eq!(std::fs::read(&prefs).unwrap(), b"bad JSON");
    }
}
