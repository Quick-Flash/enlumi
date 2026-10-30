use crate::log::LogStore;
use plot::{
    time_series::{SeriesData, TimeSeries, TimeSeriesSet},
    zgui_time_series::TimeSeriesState,
};
use plot::{
    workspace::{layout::Layout, plot_visible},
    zgui_link::LinkOptions,
    zgui_workspace::{DockPane, SeriesDrag},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    path::PathBuf,
    sync::Arc,
};
use zgui::prelude::*;

#[derive(Clone)]
pub struct Line {
    pub id: u64,
    pub source: String,
    pub name: String,
    pub data: SeriesData,
    pub revision: u64,
}
impl Line {
    pub fn label(&self) -> String {
        self.name.clone()
    }
}

#[derive(Clone, Copy)]
pub struct Pane {
    pub id: u64,
    pub loading: RwSignal<Option<String>, LocalStorage>,
    pub dock: DockPane,
    pub preset: RwSignal<Option<crate::preset::PresetSource>, LocalStorage>,
    pub background: RwSignal<Option<[u8; 3]>, LocalStorage>,
    pub menu_area: RwSignal<Option<NodeRef>, LocalStorage>,
    pub analysis: RwSignal<Option<crate::analysis_state::AnalysisResult>, LocalStorage>,
    pub trajectory: RwSignal<Option<crate::trajectory::Spec>, LocalStorage>,
    pub trajectory_setup: RwSignal<bool, LocalStorage>,
    pub trajectory_data: RwSignal<Option<Arc<crate::trajectory::Tracks>>, LocalStorage>,
    pub timeline_link: RwSignal<bool, LocalStorage>,
    pub trajectory_slots: StoredValue<BTreeMap<(usize, bool), NodeRef>, LocalStorage>,
    pub trajectory_drop: RwSignal<Option<(usize, bool)>, LocalStorage>,
    pub snapshot: plot::zgui_svg::PlotSnapshotSource,
    pub links: RwSignal<LinkOptions, LocalStorage>,
    pub lines: RwSignal<Vec<Line>, LocalStorage>,
    pub geometry: RwSignal<BTreeSet<u64>, LocalStorage>,
    pub legend_order: RwSignal<Vec<u64>, LocalStorage>,
    pub hidden_geometry: RwSignal<BTreeSet<u64>, LocalStorage>,
    pub selected: RwSignal<Option<u64>, LocalStorage>,
    pub drawing: RwSignal<BTreeMap<u64, plot::line_draw::LineDraw>, LocalStorage>,
    pub plot: TimeSeriesState,
    pub series: Signal<TimeSeriesSet, LocalStorage>,
    pub view_lines: Signal<Vec<plot::zgui_series::SeriesLine>, LocalStorage>,
}
impl Pane {
    pub(crate) fn new(id: u64) -> Self {
        let lines = RwSignal::new_local(Vec::<Line>::new());
        let trajectory = RwSignal::new_local(None::<crate::trajectory::Spec>);
        let view_lines = Signal::derive_local(move || {
            let pairs = trajectory.get().map(|spec| spec.pairs());
            lines
                .get()
                .into_iter()
                .filter(|line| {
                    pairs
                        .as_ref()
                        .is_none_or(|pairs| pairs.iter().any(|pair| pair.y == line.source))
                })
                .map(|line| plot::zgui_series::SeriesLine {
                    id: line.id,
                    name: line.name.clone(),
                    data: line.data,
                    revision: line.revision,
                })
                .collect()
        });
        let series = Memo::new_owning(move |_| {
            let series = lines.with(|lines| {
                TimeSeriesSet::new(
                    lines
                        .iter()
                        .map(|line| {
                            TimeSeries::new(line.id.to_string(), line.label(), line.data.clone())
                        })
                        .collect(),
                )
                .expect("unique line IDs")
            });
            (series, true)
        });
        let series = Signal::derive_local(move || series.get());
        Self {
            id,
            loading: RwSignal::new_local(None),
            dock: DockPane::new(id),
            preset: RwSignal::new_local(None),
            background: RwSignal::new_local(None),
            menu_area: RwSignal::new_local(None),
            analysis: RwSignal::new_local(None),
            trajectory,
            trajectory_setup: RwSignal::new_local(false),
            trajectory_data: RwSignal::new_local(None),
            timeline_link: RwSignal::new_local(true),
            trajectory_slots: StoredValue::new_local(BTreeMap::new()),
            trajectory_drop: RwSignal::new_local(None),
            snapshot: StoredValue::new_local(None),
            links: RwSignal::new_local(LinkOptions {
                x: false,
                y: false,
                pointer: true,
            }),
            lines,
            geometry: RwSignal::new_local(BTreeSet::new()),
            legend_order: RwSignal::new_local(Vec::new()),
            hidden_geometry: RwSignal::new_local(BTreeSet::new()),
            view_lines,
            selected: RwSignal::new_local(None),
            drawing: RwSignal::new_local(BTreeMap::new()),
            plot: TimeSeriesState::default(),
            series,
        }
    }
    pub fn display(self) -> plot::zgui_series::SeriesPane {
        plot::zgui_series::SeriesPane {
            id: self.id,
            lines: self.view_lines,
            selected: self.selected,
            drawing: self.drawing,
            plot: self.plot,
            series: self.series,
            links: self.links,
        }
    }
    #[cfg(test)]
    pub fn selection(self) -> Option<Line> {
        let id = self.selected.get_untracked()?;
        self.lines
            .with_untracked(|lines| lines.iter().find(|line| line.id == id).cloned())
    }
    pub fn remove_line(self, id: u64) {
        let source = self.lines.with_untracked(|lines| {
            lines
                .iter()
                .find(|line| line.id == id)
                .map(|line| line.source.clone())
        });
        let mut keep_input = false;
        if let Some(source) = source {
            self.trajectory.update(|spec| {
                if let Some(spec) = spec {
                    spec.extra.retain(|pair| pair.y != source);
                    if spec.y == source {
                        if spec.extra.is_empty() {
                            spec.y.clear();
                        } else {
                            let pair = spec.extra.remove(0);
                            spec.x = pair.x;
                            spec.y = pair.y;
                            spec.fix = pair.fix;
                            spec.gps_scale = pair.gps_scale;
                        }
                    }
                    keep_input = spec
                        .pairs()
                        .iter()
                        .any(|pair| pair.x == source || pair.fix == source);
                }
            });
        }
        self.plot.auto_fit.set(false);
        if !keep_input {
            self.lines
                .update(|lines| lines.retain(|line| line.id != id));
        }
        self.display().remove_appearance(id);
    }
}

