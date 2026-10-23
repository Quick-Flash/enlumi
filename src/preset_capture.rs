use crate::{
    derived::FilterSpec,
    preset::*,
    profile::{self, Operation, Scalar, Source},
    state::{Pane, Workbench},
};
use std::collections::{BTreeMap, BTreeSet};
use zgui::prelude::*;

#[derive(Clone)]
pub enum CaptureKind {
    Selection(Vec<String>),
    Plot(u64),
    View,
    Workspace,
}
#[derive(Clone)]
pub struct Bundle {
    pub document: serde_json::Value,
    pub files: BTreeMap<String, String>,
}
struct Capture {
    workbench: Workbench,
    profile: profile::Profile,
    files: BTreeMap<String, String>,
    sources: BTreeMap<String, String>,
    visiting: BTreeSet<String>,
    geometry: BTreeMap<u64, String>,
    prefix: String,
    validate: bool,
}

pub fn capture(workbench: Workbench, kind: &CaptureKind, name: &str) -> Result<Bundle, String> {
    capture_inner(workbench, kind, name, true)
}

pub fn capture_for_comparison(
    workbench: Workbench,
    kind: &CaptureKind,
    name: &str,
) -> Result<Bundle, String> {
    // Menu labels compare settings; source validation and file packaging belong to Save.
    capture_inner(workbench, kind, name, false)
}

