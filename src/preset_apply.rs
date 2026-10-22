use crate::{
    preset::*,
    preset_ui::{ApplyAction, ApplyRequest},
    profile::{self, identity::ProfileKey},
    state::{Pane, Workbench},
};
use plot::{
    time_series::{SeriesData, TimeSeries, TimeSeriesSet},
    workspace::layout::Layout,
};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};
use zgui::prelude::*;

pub fn prepared_recipe(plot: &PlotPreset, name: String) -> profile::View {
    let mut fields = plot
        .lines
        .iter()
        .map(|line| profile::Source::Field(line.source.clone()))
        .collect::<Vec<_>>();
    if let Some(condition) = plot
        .analysis
        .as_ref()
        .and_then(|analysis| analysis.condition.as_ref())
        && !plot.lines.iter().any(|line| &line.source == condition)
    {
        fields.push(profile::Source::Field(condition.clone()));
    }
    profile::View {
        name,
        fields,
        geometry: plot.geometry.clone(),
        tab: false,
        stacked: true,
    }
}
impl Workbench {
    pub fn use_saved_preset(self, key: ProfileKey, name: String, action: ApplyAction) {
        if self.loading.get_untracked()
            || self.profiles.busy.get_untracked()
            || self.derived_busy.get_untracked()
            || self.workspace_finishing()
        {
            self.status
                .set("Wait for loading and filters to finish".into());
            return;
        }
        self.authoring.pending.set_value(Some(ApplyRequest {
            key: key.clone(),
            name,
            view: self.bind_view().view().id,
            action,
        }));
        if self
            .profiles
            .instances
            .with_untracked(|instances| instances.contains_key(&key))
        {
            self.resume_preset_apply();
        } else {
            self.profiles.flight.set(key.flight);
            self.apply_profile(key.path);
        }
    }
    pub(crate) fn resume_preset_apply(self) {
        if self.profiles.busy.get_untracked() {
            return;
        }
        let Some(request) = self.authoring.pending.get_value() else {
            return;
        };
        let Some(workbench) = self
            .for_view(request.view)
            .or_else(|| matches!(request.action, ApplyAction::Workspace).then_some(self))
        else {
            self.authoring.pending.set_value(None);
            return;
        };
        let Some(instance) = self
            .profiles
            .instances
            .with_untracked(|instances| instances.get(&request.key).cloned())
        else {
            if self
                .profiles
                .failures
                .with_untracked(|failures| failures.contains_key(&request.key))
            {
                self.authoring.pending.set_value(None);
                return;
            }
            self.profiles.flight.set(request.key.flight);
            self.load_profile(request.key.path, None, false);
            return;
        };
        self.authoring.pending.set_value(None);
        let result = match request.action {
            ApplyAction::Plot => available_plots(&instance)
                .iter()
                .find(|plot| plot.name == request.name)
                .cloned()
                .ok_or("Plot preset no longer exists".into())
                .and_then(|plot| workbench.apply_plot_preset(&instance.key, &plot, None)),
            ApplyAction::NewView | ApplyAction::AppendView => {
                crate::preset_lookup::view(&instance.definition, &request.name)
                    .cloned()
                    .ok_or("View preset no longer exists".into())
                    .and_then(|view| {
                        workbench.apply_view_preset(
                            &instance.key,
                            view,
                            matches!(request.action, ApplyAction::NewView),
                        )
                    })
            }
            ApplyAction::Workspace
                if self.authoring.workspace_load.with_value(|load| {
                    load.as_ref().is_some_and(|load| load.key == instance.key)
                }) =>
            {
                self.complete_workspace_load(&instance);
                Ok(())
            }
            ApplyAction::Workspace => instance
                .definition
                .workspace_presets
                .iter()
                .find(|workspace| workspace.name == request.name)
                .cloned()
                .ok_or("Workspace preset no longer exists".into())
                .and_then(|workspace| workbench.open_workspace_preset(&instance.key, workspace)),
        };
        if let Err(error) = result {
            self.status.set(error);
        }
    }
    pub fn apply_plot_preset(
        self,
        key: &ProfileKey,
        recipe: &PlotPreset,
        target: Option<u64>,
    ) -> Result<(), String> {
        let pane = if let Some(id) = target {
            let pane = self.find(id).ok_or("Destination plot was closed")?;
            let mode = pane
                .analysis
                .with_untracked(|result| result.as_ref().map(|result| result.mode));
            let wanted = recipe
                .analysis
                .as_ref()
                .map(|analysis| mode_of(&analysis.mode));
            if mode != wanted || wanted.is_some_and(|mode| mode.windowed()) {
                return Err("Choose matching time/FFT/PSD plot; maps need a new plot".into());
            }
            pane
        } else {
            self.add_plot(self.active().get_untracked(), true, false)
        };
        self.populate_preset(key, recipe.clone(), pane, target.is_some());
        Ok(())
    }
    pub(crate) fn apply_view_preset(
        self,
        key: &ProfileKey,
        recipe: ViewPreset,
        new: bool,
    ) -> Result<(), String> {
        let target = if new {
            let id = self.create_view();
            let target = self.for_view(id).unwrap();
            target.view().name.set(recipe.name.clone());
            if self.is_current_view() {
                self.switch_view(id);
            }
            target
        } else {
            self
        };
        if new {
            target.view().preset.set(Some(PresetSource {
                path: key.path.clone(),
                name: recipe.name.clone(),
                baseline: String::new(),
            }));
            target.authoring.restoring.update_value(|pending| {
                pending.insert(target.view().id, recipe.plots.len());
            });
        }
        let old = target.layout().get_untracked();
        let mut panes = Vec::new();
        for (index, plot) in recipe.plots.iter().enumerate() {
            let pane = if new && index == 0 {
                target.pane()
            } else {
                target.add_plot(target.active().get_untracked(), true, false)
            };
            panes.push(pane.id);
            target.profiles.titles.update(|titles| {
                titles.insert(pane.id, plot.name.clone());
            });
            target.populate_preset(key, plot.clone(), pane, false);
        }
        let mut split = split_max(&target.layout().get_untracked()) + 1;
        let layout = restore_layout(&recipe.layout, &panes, &mut split);
        target.layout().set(if new {
            layout
        } else {
            Layout::Split {
                id: split + 1,
                stacked: true,
                ratio: 0.5,
                first: Box::new(old),
                second: Box::new(layout),
            }
        });
        target.finish_view_restore();
        Ok(())
    }
    pub(crate) fn populate_preset(
        self,
        key: &ProfileKey,
        recipe: PlotPreset,
        pane: Pane,
        fields_only: bool,
    ) {
        let Some(instance) = self
            .profiles
            .instances
            .with_untracked(|instances| instances.get(key).cloned())
        else {
            return;
        };
        let options = recipe.analysis.clone();
        let mut inputs = Vec::new();
        let mut warnings = Vec::new();
        for line in &recipe.lines {
            let Some(source) = instance.resolve(&line.source) else {
                warnings.push(format!("Unavailable input {}", line.source));
                continue;
            };
            let data = self
                .derived
                .with_untracked(|fields| {
                    fields
                        .iter()
                        .find(|field| field.key() == source)
                        .and_then(|field| field.data.clone())
                })
                .or_else(|| {
                    instance
                        .views
                        .values()
                        .flat_map(|view| view.fields.iter())
                        .find(|(id, _)| *id == source)
                        .map(|(_, data)| data.clone())
                });
            if let Some(data) = data {
                inputs.push((
                    source.clone(),
                    self.field_name_untracked(&source),
                    data,
                    line.source.clone(),
                ));
            } else {
                warnings.push(format!("Unavailable input {source}"));
            }
        }
        let condition_source = options
            .as_ref()
            .and_then(|analysis| analysis.condition.as_ref())
            .and_then(|token| instance.resolve(token))
            .unwrap_or_default();
        let conditioning = options
            .as_ref()
            .and_then(|analysis| analysis.condition.as_ref())
            .and_then(|token| instance.resolve(token))
            .and_then(|source| {
                self.derived
                    .with_untracked(|fields| {
                        fields
                            .iter()
                            .find(|field| field.key() == source)
                            .and_then(|field| field.data.clone())
                    })
                    .or_else(|| {
                        instance
                            .views
                            .values()
                            .flat_map(|view| view.fields.iter())
                            .find(|(id, _)| *id == source)
                            .map(|(_, data)| data.clone())
                    })
            });
        if !fields_only {
            self.profiles.titles.update(|titles| {
                titles.insert(pane.id, recipe.name.clone());
            });
        }
        if !warnings.is_empty() {
            self.status.set(warnings.join("; "));
        }
        if inputs.is_empty() && !recipe.lines.is_empty() && recipe.geometry.is_empty() {
            self.status.set(format!(
                "{}: no usable inputs; see profile diagnostics",
                recipe.name
            ));
            pane.loading.set(None);
            self.preset_plot_finished();
            return;
        }
        let preserve_bounds =
            pane.loading.with_untracked(Option::is_some) && !pane.plot.auto_fit.get_untracked();
        let generation = self.generation.get_untracked();
        let interval = (
            self.analysis().start.get_untracked().parse().ok(),
            self.analysis().end.get_untracked().parse().ok(),
        );
        let prepared = if fields_only {
            pane.analysis
                .with_untracked(|result| result.as_ref().map(|result| result.options.clone()))
        } else {
            options.map(|analysis| analysis_options(&analysis, interval))
        };
        let key = key.clone();
        let workbench = self.bind_view();
        // Work owns inputs and its destination; no shared Analyze editor is changed.
        self.task(async move {
            let computation = inputs.clone();
            let configuration = prepared.clone();
            let condition = conditioning.clone();
            let computed = blocking(move || compute(computation, configuration, condition)).await;
            if !workbench.view_exists()
                || workbench.generation.get_untracked() != generation
                || workbench.find(pane.id).is_none()
            {
                pane.loading.set(None);
                workbench.preset_plot_finished();
                return;
            }
            match computed {
                Ok((series, result)) => {
                    let spectral = result.is_some();
                    let previous_ids = pane.lines.with_untracked(|lines| {
                        lines
                            .iter()
                            .map(|line| line.id)
                            .collect::<std::collections::BTreeSet<_>>()
                    });
                    if fields_only {
                        pane.plot.auto_fit.set(false);
                    }
                    if let Some(mut result) = result {
                        result.title = recipe.name.clone();
                        result.condition = condition_source;
                        let units = inputs
                            .iter()
                            .map(|(source, _, _, _)| {
                                instance
                                    .metadata
                                    .get(source)
                                    .map(|meta| meta.unit.clone())
                                    .filter(|unit| !unit.is_empty())
                                    .unwrap_or_else(|| "unknown unit".into())
                            })
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>()
                            .join(", ");
                        let power_units = if result.mode == crate::spectral::Mode::Fft {
                            units
                        } else {
                            format!("({units})\u{00b2}/Hz")
                        };
                        result.units = match result.mode {
                            crate::spectral::Mode::Heatmap => format!(
                                "Y: {} [{}] / color: dB re 1 {power_units}",
                                workbench.field_name_untracked(&result.condition),
                                instance
                                    .metadata
                                    .get(&result.condition)
                                    .map(|meta| meta.unit.as_str())
                                    .filter(|unit| !unit.is_empty())
                                    .unwrap_or("unknown unit")
                            ),
                            crate::spectral::Mode::Spectrogram => {
                                format!("Y: Frequency (Hz) / color: dB re 1 {power_units}")
                            }
                            _ => power_units,
                        };
                        if fields_only {
                            if let Some(old) = pane.analysis.get_untracked() {
                                result.options = old.options;
                                result.title = old.title;
                                result.units = old.units;
                                result.condition = old.condition;
                                result.summary = format!("{}\n{}", old.summary, result.summary);
                            }
                            let mut all = pane
                                .analysis
                                .get_untracked()
                                .map(|old| old.series.series().to_vec())
                                .unwrap_or_default();
                            for item in result.series.series() {
                                if !all.iter().any(|old| old.id() == item.id()) {
                                    all.push(item.clone());
                                }
                            }
                            result.series = TimeSeriesSet::new(all).unwrap();
                        }
                        workbench.set_analysis_panel(pane, result);
                    } else {
                        pane.lines.update(|lines| {
                            for series in series {
                                if lines.iter().any(|line| line.source == series.id()) {
                                    continue;
                                }
                                lines.push(crate::state::Line {
                                    id: workbench.id(),
                                    source: series.id().into(),
                                    name: series.label().into(),
                                    data: series.data().clone(),
                                    revision: 0,
                                });
                            }
                        });
                        if !fields_only && !preserve_bounds {
                            pane.fit_axes(plot::interaction::Axes::BOTH);
                        }
                    }
                    if !spectral {
                        let placements = pane.lines.with_untracked(|lines| {
                            lines
                                .iter()
                                .filter(|line| !previous_ids.contains(&line.id))
                                .filter_map(|line| {
                                    inputs
                                        .iter()
                                        .find(|(source, _, _, _)| source == &line.source)
                                        .map(|(_, _, _, token)| {
                                            crate::profile_instance::Placement {
                                                pane: pane.id,
                                                item: profile::identity::Item::Field(token.clone()),
                                                lines: BTreeMap::from([(token.clone(), line.id)]),
                                            }
                                        })
                                })
                                .collect::<Vec<_>>()
                        });
                        workbench.profiles.instances.update(|instances| {
                            if let Some(instance) = instances.get_mut(&key) {
                                instance.placements.extend(placements);
                            }
                        });
                    }
                    if fields_only {
                        return;
                    }

                    workbench.restore_preset_appearance(pane, &recipe, &inputs, &instance);
                    let baseline = crate::preset_capture::capture(
                        workbench,
                        &crate::preset_capture::CaptureKind::Plot(pane.id),
                        &recipe.name,
                    )
                    .ok()
                    .and_then(|bundle| serde_json::to_string(&bundle.document).ok())
                    .unwrap_or_default();
                    if instance
                        .definition
                        .plot_presets
                        .iter()
                        .any(|plot| plot.name == recipe.name)
                        || instance
                            .definition
                            .views
                            .iter()
                            .any(|plot| plot.name == recipe.name)
                    {
                        pane.preset.set(Some(PresetSource {
                            path: key.path.clone(),
                            name: recipe.name.clone(),
                            baseline,
                        }));
                    }
                }
                Err(error) => workbench.status.set(format!("{}: {error}", recipe.name)),
            }
            pane.loading.set(None);
            workbench.preset_plot_finished();
        });
    }
    fn preset_plot_finished(self) {
        self.authoring.restoring.update_value(|pending| {
            if let Some(count) = pending.get_mut(&self.view().id) {
                *count = count.saturating_sub(1);
            }
        });
        self.finish_view_restore();
        self.finish_workspace_load();
    }
    fn finish_view_restore(self) {
        let done = self
            .authoring
            .restoring
            .with_value(|pending| pending.get(&self.view().id) == Some(&0));
        if !done {
            return;
        }
        self.authoring.restoring.update_value(|pending| {
            pending.remove(&self.view().id);
        });
        if !self.view_exists() {
            return;
        }
        if let Some(mut source) = self.view().preset.get_untracked()
            && let Ok(bundle) = crate::preset_capture::capture(
                self,
                &crate::preset_capture::CaptureKind::View,
                &source.name,
            )
        {
            source.baseline = serde_json::to_string(&bundle.document).unwrap_or_default();
            self.view().preset.set(Some(source));
        }
    }
    fn restore_preset_appearance(
        self,
        pane: Pane,
        recipe: &PlotPreset,
        inputs: &[(String, String, SeriesData, String)],
        instance: &crate::profile_instance::AppliedProfile,
    ) {
        let mut order = BTreeMap::new();
        for line in pane.lines.get_untracked() {
            let source = line
                .source
                .strip_prefix("@spectrum/")
                .unwrap_or(&line.source);
            let Some((_, _, _, token)) = inputs.iter().find(|(id, _, _, _)| id == source) else {
                continue;
            };
            let Some(style) = recipe.lines.iter().find(|line| &line.source == token) else {
                continue;
            };
            apply_line_appearance(pane, line.id, style);
            order.insert(format!("line:{token}"), line.id);
        }
        for token in &recipe.geometry {
            if let Some(id) = instance.ids.geometry.get(token) {
                pane.geometry.update(|ids| {
                    ids.insert(*id);
                });
                order.insert(format!("geometry:{token}"), *id);
                if let Some(style) = recipe.geometry_style.get(token) {
                    if let Some(mut item) = self.geometry_item(*id) {
                        item.width = style.width;
                        item.shade = style.shade;
                        item.extend = style.extend;
                        item.orientation = style.orientation;
                        item.labels = style.labels;
                        self.update_geometry(item, false);
                    }
                    if style.hidden {
                        pane.hidden_geometry.update(|hidden| {
                            hidden.insert(*id);
                        });
                    }
                }
            }
        }
        self.profiles.instances.update(|instances| {
            if let Some(instance) = instances.get_mut(&instance.key) {
                for token in &recipe.geometry {
                    let item = profile::identity::Item::Geometry(token.clone());
                    if !instance
                        .placements
                        .iter()
                        .any(|placement| placement.pane == pane.id && placement.item == item)
                    {
                        instance
                            .placements
                            .push(crate::profile_instance::Placement {
                                pane: pane.id,
                                item,
                                lines: BTreeMap::new(),
                            });
                    }
                }
            }
        });
        pane.legend_order.set(
            recipe
                .legend_order
                .iter()
                .filter_map(|token| order.get(token).copied())
                .collect(),
        );
        let mut ranges = pane.plot.ranges.get_untracked();
        let mode = pane
            .analysis
            .with_untracked(|result| result.as_ref().map(|result| result.mode));
        for existing in self
            .panes()
            .get_untracked()
            .into_iter()
            .filter(|existing| existing.id != pane.id)
        {
            if existing
                .analysis
                .with_untracked(|result| result.as_ref().map(|result| result.mode))
                != mode
            {
                continue;
            }
            let links = existing.links.get_untracked();
            let previous = existing.plot.ranges.get_untracked();
            if recipe.links.x && links.x {
                ranges.x = previous.x;
            }
            if recipe.links.y && links.y {
                ranges.y = previous.y;
            }
        }
        let _ = pane.plot.set_view(ranges);
        pane.links.set(plot::zgui_link::LinkOptions {
            x: recipe.links.x,
            y: recipe.links.y,
            pointer: recipe.links.pointer,
        });
    }
}
fn mode_of(mode: &str) -> crate::spectral::Mode {
    match mode {
        "psd" => crate::spectral::Mode::Psd,
        "heatmap" => crate::spectral::Mode::Heatmap,
        "spectrogram" => crate::spectral::Mode::Spectrogram,
        _ => crate::spectral::Mode::Fft,
    }
}
fn analysis_options(
    analysis: &Analysis,
    interval: (Option<f64>, Option<f64>),
) -> crate::spectral::Options {
    crate::spectral::Options {
        mode: mode_of(&analysis.mode),
        window: match analysis.window.as_str() {
            "hamming" => crate::spectral::Window::Hamming,
            "blackman" => crate::spectral::Window::Blackman,
            "rectangular" => crate::spectral::Window::Rectangular,
            _ => crate::spectral::Window::Hann,
        },
        size: analysis.size,
        overlap: analysis.overlap,
        heatmap_bins: analysis.heatmap_bins,
        remove_dc: analysis.remove_dc,
        start: interval.0,
        end: interval.1,
    }
}
fn compute(
    inputs: Vec<(String, String, SeriesData, String)>,
    options: Option<crate::spectral::Options>,
    condition: Option<SeriesData>,
) -> Result<
    (
        Vec<TimeSeries>,
        Option<crate::analysis_state::AnalysisResult>,
    ),
    String,