#[derive(Clone, Copy)]
pub struct Workbench {
    pub timeline: crate::timeline::Timeline,
    pub authoring: crate::preset_ui::Authoring,
    pub views: crate::view_state::Views,
    pub workspaces: crate::workspace_state::Workspaces,
    pub(crate) bound_view: Option<crate::view_state::WorkspaceView>,
    pub screenshot_clipboard: StoredValue<plot::clipboard::ScreenshotClipboard, LocalStorage>,
    pub log: RwSignal<Option<Arc<LogStore>>, LocalStorage>,
    pub status: RwSignal<String, LocalStorage>,
    pub loading: RwSignal<bool, LocalStorage>,
    pub field_query: RwSignal<String, LocalStorage>,
    pub browser_fields: RwSignal<Vec<String>, LocalStorage>,
    pub sidebar_width: RwSignal<f32, LocalStorage>,
    pub sidebar_visible: RwSignal<bool, LocalStorage>,
    pub derived: RwSignal<Vec<crate::derived::DerivedField>, LocalStorage>,
    pub geometry: crate::geometry_state::GeometryUi,
    pub bands: crate::band_state::BandUi,
    pub measurement: crate::measurement_state::MeasurementUi,
    pub profiles: crate::profile_state::ProfileUi,
    pub derived_busy: RwSignal<bool, LocalStorage>,
    pub filters_open: RwSignal<bool, LocalStorage>,
    pub filter_mode: RwSignal<crate::filter_mode::FilterMode, LocalStorage>,
    pub filter_inputs: RwSignal<Vec<crate::derived::FilterInput>, LocalStorage>,
    pub filter_input_ui: crate::filter_input_drag::FilterInputUi,
    pub filter_outputs: RwSignal<Vec<u64>, LocalStorage>,
    pub filter_drop: NodeRef,
    pub filter_button: NodeRef,
    pub filter_drop_active: RwSignal<bool, LocalStorage>,
    pub filter_kind: RwSignal<String, LocalStorage>,
    pub filter_names: RwSignal<Vec<String>, LocalStorage>,
    pub filter_angle_unit: RwSignal<String, LocalStorage>,
    pub filter_zero_phase: RwSignal<bool, LocalStorage>,
    pub filter_window: RwSignal<String, LocalStorage>,
    pub filter_initial: RwSignal<String, LocalStorage>,
    pub filter_cutoff: RwSignal<String, LocalStorage>,
    pub filter_gain: RwSignal<String, LocalStorage>,
    pub filter_offset: RwSignal<String, LocalStorage>,
    pub script_source: RwSignal<String, LocalStorage>,
    pub script_names: RwSignal<String, LocalStorage>,
    pub script_output_ids: RwSignal<Vec<Option<u64>>, LocalStorage>,
    pub script_busy: RwSignal<bool, LocalStorage>,
    pub filter_cancel: RwSignal<Arc<std::sync::atomic::AtomicBool>, LocalStorage>,
    pub filter_error: RwSignal<String, LocalStorage>,
    pub(crate) filter_generation: RwSignal<u64, LocalStorage>,
    pub drag: SeriesDrag,
    pub line_drag: crate::legend_drag::LegendDrag,
    pub drop_target: RwSignal<Option<u64>, LocalStorage>,
    pub selected_field: RwSignal<Option<String>, LocalStorage>,
    pub selection_anchor: RwSignal<Option<String>, LocalStorage>,
    pub selected_fields: RwSignal<BTreeSet<String>, LocalStorage>,
    pub expanded: RwSignal<BTreeSet<String>, LocalStorage>,
    pub(crate) owner: StoredValue<zgui::reactive::Owner, LocalStorage>,
    pub(crate) next_plot: RwSignal<u64, LocalStorage>,
    pub(crate) generation: RwSignal<u64, LocalStorage>,
    pub(crate) loaded_generation: RwSignal<u64, LocalStorage>,
    next_id: RwSignal<u64, LocalStorage>,
    pub(crate) cache: RwSignal<BTreeMap<String, SeriesData>, LocalStorage>,
}
impl Workbench {
    pub fn new() -> Self {
        let views = crate::view_state::Views::new();
        Self {
            timeline: crate::timeline::Timeline::new(),
            authoring: crate::preset_ui::Authoring::new(),
            views,
            workspaces: crate::workspace_state::Workspaces::new(views),
            bound_view: None,
            geometry: crate::geometry_state::GeometryUi::new(),
            screenshot_clipboard: StoredValue::new_local(Default::default()),
            bands: crate::band_state::BandUi::new(),
            measurement: crate::measurement_state::MeasurementUi::new(),
            profiles: crate::profile_state::ProfileUi::new(),
            log: RwSignal::new_local(None),
            status: RwSignal::new_local("Open a log file or drop one into the window".into()),
            loading: RwSignal::new_local(false),
            field_query: RwSignal::new_local(String::new()),
            browser_fields: RwSignal::new_local(Vec::new()),
            sidebar_width: RwSignal::new_local(280.0),
            sidebar_visible: RwSignal::new_local(true),
            derived: RwSignal::new_local(Vec::new()),
            derived_busy: RwSignal::new_local(false),
            filters_open: RwSignal::new_local(false),
            filter_mode: RwSignal::new_local(crate::filter_mode::FilterMode::New),
            filter_inputs: RwSignal::new_local(Vec::new()),
            filter_input_ui: crate::filter_input_drag::FilterInputUi::new(),
            filter_outputs: RwSignal::new_local(Vec::new()),
            filter_drop: NodeRef::new(),
            filter_button: NodeRef::new(),
            filter_drop_active: RwSignal::new_local(false),
            filter_kind: RwSignal::new_local("low".into()),
            filter_names: RwSignal::new_local(
                ["Magnitude", "Roll", "Pitch", "Yaw"]
                    .map(String::from)
                    .to_vec(),
            ),
            filter_angle_unit: RwSignal::new_local("rad".into()),
            filter_zero_phase: RwSignal::new_local(false),
            filter_window: RwSignal::new_local("5".into()),
            filter_initial: RwSignal::new_local("0".into()),
            filter_cutoff: RwSignal::new_local("5".into()),
            filter_gain: RwSignal::new_local("1".into()),
            filter_offset: RwSignal::new_local("0".into()),
            script_source: RwSignal::new_local(crate::script_compile::EXAMPLE.into()),
            script_names: RwSignal::new_local("Result".into()),
            script_output_ids: RwSignal::new_local(Vec::new()),
            script_busy: RwSignal::new_local(false),
            filter_cancel: RwSignal::new_local(Arc::new(std::sync::atomic::AtomicBool::new(false))),
            filter_error: RwSignal::new_local(String::new()),
            filter_generation: RwSignal::new_local(0),
            drag: SeriesDrag::default(),
            line_drag: crate::legend_drag::LegendDrag::new(),
            drop_target: RwSignal::new_local(None),
            selected_field: RwSignal::new_local(None),
            selection_anchor: RwSignal::new_local(None),
            selected_fields: RwSignal::new_local(BTreeSet::new()),
            expanded: RwSignal::new_local(BTreeSet::new()),
            owner: StoredValue::new_local(
                zgui::reactive::Owner::current().expect("workspace owner"),
            ),
            next_plot: RwSignal::new_local(2),
            generation: RwSignal::new_local(0),
            loaded_generation: RwSignal::new_local(0),
            next_id: RwSignal::new_local(0),
            cache: RwSignal::new_local(BTreeMap::new()),
        }
    }
    pub(crate) fn id(self) -> u64 {
        self.next_id.update(|id| *id += 1);
        self.next_id.get_untracked()
    }
    pub fn pane(self) -> Pane {
        self.find(self.active().get_untracked())
            .expect("active plot exists")
    }
    pub fn select(self, pane: u64, id: u64) {
        self.activate(pane);
        let Some(pane) = self.find(pane) else {
            return;
        };
        pane.selected.set(Some(id));
    }
    pub fn open(self, path: PathBuf) {
        let unfinished = self
            .authoring
            .workspace_load
            .with_value(|load| load.as_ref().map(|load| load.workspace));
        self.clear_workspace_load();
        self.profiles.cancel();
        for view in self.all_views() {
            view.analysis.cancel();
        }
        self.cancel_band_source();
        self.bands.jobs.update_value(BTreeMap::clear);
        self.generation.update(|id| *id += 1);
        let generation = self.generation.get_untracked();
        self.loading.set(true);
        self.status.set(format!("Loading {}...", path.display()));
        self.task(async move {
            let result = blocking(move || LogStore::read(&path)).await;
            if self.generation.get_untracked() != generation {
                return;
            }
            match result {
                Ok(log) => {
                    let log = Arc::new(log);
                    let session = if let Some(session) =
                        crate::log_session::Session::capture(self, unfinished)
                    {
                        self.status.set("Refreshing workspace data…".into());
                        let replacement = log.clone();
                        Some(blocking(move || session.refresh(replacement)).await)
                    } else {
                        None
                    };
                    if self.generation.get_untracked() != generation {
                        return;
                    }
                    self.finish_view_gestures();
                    self.loaded_generation.set(generation);
                    self.loading.set(false);
                    self.measurement.clear();
                    if session.is_none() {
                        self.profiles.reset();
                    }
                    self.authoring.pending.set_value(None);
                    self.authoring.saved.set_value(None);
                    self.authoring.browse.set(None);
                    self.authoring.restoring.set_value(Default::default());
                    self.authoring.open.set(false);
                    if session.is_none() {
                        for view in self.all_views() {
                            view.preset.set(None);
                            view.analysis.reset();
                        }
                    }

                    self.cancel_filter_run();
                    self.status.set(format!(
                        "{} signals | {:.2}s | {} warnings",
                        log.catalogue.len(),
                        log.duration,
                        log.warnings.len()
                    ));
                    self.filters_open.set(false);
                    self.filter_mode.set(crate::filter_mode::FilterMode::New);
                    self.filter_inputs.set(Vec::new());
                    self.filter_outputs.set(Vec::new());
                    self.filter_drop_active.set(false);
                    self.filter_error.set(String::new());
                    self.line_drag.cancel();
                    self.drag.state.set(None);
                    self.drop_target.set(None);
                    if let Some(session) = session {
                        if !log
                            .settings
                            .contains_key(&self.profiles.flight.get_untracked())
                        {
                            self.profiles
                                .flight
                                .set(log.settings.keys().next().copied().unwrap_or(1));
                        }
                        self.log.set(Some(log));
                        self.restore_log_session(session);
                        self.apply_default_profiles();
                        return;
                    }
                    self.cache.set(BTreeMap::new());
                    self.derived.set(Vec::new());
                    self.field_query.set(String::new());
                    self.selected_field.set(None);
                    self.selected_fields.set(BTreeSet::new());
                    self.selection_anchor.set(None);
                    self.expanded.set(BTreeSet::new());
                    for pane in self.all_panes() {
                        if pane.analysis.get_untracked().is_some() {
                            self.close_plot(pane.id);
                        }
                    }
                    for pane in self.all_panes() {
                        pane.preset.set(None);
                        pane.lines.set(Vec::new());
                        pane.analysis.set(None);
                        pane.selected.set(None);
                        pane.plot.hidden.set(Default::default());
                        pane.plot.styles.set(Default::default());
                        pane.drawing.set(Default::default());
                        pane.plot.hover.set(None);
                        pane.plot.auto_fit.set(true);
                    }
                    self.clear_generated_bands();
                    self.profiles
                        .flight
                        .set(log.settings.keys().next().copied().unwrap_or(1));
                    self.log.set(Some(log));
                    self.timeline.time.set(None);
                    if let Some(source) = self.workspace().source.get_untracked() {
                        self.authoring
                            .pending
                            .set_value(Some(crate::preset_ui::ApplyRequest {
                                key: crate::profile::identity::ProfileKey::new(
                                    source.path,
                                    self.profiles.flight.get_untracked(),
                                ),
                                name: source.name,
                                view: self.views.active.get_untracked(),
                                action: crate::preset_ui::ApplyAction::Workspace,
                            }));
                    }
                    self.apply_default_profiles();
                }
                Err(error) => {
                    self.loading.set(false);
                    self.status.set(error);
                }
            }
        });
    }
    pub fn add(self, source: String) {
        self.bind_view().add_view_field(source);
    }
    fn add_view_field(self, source: String) {
        if let Some(source) = source.strip_prefix("@analysis/") {
            self.drop_analysis_sources(
                vec![source.to_owned()],
                Some(self.active().get_untracked()),
            );
            return;
        }
        if self.loading.get_untracked() {
            return;
        }
        let Some(log) = self.log.get_untracked() else {
            return;
        };
        let active = self.pane();
        let pane = if active.analysis.get_untracked().is_some() {
            self.add_plot(active.id, false, true)
        } else {
            active
        };
        pane.trajectory.update(|spec| {
            if let Some(spec) = spec {
                spec.add_source(&source);
            }
        });
        if let Some(id) = pane.lines.with_untracked(|lines| {
            lines
                .iter()
                .find(|line| line.source == source)
                .map(|line| line.id)
        }) {
            self.select(self.active().get_untracked(), id);
            return;
        }
        if crate::derived::id(&source).is_some() {
            if self.derived_busy.get_untracked() {
                self.status.set("Wait for filters to finish".into());
                return;
            }
            let field = self.derived.with_untracked(|fields| {
                fields.iter().find(|field| field.key() == source).cloned()
            });
            if let Some(field) = field
                && let Some(data) = field.data
            {
                let id = self.id();
                self.assign_line_color(pane, id);
                pane.lines.update(|lines| {
                    lines.push(Line {
                        id,
                        name: self.field_name(&source),
                        source,
                        data,
                        revision: 0,
                    })
                });
                pane.selected.set(Some(id));
                pane.fit_axes(plot::interaction::Axes::BOTH);
            }
            return;
        }
        let generation = self.generation.get_untracked();
        let cached = self
            .cache
            .with_untracked(|cache| cache.get(&source).cloned());
        self.status.set(format!("Preparing {source}..."));
        self.task(async move {
            let key = source.clone();
            let result = match cached {
                Some(data) => Ok(data),
                None => blocking(move || log.prepare(&key)).await,
            };
            if self.generation.get_untracked() != generation || self.find(pane.id).is_none() {
                return;
            }
            match result {
                Ok(data) => {
                    if pane
                        .lines
                        .with_untracked(|lines| lines.iter().any(|line| line.source == source))
                    {
                        return;
                    }
                    self.cache.update(|cache| {
                        cache.insert(source.clone(), data.clone());
                    });
                    let id = self.id();
                    self.assign_line_color(pane, id);
                    pane.lines.update(|lines| {
                        lines.push(Line {
                            id,
                            source: source.clone(),
                            name: self.field_name(&source),
                            data,
                            revision: 0,
                        })
                    });
                    // Do not switch active pane when work finishes after user selected another pane.
                    pane.selected.set(Some(id));
                    pane.fit_axes(plot::interaction::Axes::BOTH);
                    self.status.set(format!("Added {source}"));
                }
                Err(error) => self.status.set(error),
            }
        });
    }
    pub(crate) fn task(self, future: impl Future<Output = ()> + 'static) {
        self.owner.with_value(|owner| {
            owner.with(|| {
                spawn_local(future);
            })
        });
    }
    pub fn find(self, id: u64) -> Option<Pane> {
        self.all_views().iter().find_map(|view| {
            view.panes
                .with_untracked(|panes| panes.iter().find(|pane| pane.id == id).copied())
        })
    }
    pub fn activate(self, id: u64) {
        if let Some(workbench) = self.for_plot(id) {
            workbench.layout().update(|layout| {
                layout.activate(id);
            });
            workbench.active().set(id);
        }
    }
    pub fn add_plot(self, target: u64, stacked: bool, tab: bool) -> Pane {
        self.for_plot(target)
            .unwrap_or_else(|| self.bind_view())
            .add_view_plot(target, stacked, tab)
    }
    fn add_view_plot(self, target: u64, stacked: bool, tab: bool) -> Pane {
        self.finish_dock();
        let id = self.next_plot.get_untracked();
        self.next_plot.set(id + 1);
        let pane = self.owner.with_value(|owner| owner.with(|| Pane::new(id)));
        self.layout().update(|layout| {
            if tab {
                layout.add_plot_tab(target, id);
            } else {
                layout.split(target, id, stacked);
            }
        });
        self.panes().update(|panes| panes.push(pane));
        self.maximized().set(None);
        self.activate(id);
        pane
    }
    pub fn duplicate_plot(self, source: Pane) {
        let copy = self.add_plot(source.id, false, false);
        self.copy_plot(source, copy);
    }
    pub(crate) fn copy_plot(self, source: Pane, copy: Pane) {
        let hidden = source.plot.hidden.get_untracked();
        let mut copied_hidden = BTreeSet::new();
        let mut styles = BTreeMap::new();
        let mut drawing = BTreeMap::new();
        let mut ids = BTreeMap::new();
        let lines = source
            .lines
            .get_untracked()
            .into_iter()
            .map(|mut line| {
                let old = line.id.to_string();
                let options = source
                    .drawing
                    .with_untracked(|drawing| drawing.get(&line.id).copied().unwrap_or_default());
                let old_id = line.id;
                line.id = self.id();
                ids.insert(old_id, line.id);
                drawing.insert(line.id, options);
                line.revision = 0;
                let new = line.id.to_string();
                if hidden.contains(&old) {
                    copied_hidden.insert(new.clone());
                }
                styles.insert(new, source.plot.style(&old));
                line
            })
            .collect();
        copy.analysis.set(source.analysis.get_untracked());
        copy.trajectory.set(source.trajectory.get_untracked());
        copy.trajectory_setup
            .set(source.trajectory_setup.get_untracked());
        copy.background.set(source.background.get_untracked());
        if let Some(title) = self
            .profiles
            .titles
            .with_untracked(|titles| titles.get(&source.id).cloned())
        {
            self.profiles.titles.update(|titles| {
                titles.insert(copy.id, title);
            });
        }
        copy.lines.set(lines);
        copy.plot.hidden.set(copied_hidden);
        copy.plot.styles.set(styles);
        copy.drawing.set(drawing);
        copy.plot.ranges.set(source.plot.ranges.get_untracked());
        copy.plot.auto_fit.set(source.plot.auto_fit.get_untracked());
        copy.links.set(source.links.get_untracked());
        copy.timeline_link.set(source.timeline_link.get_untracked());
        copy.geometry.set(source.geometry.get_untracked());
        copy.legend_order
            .set(source.legend_order.with_untracked(|order| {
                order
                    .iter()
                    .map(|id| ids.get(id).copied().unwrap_or(*id))
                    .collect()
            }));
        self.profiles.instances.update(|instances| {
            for instance in instances.values_mut() {
                let copied = instance
                    .placements
                    .iter()
                    .filter(|placement| placement.pane == source.id)
                    .map(|placement| crate::profile_instance::Placement {
                        pane: copy.id,
                        item: placement.item.clone(),
                        lines: placement
                            .lines
                            .iter()
                            .filter_map(|(token, id)| ids.get(id).map(|id| (token.clone(), *id)))
                            .collect(),
                    })
                    .collect::<Vec<_>>();
                instance.placements.extend(copied);
            }
        });
        copy.selected.set(
            source
                .selected
                .get_untracked()
                .and_then(|id| ids.get(&id).copied()),
        );
        copy.plot.lod.set(source.plot.lod.get_untracked());
        copy.plot
            .annotations
            .set(source.plot.annotations.get_untracked());
        copy.plot
            .show_annotations
            .set(source.plot.show_annotations.get_untracked());
        copy.plot
            .histogram
            .set(source.plot.histogram.get_untracked());
        copy.hidden_geometry
            .set(source.hidden_geometry.get_untracked());
    }
    pub fn close_plot(self, id: u64) {
        if let Some(workbench) = self.for_plot(id) {
            workbench.close_view_plot(id);
        }
    }
    fn close_view_plot(self, id: u64) {
        if self
            .measurement
            .selection
            .get_untracked()
            .is_some_and(|measurement| measurement.pane == id)
        {
            self.measurement.clear();
        }
        if self
            .geometry
            .drag
            .get_untracked()
            .is_some_and(|drag| drag.pane == id)
        {
            self.cancel_geometry_drag();
        }

        self.line_drag.cancel();
        if self.find(id).is_none() {
            return;
        }
        self.finish_dock();
        self.dock()
            .nodes
            .update_value(|nodes| nodes.retain(|(pane, tab), _| *pane != id && *tab != id));
        self.dock()
            .strips
            .update_value(|strips| strips.retain(|strip| strip.pane != id));
        let current = self.active().get_untracked();
        let tabs = self.layout().get_untracked().tab_ids(id);
        let neighbor = tabs.iter().position(|tab| *tab == id).and_then(|index| {
            tabs.get(index + 1)
                .or_else(|| index.checked_sub(1).and_then(|index| tabs.get(index)))
                .copied()
        });
        self.layout().update(|layout| layout.remove(id));
        self.panes()
            .update(|panes| panes.retain(|pane| pane.id != id));
        if self.maximized().get_untracked() == Some(id) {
            self.maximized().set(None);
        }
        let panes = self.panes().get_untracked();
        if panes.is_empty() {
            self.add_plot(id, false, false);
        } else if current == id {
            let layout = self.layout().get_untracked();
            let next = neighbor
                .and_then(|id| panes.iter().find(|pane| pane.id == id))
                .or_else(|| panes.iter().find(|pane| layout.is_visible(pane.id)))
                .unwrap_or(&panes[0]);
            self.activate(next.id);
        }
        self.drop_target.set(None);
    }
    pub fn maximize(self, id: u64) {
        self.finish_dock();
        self.activate(id);
        self.maximized()
            .update(|current| *current = if *current == Some(id) { None } else { Some(id) });
    }
    pub fn hit_plot(self, x: f32, y: f32) -> Option<u64> {
        self.hit_plot_kind(x, y, false)
    }
    pub fn hit_any_plot(self, x: f32, y: f32) -> Option<u64> {
        self.hit_plot_kind(x, y, true)
    }
    fn hit_plot_kind(self, x: f32, y: f32, analysis: bool) -> Option<u64> {
        if self.filters_open.get_untracked() {
            return None;
        }
        let layout = self.layout().get_untracked();
        let maximized = self.maximized().get_untracked();
        self.panes().with_untracked(|panes| {
            panes.iter().find_map(|pane| {
                if (!analysis && pane.analysis.get_untracked().is_some())
                    || !plot_visible(pane.id, maximized, layout.clone())
                {
                    return None;
                }
                let bounds = pane.dock.node.window_bounds()?;
                let scale = pane.dock.node.scale();
                let px = x * scale;
                let py = y * scale;
                (px >= bounds.origin.x.0
                    && px < bounds.origin.x.0 + bounds.size.width.0
                    && py >= bounds.origin.y.0 + 28.0 * scale
                    && py < bounds.origin.y.0 + bounds.size.height.0)
                    .then_some(pane.id)
            })
        })
    }
    pub(crate) fn finish_dock(self) {
        let done = self.dock().settle_done.get_value();
        self.dock().settle_done.set_value(None);
        if let Some(done) = done {
            done();
        }
        self.dock().settle_frame.set_value(None);
        self.dock().settling.set(None);
        self.dock().rebase.set(false);
        self.dock().drag.set(None);
        self.dock().target.set(None);
        self.dock().group_target.set(None);
        self.dock().preview.set(None);
    }
    pub fn toggle_branch(self, id: String) {
        self.expanded.update(|expanded| {
            if !expanded.remove(&id) {
                expanded.insert(id);
            }
        });
    }
    pub fn toggle_sidebar(self) {
        self.line_drag.cancel();
        self.sidebar_visible.update(|shown| *shown = !*shown);
        self.drag.state.set(None);
        self.drop_target.set(None);
    }
    pub fn equalize_panels(self) {
        self.finish_dock();
        self.maximized().set(None);
        self.layout().update(Layout::equalize);
    }
}