fn capture_inner(
    workbench: Workbench,
    kind: &CaptureKind,
    name: &str,
    validate: bool,
) -> Result<Bundle, String> {
    if name.trim().is_empty() {
        return Err("Choose a name".into());
    }
    let prefix = format!("p_{:016x}", hash(name.as_bytes()));
    let profile = serde_json::from_value(serde_json::json!({"version":2,"name":name}))
        .map_err(|e| e.to_string())?;
    let mut capture = Capture {
        workbench,
        profile,
        files: BTreeMap::new(),
        sources: BTreeMap::new(),
        visiting: BTreeSet::new(),
        geometry: BTreeMap::new(),
        prefix,
        validate,
    };
    match kind {
        CaptureKind::Selection(sources) => {
            if sources.is_empty() {
                return Err("Select fields to save".into());
            }
            for source in sources {
                capture.source(source)?;
            }
        }
        CaptureKind::Plot(id) => {
            let pane = workbench.find(*id).ok_or("Plot was closed")?;
            let plot = capture.plot(pane, name)?;
            capture.profile.plot_presets.push(plot);
        }
        CaptureKind::View => {
            let view = capture.view(workbench, name)?;
            capture.profile.view_presets.push(view);
        }
        CaptureKind::Workspace => {
            let workspace = workbench.workspace();
            let current_views = workbench.workspace_views(workspace.id);
            let active_view = if workbench.workspaces.active.get_untracked() == workspace.id {
                workbench.views.active.get_untracked()
            } else {
                workspace.active_view
            };
            let active = current_views
                .iter()
                .position(|view| view.id == active_view)
                .unwrap_or(0);
            let mut views = Vec::new();
            for view in current_views {
                let target = workbench
                    .for_view(view.id)
                    .ok_or("Workspace view was closed")?;
                views.push(capture.view(target, &view.name.get_untracked())?);
            }
            capture.profile.workspace_presets.push(WorkspacePreset {
                name: name.into(),
                views,
                active,
            });
        }
    }
    if validate {
        crate::preset::validate(&capture.profile)?;
    }
    Ok(Bundle {
        document: serde_json::to_value(capture.profile).map_err(|e| e.to_string())?,
        files: capture.files,
    })
}
pub fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(14695981039346656037, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(1099511628211)
    })
}
impl Capture {
    fn view(&mut self, workbench: Workbench, name: &str) -> Result<ViewPreset, String> {
        let panes = workbench.panes().get_untracked();
        let mut plots = Vec::new();
        let mut ids = BTreeMap::new();
        for (index, pane) in panes.iter().enumerate() {
            ids.insert(pane.id, index);
            let title = workbench
                .profiles
                .titles
                .with_untracked(|titles| titles.get(&pane.id).cloned())
                .unwrap_or_else(|| format!("Plot {}", index + 1));
            plots.push(self.plot(*pane, &title)?);
        }
        Ok(ViewPreset {
            name: name.into(),
            plots,
            layout: layout(workbench.layout().get_untracked(), &ids)?,
        })
    }
    fn metadata(&self, source: &str) -> profile::Metadata {
        self.workbench
            .profiles
            .metadata
            .with_untracked(|metadata| metadata.get(source).cloned())
            .unwrap_or_else(|| profile::Metadata {
                name: self.workbench.field_name_untracked(source),
                ..Default::default()
            })
    }
    fn source(&mut self, source: &str) -> Result<String, String> {
        let source = source.strip_prefix("@spectrum/").unwrap_or(source);
        if let Some(token) = self.sources.get(source) {
            return Ok(token.clone());
        }
        if !self.visiting.insert(source.into()) {
            return Err("Filter dependency cycle".into());
        }
        let id = format!("{}_field_{:016x}", self.prefix, hash(source.as_bytes()));
        let metadata = self.metadata(source);
        if let Some(derived) = crate::derived::id(source) {
            let field = self
                .workbench
                .derived
                .with_untracked(|fields| fields.iter().find(|field| field.id == derived).cloned())
                .ok_or_else(|| format!("Missing filter {source}"))?;
            let inputs = field
                .sources
                .iter()
                .map(|source| self.source(source).map(Source::Field))
                .collect::<Result<Vec<_>, _>>()?;
            let operation = match field.filter {
                FilterSpec::LowPass(hz) => Operation::LowPass {
                    hz: Scalar::Number(hz),
                },
                FilterSpec::HighPass(hz) => Operation::HighPass {
                    hz: Scalar::Number(hz),
                },
                FilterSpec::ZeroPhaseLowPass(hz) => Operation::ZeroPhaseLowPass {
                    hz: Scalar::Number(hz),
                },
                FilterSpec::ZeroPhaseHighPass(hz) => Operation::ZeroPhaseHighPass {
                    hz: Scalar::Number(hz),
                },
                FilterSpec::MovingAverage(samples) => Operation::MovingAverage { samples },
                FilterSpec::Derivative => Operation::Derivative,
                FilterSpec::Integral(initial) => Operation::Integral {
                    initial: Scalar::Number(initial),
                },
                FilterSpec::Affine { gain, offset } => Operation::Affine {
                    gain: Scalar::Number(gain),
                    offset: Scalar::Number(offset),
                },
                FilterSpec::Magnitude => Operation::Magnitude,
                FilterSpec::Quaternion(axis, unit) => Operation::Quaternion {
                    axis: match axis {
                        crate::derived::RpyAxis::Roll => "roll",
                        crate::derived::RpyAxis::Pitch => "pitch",
                        crate::derived::RpyAxis::Yaw => "yaw",
                    }
                    .into(),
                    degrees: unit == crate::derived::AngleUnit::Degrees,
                },
                FilterSpec::Rust(script, output) => {
                    if self.validate {
                        script.validate(field.sources.len())?;
                    }
                    let path = format!("sources/{:016x}.rs", hash(script.source.as_bytes()));
                    if self.validate {
                        self.files.insert(path.clone(), script.source.clone());
                    }
                    Operation::Rust {
                        path: path.into(),
                        outputs: script.outputs,
                        output,
                    }
                }
            };
            self.profile.filters.push(profile::Filter {
                id: id.clone(),
                metadata,
                inputs,
                operation,
            });
        } else {
            let input = if source.starts_with("@setting/") {
                self.workbench.log.with_untracked(|log| {
                    let log = log.as_ref().ok_or("Open a log first")?;
                    let parts = source.split('/').collect::<Vec<_>>();
                    let flight = parts[1].parse::<usize>().map_err(|e| e.to_string())?;
                    let index = parts[2].parse::<usize>().map_err(|e| e.to_string())?;
                    let setting = log.setting(source).ok_or("Parameter no longer exists")?;
                    let occurrence = log.settings[&flight][..index]
                        .iter()
                        .filter(|record| record.name == setting.name)
                        .count();
                    Ok::<_, String>(Source::Setting {
                        setting: profile::Setting {
                            name: setting.name.clone(),
                            flight,
                            occurrence: Some(occurrence),
                            element: parts
                                .get(3)
                                .map(|part| part.parse())
                                .transpose()
                                .map_err(|e: std::num::ParseIntError| e.to_string())?,
                        },
                    })
                })?
            } else {
                if source.starts_with('@') {
                    return Err(format!("Resolve profile source before saving: {source}"));
                }
                Source::Field(source.into())
            };
            self.profile.signals.push(profile::Signal {
                id: id.clone(),
                source: input,
                metadata,
            });
        }
        self.visiting.remove(source);
        let token = format!("${id}");
        self.sources.insert(source.into(), token.clone());
        Ok(token)
    }
    fn plot(&mut self, pane: Pane, name: &str) -> Result<PlotPreset, String> {
        use plot::{
            line_draw::{Connection, LinePattern},
            plot_data::MarkerShape,
        };
        let mut lines = Vec::new();
        let mut line_ids = BTreeMap::new();
        let (inputs, analysis) = pane.analysis.with_untracked(|result| {
            if let Some(result) = result {
                let inputs = result
                    .series
                    .series()
                    .iter()
                    .map(|series| series.id().to_owned())
                    .collect::<Vec<_>>();
                (
                    inputs,
                    Some((
                        result.mode,
                        result.options.clone(),
                        result.condition.clone(),
                    )),
                )
            } else {
                (
                    pane.lines.with_untracked(|lines| {
                        lines.iter().map(|line| line.source.clone()).collect()
                    }),
                    None,
                )
            }
        });
        for source in inputs {
            let token = self.source(&source)?;
            let original = pane.lines.with_untracked(|lines| {
                lines
                    .iter()
                    .find(|line| {
                        line.source == source
                            || line.source.strip_prefix("@spectrum/") == Some(source.as_str())
                    })
                    .cloned()
            });
            let key = original
                .as_ref()
                .map_or_else(|| source.clone(), |line| line.id.to_string());
            let style = pane.plot.style(&key);
            let draw = original
                .as_ref()
                .map(|line| {
                    pane.drawing.with_untracked(|drawing| {
                        drawing.get(&line.id).copied().unwrap_or_default()
                    })
                })
                .unwrap_or_default();
            if let Some(line) = original {
                line_ids.insert(line.id, format!("line:{token}"));
            }
            lines.push(PresetLine {
                source: token,
                color: style.color,
                width: style.width,
                radius: style.radius,
                marker: match style.marker {
                    MarkerShape::Circle => "circle",
                    MarkerShape::Square => "square",
                    MarkerShape::Triangle => "triangle",
                    MarkerShape::Diamond => "diamond",
                    MarkerShape::Cross => "cross",
                }
                .into(),
                connection: match draw.connection {
                    Connection::Linear => "linear",
                    Connection::Pre => "pre",
                    Connection::Post => "post",
                }
                .into(),
                pattern: match draw.pattern {
                    LinePattern::Solid => "solid",
                    LinePattern::Dashed => "dashed",
                    LinePattern::Dotted => "dotted",
                    LinePattern::DashDot => "dash_dot",
                    LinePattern::None => "none",
                }
                .into(),
                markers: draw.markers,
                hidden: pane
                    .plot
                    .hidden
                    .with_untracked(|hidden| hidden.contains(&key)),
            });
        }
        let mut geometry = Vec::new();
        let mut geometry_style = BTreeMap::new();
        for id in pane.geometry.get_untracked() {
            let item = self
                .workbench
                .geometry_item(id)
                .ok_or("Geometry no longer exists")?;
            let token = self.geometry.get(&id).cloned().unwrap_or_else(|| {
                format!(
                    "{}_geometry_{}",
                    self.prefix,
                    self.profile.geometry.len() + 1
                )
            });
            if !self.geometry.contains_key(&id) {
                let shape = if let Some(states) = &item.states {
                    let source = Source::Field(self.source(&states.source)?);
                    if let Some(rules) = &states.flag_rules {
                        profile::Shape::Flags {
                            source,
                            rules: rules.clone(),
                        }
                    } else {
                        profile::Shape::States {
                            source,
                            rules: states.rules.clone(),
                        }
                    }
                } else {
                    match item.kind {
                        crate::geometry::GeometryKind::Rectangle => profile::Shape::Rectangle {
                            coordinates: item.coordinates.map(Scalar::Number),
                        },
                        crate::geometry::GeometryKind::Line => profile::Shape::Line {
                            coordinates: item.coordinates.map(Scalar::Number),
                        },
                        crate::geometry::GeometryKind::Band => profile::Shape::Band {
                            start: Scalar::Number(item.coordinates[0]),
                            end: Scalar::Number(item.coordinates[2]),
                        },
                    }
                };
                self.profile.geometry.push(profile::Geometry {
                    id: token.clone(),
                    name: item.name.clone(),
                    color: item.color,
                    opacity: item.opacity,
                    shape,
                });
                self.geometry.insert(id, token.clone());
            }
            geometry_style.insert(
                token.clone(),
                GeometryStyle {
                    width: item.width,
                    shade: item.shade,
                    extend: item.extend,
                    orientation: item.orientation,
                    labels: item.labels,
                    hidden: pane
                        .hidden_geometry
                        .with_untracked(|hidden| hidden.contains(&id)),
                },
            );
            line_ids.insert(id, format!("geometry:{token}"));
            geometry.push(token);
        }
        let analysis = analysis
            .map(|(mode, options, condition)| {
                Ok::<_, String>(Analysis {
                    mode: match mode {
                        crate::spectral::Mode::Fft => "fft",
                        crate::spectral::Mode::Psd => "psd",
                        crate::spectral::Mode::Heatmap => "heatmap",
                        crate::spectral::Mode::Spectrogram => "spectrogram",
                    }
                    .into(),
                    window: match options.window {
                        crate::spectral::Window::Hann => "hann",
                        crate::spectral::Window::Hamming => "hamming",
                        crate::spectral::Window::Blackman => "blackman",
                        crate::spectral::Window::Rectangular => "rectangular",
                    }
                    .into(),
                    size: options.size,
                    overlap: options.overlap,
                    heatmap_bins: options.heatmap_bins,
                    remove_dc: options.remove_dc,
                    condition: if condition.is_empty() {
                        None
                    } else {
                        Some(self.source(&condition)?)
                    },
                })
            })
            .transpose()?;
        let links = pane.links.get_untracked();
        Ok(PlotPreset {
            name: name.into(),
            background: pane.background.get_untracked(),
            lines,
            geometry,
            geometry_style,
            analysis,
            links: Links {
                x: links.x,
                y: links.y,
                pointer: links.pointer,
            },
            legend_order: pane.legend_order.with_untracked(|order| {
                order
                    .iter()
                    .filter_map(|id| line_ids.get(id).cloned())
                    .collect()
            }),
        })
    }
}
fn layout(
    value: plot::workspace::layout::Layout,
    ids: &BTreeMap<u64, usize>,
) -> Result<PresetLayout, String> {
    use plot::workspace::layout::Layout;
    Ok(match value {
        Layout::Empty => return Err("View has no plots".into()),
        Layout::Plot(id) => PresetLayout::Plot { plot: ids[&id] },
        Layout::Tabs { plots, active } => PresetLayout::Tabs {
            plots: plots.iter().map(|id| ids[id]).collect(),
            active: ids[&active],
        },
        Layout::Split {
            stacked,
            ratio,
            first,
            second,
            ..
        } => PresetLayout::Split {
            stacked,
            ratio,
            first: Box::new(layout(*first, ids)?),
            second: Box::new(layout(*second, ids)?),
        },
    })
}
