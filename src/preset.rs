//! Portable recipes. Viewport state and sample buffers deliberately have no fields here.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotPreset {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<[u8; 3]>,
    pub lines: Vec<PresetLine>,
    #[serde(default)]
    pub geometry: Vec<String>,
    #[serde(default)]
    pub geometry_style: std::collections::BTreeMap<String, GeometryStyle>,
    #[serde(default)]
    pub legend_order: Vec<String>,
    #[serde(default)]
    pub links: Links,
    pub analysis: Option<Analysis>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory: Option<crate::trajectory::Spec>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetLine {
    pub source: String,
    pub color: [u8; 4],
    pub width: f64,
    pub radius: f64,
    pub marker: String,
    pub connection: String,
    pub pattern: String,
    pub markers: bool,
    pub hidden: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryStyle {
    pub width: f64,
    pub shade: crate::geometry::Shade,
    pub extend: bool,
    pub orientation: crate::geometry::LineOrientation,
    pub labels: bool,
    pub hidden: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Links {
    #[serde(default = "default_link")]
    pub timeline: bool,
    pub x: bool,
    pub y: bool,
    #[serde(default = "default_link")]
    pub pointer: bool,
}
fn default_link() -> bool {
    true
}
impl Default for Links {
    fn default() -> Self {
        Self {
            x: false,
            y: false,
            pointer: true,
            timeline: true,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Analysis {
    pub mode: String,
    pub window: String,
    pub size: usize,
    pub overlap: usize,
    pub heatmap_bins: usize,
    pub remove_dc: bool,
    pub condition: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewPreset {
    pub name: String,
    pub plots: Vec<PlotPreset>,
    pub layout: PresetLayout,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePreset {
    pub name: String,
    pub views: Vec<ViewPreset>,
    #[serde(default)]
    pub active: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresetLayout {
    Plot {
        plot: usize,
    },
    Tabs {
        plots: Vec<usize>,
        active: usize,
    },
    Split {
        stacked: bool,
        ratio: f64,
        first: Box<Self>,
        second: Box<Self>,
    },
}
#[derive(Clone)]
pub struct PresetSource {
    pub path: PathBuf,
    pub name: String,
    pub baseline: String,
}

pub fn validate(profile: &crate::profile::Profile) -> Result<(), String> {
    if !matches!(profile.version, 1 | 2) || profile.name.trim().is_empty() {
        return Err("Profile needs supported version and nonempty name".into());
    }
    if profile.plot_presets.len() > 128
        || profile.view_presets.len() > 128
        || profile.workspace_presets.len() > 128
    {
        return Err("Profile exceeds 128 plot or view presets".into());
    }
    let mut names = profile
        .views
        .iter()
        .map(|view| &view.name)
        .collect::<BTreeSet<_>>();
    for plot in &profile.plot_presets {
        if plot.name.trim().is_empty() || !names.insert(&plot.name) {
            return Err("Plot preset names must be nonempty and unique".into());
        }
        validate_plot(plot)?;
    }
    names.clear();
    for view in &profile.view_presets {
        if view.name.trim().is_empty()
            || !names.insert(&view.name)
            || view.plots.is_empty()
            || view.plots.len() > 128
        {
            return Err("View presets need unique names and 1–128 plots".into());
        }
        for plot in &view.plots {
            validate_plot(plot)?;
        }
        let mut seen = BTreeSet::new();
        validate_layout(&view.layout, view.plots.len(), &mut seen, 0)?;
        if seen.len() != view.plots.len() {
            return Err("Layout must contain every plot exactly once".into());
        }
    }
    names.clear();
    for workspace in &profile.workspace_presets {
        if workspace.name.trim().is_empty()
            || !names.insert(&workspace.name)
            || workspace.views.is_empty()
            || workspace.views.len() > 128
            || workspace.active >= workspace.views.len()
        {
            return Err(
                "Workspace presets need unique names, 1–128 views, and a valid active view".into(),
            );
        }
        let mut view_names = BTreeSet::new();
        for view in &workspace.views {
            if view.name.trim().is_empty()
                || !view_names.insert(&view.name)
                || view.plots.is_empty()
                || view.plots.len() > 128
            {
                return Err("Workspace views need unique names and 1–128 plots".into());
            }
            for plot in &view.plots {
                validate_plot(plot)?;
            }
            let mut seen = BTreeSet::new();
            validate_layout(&view.layout, view.plots.len(), &mut seen, 0)?;
            if seen.len() != view.plots.len() {
                return Err("Workspace view layout must contain every plot exactly once".into());
            }
        }
    }
    Ok(())
}
fn validate_plot(plot: &PlotPreset) -> Result<(), String> {
    if let Some(spec) = &plot.trajectory {
        spec.validate()?;
        if plot.analysis.is_some() {
            return Err("A plot cannot be both trajectory and frequency analysis".into());
        }
    }
    if plot.name.trim().is_empty() || plot.lines.len() > 4096 {
        return Err("Invalid plot preset".into());
    }
    let mut seen = BTreeSet::new();
    for line in &plot.lines {
        if !seen.insert(&line.source)
            || !line.width.is_finite()
            || !(0.1..=20.).contains(&line.width)
            || !line.radius.is_finite()
            || !(0.1..=30.).contains(&line.radius)
            || !matches!(line.connection.as_str(), "linear" | "pre" | "post")
            || !matches!(
                line.pattern.as_str(),
                "solid" | "dashed" | "dotted" | "dash_dot" | "none"
            )
            || !matches!(
                line.marker.as_str(),
                "circle" | "square" | "triangle" | "diamond" | "cross"
            )
        {
            return Err("Invalid preset line appearance or duplicate source".into());
        }
    }
    if let Some(a) = &plot.analysis
        && (!matches!(a.mode.as_str(), "fft" | "psd" | "heatmap" | "spectrogram")
            || !matches!(
                a.window.as_str(),
                "hann" | "hamming" | "blackman" | "rectangular"
            )
            || a.overlap >= 100
            || a.heatmap_bins == 0
            || a.heatmap_bins > 1024
            || matches!(a.mode.as_str(), "heatmap" | "spectrogram")
                && (!a.size.is_power_of_two() || !(128..=65_536).contains(&a.size))
            || matches!(a.mode.as_str(), "heatmap" | "spectrogram") && plot.lines.len() != 1
            || a.mode == "heatmap" && !(8..=512).contains(&a.heatmap_bins)
            || a.mode == "heatmap"
                && a.condition
                    .as_ref()
                    .is_none_or(|condition| condition.is_empty()))
    {
        return Err("Invalid preset analysis configuration".into());
    }
    for style in plot.geometry_style.values() {
        if !style.width.is_finite() || !(0.5..=10.).contains(&style.width) {
            return Err("Invalid geometry appearance".into());
        }
    }
    Ok(())
}
fn validate_layout(
    layout: &PresetLayout,
    count: usize,
    seen: &mut BTreeSet<usize>,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("Preset layout nesting exceeds 32".into());
    }
    match layout {
        PresetLayout::Plot { plot } => {
            if *plot >= count || !seen.insert(*plot) {
                return Err("Invalid or repeated plot index".into());
            }
        }
        PresetLayout::Tabs { plots, active } => {
            if !plots.contains(active) || plots.is_empty() {
                return Err("Invalid active tab".into());
            }
            for plot in plots {
                validate_layout(&PresetLayout::Plot { plot: *plot }, count, seen, depth + 1)?;
            }
        }
        PresetLayout::Split {
            ratio,
            first,
            second,
            ..
        } => {
            if !ratio.is_finite() || !(0.05..=0.95).contains(ratio) {
                return Err("Invalid split ratio".into());
            }
            validate_layout(first, count, seen, depth + 1)?;
            validate_layout(second, count, seen, depth + 1)?;
        }
    }
    Ok(())
}
