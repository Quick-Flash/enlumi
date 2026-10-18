//! Startup update preferences, master activity checks and GitHub links.
use serde::{Deserialize, Serialize};
use std::{io::Read, path::PathBuf, process::Command, time::Duration};

pub const BUILD_COMMIT: &str = env!("ENLUMI_BUILD_COMMIT");
pub const DOWNLOADS: &str = "https://github.com/Quick-Flash/enlumi/releases";

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub nightly: bool,
    pub dismissed: Option<String>,
}

fn path() -> Result<PathBuf, String> {
    crate::preferences::config_directory()
        .map(|p| p.join("updates.json"))
        .ok_or("Cannot find configuration directory".into())
}

impl Preferences {
    pub fn load() -> Result<Self, String> {
        let file = match std::fs::File::open(path()?) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut bytes = Vec::new();
        file.take(4097)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 4096 {
            return Err("Update preferences exceed 4 KiB".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("Update preferences: {e}"))
    }

    pub fn save(&self) -> Result<(), String> {
        let path = path()?;
        let directory = path.parent().ok_or("Invalid update preferences path")?;
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut file, self).map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn latest() -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .https_only(true)
        .build()
        .into();
    let mut response = agent
        .get("https://api.github.com/repos/Quick-Flash/enlumi/commits/master")
        .header("User-Agent", concat!("Enlumi/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github.sha")
        .call()
        .map_err(|e| format!("Could not check master: {e}"))?;
    let text = response
        .body_mut()
        .with_config()
        .limit(128)
        .read_to_string()
        .map_err(|e| format!("Could not read master revision: {e}"))?;
    let sha = text.trim();
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("GitHub returned invalid master revision".into());
    }
    Ok(sha.to_ascii_lowercase())
}

// Fixed GitHub destinations only; never execute a shell or API-provided URL.
pub fn open_page(commit: Option<&str>) -> Result<(), String> {
    let url = match commit {
        Some(sha) if sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()) => {
            format!("https://github.com/Quick-Flash/enlumi/commit/{sha}")
        }
        Some(_) => return Err("Invalid update revision".into()),
        None => DOWNLOADS.to_owned(),
    };
    open_url(&url)
}

pub fn open_repository() -> Result<(), String> {
    open_url("https://github.com/Quick-Flash/enlumi")
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = Command::new("/usr/bin/open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let status = command
        .arg(url)
        .status()
        .map_err(|e| format!("Could not open browser: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Browser command failed: {status}"))
    }
}