> {
    let mut all = Vec::new();
    let mut heatmap = None;
    let mut summaries = Vec::new();
    for (source, name, data, _) in inputs {
        let data = if let Some(options) = &options {
            let spectrum = match crate::spectral::compute(
                &data,
                condition.as_ref(),
                options,
                &AtomicBool::new(false),
            ) {
                Ok(spectrum) => spectrum,
                Err(error) => {
                    summaries.push(format!("{name}: {error}"));
                    continue;
                }
            };
            heatmap = spectrum.heatmap;
            summaries.push(format!(
                "{name}: {} windows, {} excluded samples",
                spectrum.windows, spectrum.excluded_samples
            ));
            SeriesData::new(spectrum.points)?
        } else {
            data
        };
        all.push(TimeSeries::new(source, name, data));
    }
    if all.is_empty() && options.is_some() {
        return Err(summaries.join("; "));
    }
    let result = options.map(|options| crate::analysis_state::AnalysisResult {
        mode: options.mode,
        options,
        title: String::new(),
        units: String::new(),
        summary: summaries.join("\n"),
        condition: String::new(),
        series: TimeSeriesSet::new(all.clone()).unwrap(),
        heatmap,
    });
    Ok((all, result))
}
fn split_max(layout: &Layout) -> u64 {
    match layout {
        Layout::Split {
            id, first, second, ..
        } => (*id).max(split_max(first)).max(split_max(second)),
        _ => 0,
    }
}
fn restore_layout(recipe: &PresetLayout, panes: &[u64], split: &mut u64) -> Layout {
    match recipe {
        PresetLayout::Plot { plot } => Layout::Plot(panes[*plot]),
        PresetLayout::Tabs { plots, active } => Layout::Tabs {
            plots: plots.iter().map(|index| panes[*index]).collect(),
            active: panes[*active],
        },
        PresetLayout::Split {
            stacked,
            ratio,
            first,
            second,
        } => {
            *split += 1;
            let id = *split;
            Layout::Split {
                id,
                stacked: *stacked,
                ratio: *ratio,
                first: Box::new(restore_layout(first, panes, split)),
                second: Box::new(restore_layout(second, panes, split)),
            }
        }
    }
}

