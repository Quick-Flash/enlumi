//! Portable, explicit setup recipes. Firmware knowledge belongs in these files.
#[cfg(test)]
mod examples_tests;
pub mod prepare;
pub mod resolve;
#[cfg(test)]
mod tests;

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub auto_apply: bool,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub requires_settings: Vec<SettingCondition>,
    #[serde(default)]
    pub signals: Vec<Signal>,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub geometry: Vec<Geometry>,
    #[serde(default)]
    pub views: Vec<View>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub name: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub axis: String,
}
impl Metadata {
    pub fn label(&self) -> String {
        self.label_for(&self.name)
    }
    pub fn label_for(&self, name: &str) -> String {
        if self.unit.is_empty() {
            name.to_owned()
        } else {
            format!("{name} [{}]", self.unit)
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signal {
    pub id: String,
    pub source: Source,
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Source {
    Field(String),
    Setting { setting: Setting },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub name: String,
    #[serde(default = "one")]
    pub flight: usize,
    /// Zero-based occurrence among records with this name. Required if ambiguous.
    pub occurrence: Option<usize>,
    /// Zero-based numeric array element. Omit for scalar parameters.
    pub element: Option<usize>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingCondition {
    pub setting: Setting,
    pub starts_with: Option<String>,
    pub equals: Option<String>,
}
fn one() -> usize {
    1
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Scalar {
    Number(f64),
    Setting { setting: Setting },
    Ratio { ratio: Box<[Scalar; 2]> },
}
impl Default for Scalar {
    fn default() -> Self {
        Self::Number(0.0)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub id: String,
    pub metadata: Metadata,
    pub inputs: Vec<Source>,
    pub operation: Operation,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Affine {
        gain: Scalar,
        #[serde(default)]
        offset: Scalar,
    },
    LowPass {
        hz: Scalar,
    },
    HighPass {
        hz: Scalar,
    },
    ZeroPhaseLowPass {
        hz: Scalar,
    },
    ZeroPhaseHighPass {
        hz: Scalar,
    },
    MovingAverage {
        samples: usize,
    },
    Derivative,
    Integral {
        #[serde(default)]
        initial: Scalar,
    },
    Magnitude,
    Quaternion {
        axis: String,
        #[serde(default)]
        degrees: bool,
    },
    Rust {
        path: PathBuf,
        #[serde(default = "one")]
        outputs: usize,
        #[serde(default)]
        output: usize,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub id: String,
    pub name: String,
    #[serde(default = "color")]
    pub color: [u8; 3],
    #[serde(default = "opacity")]
    pub opacity: f64,
    pub shape: Shape,
}
fn color() -> [u8; 3] {
    [235, 174, 82]
}
fn opacity() -> f64 {
    0.18
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    Rectangle {
        coordinates: [Scalar; 4],
    },
    Line {
        coordinates: [Scalar; 4],
    },
    Band {
        start: Scalar,
        end: Scalar,
    },
    States {
        source: Source,
        rules: Vec<crate::state_bands::StateRule>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub name: String,
    #[serde(default)]
    pub fields: Vec<Source>,
    #[serde(default)]
    pub geometry: Vec<String>,
    #[serde(default)]
    pub tab: bool,
    #[serde(default = "yes")]
    pub stacked: bool,
}
fn yes() -> bool {
    true
}

pub fn directory() -> Result<PathBuf, String> {
    crate::preferences::config_directory()
        .map(|path| path.join("profiles"))
        .ok_or("Cannot find user configuration directory".into())
}

pub fn discover() -> Result<Vec<PathBuf>, String> {
    let directory = directory()?;
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

pub fn load(path: &Path) -> Result<Profile, String> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(4_194_305)
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    if text.len() > 4_194_304 {
        return Err("Profile exceeds 4 MiB".into());
    }
    let profile: Profile = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if profile.version != 1 {
        return Err(format!("Unsupported profile version {}", profile.version));
    }
    if profile.name.trim().is_empty() {
        return Err("Profile needs a name".into());
    }
    if profile.filters.len() + profile.geometry.len() > 2048
        || profile.views.len() > 32
        || profile.signals.len() > 4096
    {
        return Err("Profile exceeds 2048 resources or 32 views".into());
    }
    Ok(profile)
}

pub type MetadataMap = BTreeMap<String, Metadata>;
