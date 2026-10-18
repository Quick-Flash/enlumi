//! Select completed nightly assets; never install a raw branch revision.
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Nightly {
    pub tag: String,
    pub commit: String,
    pub asset: String,
    pub digest: String,
    pub size: u64,
}

pub fn platform() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("linux-x86_64"),
        ("windows", "x86_64") => Ok("windows-x86_64"),
        ("macos", "aarch64") => Ok("macos-aarch64"),
        ("macos", "x86_64") => Ok("macos-x86_64"),
        _ => Err("No nightly bundle available for this platform".into()),
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    target_commitish: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    created_at: String,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
    size: u64,
    digest: Option<String>,
}

pub fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .https_only(true)
        .build()
        .into()
}

fn json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let mut response = agent(Duration::from_secs(20))
        .get(&format!(
            "https://api.github.com/repos/Quick-Flash/enlumi/{path}"
        ))
        .header("User-Agent", concat!("Enlumi/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(8 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

pub fn latest() -> Result<Option<Nightly>, String> {
    let platform = platform()?;
    let extension = if cfg!(windows) { "zip" } else { "tar.gz" };
    let name = format!("enlumi-{platform}.{extension}");
    let releases: Vec<Release> = json("releases?per_page=100")?;
    let release = releases
        .into_iter()
        .filter(|r| {
            !r.draft
                && r.prerelease
                && r.tag_name.starts_with("nightly-")
                && r.tag_name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                && hex(&r.target_commitish, 40)
                && r.published_at.is_some()
        })
        // GitHub created_at uses commit date, not publication time.
        .max_by(|a, b| a.created_at.cmp(&b.created_at));
    let Some(release) = release else {
        return Ok(None);
    };
    if release.target_commitish == crate::updates::BUILD_COMMIT {
        return Ok(None);
    }
    let Some(asset) = release
        .assets
        .into_iter()
        .find(|a| a.name == name && a.state == "uploaded")
    else {
        return Ok(None);
    };
    let digest = asset
        .digest
        .and_then(|s| s.strip_prefix("sha256:").map(str::to_owned))
        .filter(|s| hex(s, 64))
        .ok_or("Nightly asset has no SHA-256 digest")?;
    // A slow, older PR build may publish after a newer build. Do not downgrade.
    if !hex(crate::updates::BUILD_COMMIT, 40) {
        return Ok(None);
    }
    #[derive(Deserialize)]
    struct Comparison {
        status: String,
    }
    let comparison: Comparison = json(&format!(
        "compare/{}...{}?per_page=1",
        crate::updates::BUILD_COMMIT,
        release.target_commitish
    ))?;
    match comparison.status.as_str() {
        "ahead" => {}
        "diverged" => {
            return Err(
                "Nightly history differs from this build. Use Downloads to switch versions.".into(),
            );
        }
        _ => return Ok(None),
    }
    Ok(Some(Nightly {
        tag: release.tag_name,
        commit: release.target_commitish,
        asset: name,
        digest: digest.to_ascii_lowercase(),
        size: asset.size,
    }))
}