/// Legacy `views` are plot recipes in browser, without rewriting their files.
pub fn available_plots(instance: &crate::profile_instance::AppliedProfile) -> Vec<PlotPreset> {
    let mut plots = instance.definition.plot_presets.clone();
    for view in &instance.definition.views {
        let Some(prepared) = instance.views.get(&view.name) else {
            continue;
        };
        plots.push(PlotPreset {
            name: view.name.clone(),
            lines: prepared
                .fields
                .iter()
                .enumerate()
                .map(|(index, (source, _))| PresetLine {
                    source: source.clone(),
                    color: plot::palette::DEFAULT_PALETTE
                        [index % plot::palette::DEFAULT_PALETTE.len()],
                    width: 1.5,
                    radius: 3.,
                    marker: "circle".into(),
                    connection: "linear".into(),
                    pattern: "solid".into(),
                    markers: false,
                    hidden: false,
                })
                .collect(),
            geometry: view
                .geometry
                .iter()
                .map(|id| id.trim_start_matches('$').to_owned())
                .collect(),
            geometry_style: BTreeMap::new(),
            legend_order: Vec::new(),
            links: Links::default(),
            analysis: None,
        });
    }
    plots
}

pub(crate) fn apply_line_appearance(pane: Pane, id: u64, style: &PresetLine) {
    use plot::{
        line_draw::{Connection, LineDraw, LinePattern},
        plot_data::{MarkerShape, SeriesStyle},
    };
    let key = id.to_string();
    pane.plot.styles.update(|styles| {
        styles.insert(
            key.clone(),
            SeriesStyle {
                color: style.color,
                width: style.width,
                radius: style.radius,
                marker: match style.marker.as_str() {
                    "square" => MarkerShape::Square,
                    "triangle" => MarkerShape::Triangle,
                    "diamond" => MarkerShape::Diamond,
                    "cross" => MarkerShape::Cross,
                    _ => MarkerShape::Circle,
                },
            },
        );
    });
    pane.drawing.update(|drawing| {
        drawing.insert(
            id,
            LineDraw {
                connection: match style.connection.as_str() {
                    "pre" => Connection::Pre,
                    "post" => Connection::Post,
                    _ => Connection::Linear,
                },
                pattern: match style.pattern.as_str() {
                    "dashed" => LinePattern::Dashed,
                    "dotted" => LinePattern::Dotted,
                    "dash_dot" => LinePattern::DashDot,
                    "none" => LinePattern::None,
                    _ => LinePattern::Solid,
                },
                markers: style.markers,
            },
        );
    });
    if style.hidden {
        pane.plot.hidden.update(|hidden| {
            hidden.insert(key);
        });
    }
}
