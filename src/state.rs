use crate::log::LogStore;
use plot::{
    time_series::{SeriesData, TimeSeries, TimeSeriesSet},
    zgui_time_series::TimeSeriesState,
};
use plot::{
    workspace::{layout::Layout, plot_visible},
    zgui_link::LinkOptions,
    zgui_workspace::{DockPane, DockState, SeriesDrag},
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
    pub dock: DockPane,
    pub links: RwSignal<LinkOptions, LocalStorage>,
    pub lines: RwSignal<Vec<Line>, LocalStorage>,
    pub selected: RwSignal<Option<u64>, LocalStorage>,
    pub drawing: RwSignal<BTreeMap<u64, plot::line_draw::LineDraw>, LocalStorage>,
    pub plot: TimeSeriesState,
    pub series: Signal<TimeSeriesSet, LocalStorage>,
    pub view_lines: Signal<Vec<plot::zgui_series::SeriesLine>, LocalStorage>,
}
impl Pane {
    fn new(id: u64) -> Self {
        let lines = RwSignal::new_local(Vec::<Line>::new());
        let view_lines = Signal::derive_local(move || {
            lines
                .get()
                .into_iter()
                .map(|line| plot::zgui_series::SeriesLine {
                    id: line.id,
                    name: line.name,
                    data: line.data,
                    revision: line.revision,
                })
                .collect()
        });
        let series = Signal::derive_local(move || {
            TimeSeriesSet::new(
                lines
                    .get()
                    .iter()
                    .map(|line| {
                        TimeSeries::new(line.id.to_string(), line.label(), line.data.clone())
                    })
                    .collect(),
            )
            .expect("unique line IDs")
        });
        Self {
            id,
            dock: DockPane::new(id),
            links: RwSignal::new_local(LinkOptions {
                x: false,
                y: false,
                pointer: true,
            }),
            lines,
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
        self.plot.auto_fit.set(false);
        self.lines
            .update(|lines| lines.retain(|line| line.id != id));
        self.display().remove_appearance(id);
    }
}

#[derive(Clone, Copy)]
pub struct Workbench {
    pub log: RwSignal<Option<Arc<LogStore>>, LocalStorage>,
    pub status: RwSignal<String, LocalStorage>,
    pub loading: RwSignal<bool, LocalStorage>,
    pub query: RwSignal<String, LocalStorage>,
    pub sidebar_width: RwSignal<f32, LocalStorage>,
    pub sidebar_visible: RwSignal<bool, LocalStorage>,
    pub derived: RwSignal<Vec<crate::derived::DerivedField>, LocalStorage>,
    pub derived_busy: RwSignal<bool, LocalStorage>,
    pub filters_open: RwSignal<bool, LocalStorage>,
    pub filter_edit: RwSignal<Option<u64>, LocalStorage>,
    pub filter_inputs: RwSignal<Vec<crate::derived::FilterInput>, LocalStorage>,
    pub filter_outputs: RwSignal<Vec<u64>, LocalStorage>,
    pub filter_drop: NodeRef,
    pub filter_button: NodeRef,
    pub filter_drop_active: RwSignal<bool, LocalStorage>,
    pub filter_kind: RwSignal<String, LocalStorage>,
    pub filter_cutoff: RwSignal<String, LocalStorage>,
    pub filter_gain: RwSignal<String, LocalStorage>,
    pub filter_offset: RwSignal<String, LocalStorage>,
    pub filter_error: RwSignal<String, LocalStorage>,
    pub(crate) filter_generation: RwSignal<u64, LocalStorage>,
    pub active: RwSignal<u64, LocalStorage>,
    pub panes: RwSignal<Vec<Pane>, LocalStorage>,
    pub layout: RwSignal<Layout, LocalStorage>,
    pub dock: DockState,
    pub maximized: RwSignal<Option<u64>, LocalStorage>,
    pub drag: SeriesDrag,
    pub line_drag: crate::legend_drag::LegendDrag,
    pub drop_target: RwSignal<Option<u64>, LocalStorage>,
    pub selected_field: RwSignal<Option<String>, LocalStorage>,
    pub selection_anchor: RwSignal<Option<String>, LocalStorage>,
    pub selected_fields: RwSignal<BTreeSet<String>, LocalStorage>,
    pub expanded: RwSignal<BTreeSet<String>, LocalStorage>,
    owner: StoredValue<zgui::reactive::Owner, LocalStorage>,
    next_plot: RwSignal<u64, LocalStorage>,
    pub(crate) generation: RwSignal<u64, LocalStorage>,
    next_id: RwSignal<u64, LocalStorage>,
    pub(crate) cache: RwSignal<BTreeMap<String, SeriesData>, LocalStorage>,
}
impl Workbench {
    pub fn new() -> Self {
        let panes = RwSignal::new_local(vec![Pane::new(1)]);
        let layout = RwSignal::new_local(Layout::Plot(1));
        let dock = DockState::new(
            layout,
            Signal::derive_local(move || {
                panes.with(|panes| panes.iter().map(|pane| pane.dock).collect())
            }),
        );
        Self {
            log: RwSignal::new_local(None),
            status: RwSignal::new_local("Open a PX4 .ulg file to begin".into()),
            loading: RwSignal::new_local(false),
            query: RwSignal::new_local(String::new()),
            sidebar_width: RwSignal::new_local(280.0),
            sidebar_visible: RwSignal::new_local(true),
            derived: RwSignal::new_local(Vec::new()),
            derived_busy: RwSignal::new_local(false),
            filters_open: RwSignal::new_local(false),
            filter_edit: RwSignal::new_local(None),
            filter_inputs: RwSignal::new_local(Vec::new()),
            filter_outputs: RwSignal::new_local(Vec::new()),
            filter_drop: NodeRef::new(),
            filter_button: NodeRef::new(),
            filter_drop_active: RwSignal::new_local(false),
            filter_kind: RwSignal::new_local("low".into()),
            filter_cutoff: RwSignal::new_local("5".into()),
            filter_gain: RwSignal::new_local("1".into()),
            filter_offset: RwSignal::new_local("0".into()),
            filter_error: RwSignal::new_local(String::new()),
            filter_generation: RwSignal::new_local(0),
            active: RwSignal::new_local(1),
            panes,
            layout,
            dock,
            maximized: RwSignal::new_local(None),
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
            next_id: RwSignal::new_local(0),
            cache: RwSignal::new_local(BTreeMap::new()),
        }
    }
    pub(crate) fn id(self) -> u64 {
        self.next_id.update(|id| *id += 1);
        self.next_id.get_untracked()
    }
    pub fn pane(self) -> Pane {
        self.find(self.active.get_untracked())
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
        self.generation.update(|id| *id += 1);
        let generation = self.generation.get_untracked();
        self.loading.set(true);
        self.status.set(format!("Loading {}...", path.display()));
        self.task(async move {
            let result = blocking(move || LogStore::read(&path)).await;
            if self.generation.get_untracked() != generation {
                return;
            }
            self.loading.set(false);
            match result {
                Ok(log) => {
                    self.filter_generation.update(|id| *id += 1);
                    self.derived_busy.set(false);
                    self.status.set(format!(
                        "{} signals | {:.2}s | {} warnings",
                        log.catalogue.len(),
                        log.duration,
                        log.warnings.len()
                    ));
                    self.cache.set(BTreeMap::new());
                    self.derived.set(Vec::new());
                    self.filters_open.set(false);
                    self.filter_edit.set(None);
                    self.filter_inputs.set(Vec::new());
                    self.filter_outputs.set(Vec::new());
                    self.filter_drop_active.set(false);
                    self.filter_error.set(String::new());
                    self.query.set(String::new());
                    self.selected_field.set(None);
                    self.selected_fields.set(BTreeSet::new());
                    self.selection_anchor.set(None);
                    self.expanded.set(BTreeSet::new());
                    self.line_drag.cancel();
                    self.drag.state.set(None);
                    self.drop_target.set(None);
                    for pane in self.panes.get_untracked() {
                        pane.lines.set(Vec::new());
                        pane.selected.set(None);
                        pane.plot.hidden.set(Default::default());
                        pane.plot.styles.set(Default::default());
                        pane.drawing.set(Default::default());
                        pane.plot.hover.set(None);
                        pane.plot.auto_fit.set(true);
                    }
                    self.log.set(Some(Arc::new(log)));
                }
                Err(error) => self.status.set(error),
            }
        });
    }
    pub fn add(self, source: String) {
        if self.loading.get_untracked() {
            return;
        }
        let Some(log) = self.log.get_untracked() else {
            return;
        };
        let pane = self.pane();
        if let Some(id) = pane.lines.with_untracked(|lines| {
            lines
                .iter()
                .find(|line| line.source == source)
                .map(|line| line.id)
        }) {
            self.select(self.active.get_untracked(), id);
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
                        source,
                        name: field.name,
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
                            name: source.clone(),
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
        self.panes
            .with_untracked(|panes| panes.iter().find(|pane| pane.id == id).copied())
    }
    pub fn activate(self, id: u64) {
        if self.find(id).is_some() {
            self.layout.update(|layout| {
                layout.activate(id);
            });
            self.active.set(id);
        }
    }
    pub fn add_plot(self, target: u64, stacked: bool, tab: bool) -> Pane {
        self.finish_dock();
        let id = self.next_plot.get_untracked();
        self.next_plot.set(id + 1);
        let pane = self.owner.with_value(|owner| owner.with(|| Pane::new(id)));
        self.layout.update(|layout| {
            if tab {
                layout.add_plot_tab(target, id);
            } else {
                layout.split(target, id, stacked);
            }
        });
        self.panes.update(|panes| panes.push(pane));
        self.maximized.set(None);
        self.activate(id);
        pane
    }
    pub fn duplicate_plot(self, source: Pane) {
        let copy = self.add_plot(source.id, false, false);
        let hidden = source.plot.hidden.get_untracked();
        let mut copied_hidden = BTreeSet::new();
        let mut styles = BTreeMap::new();
        let mut drawing = BTreeMap::new();
        let lines = source
            .lines
            .get_untracked()
            .into_iter()
            .map(|mut line| {
                let old = line.id.to_string();
                let options = source
                    .drawing
                    .with_untracked(|drawing| drawing.get(&line.id).copied().unwrap_or_default());
                line.id = self.id();
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
        copy.lines.set(lines);
        copy.plot.hidden.set(copied_hidden);
        copy.plot.styles.set(styles);
        copy.drawing.set(drawing);
        copy.plot.ranges.set(source.plot.ranges.get_untracked());
        copy.plot.auto_fit.set(source.plot.auto_fit.get_untracked());
        copy.links.set(source.links.get_untracked());
    }
    pub fn close_plot(self, id: u64) {
        self.line_drag.cancel();
        if self.find(id).is_none() {
            return;
        }
        self.finish_dock();
        self.dock
            .nodes
            .update_value(|nodes| nodes.retain(|(pane, tab), _| *pane != id && *tab != id));
        self.dock
            .strips
            .update_value(|strips| strips.retain(|strip| strip.pane != id));
        let current = self.active.get_untracked();
        let tabs = self.layout.get_untracked().tab_ids(id);
        let neighbor = tabs.iter().position(|tab| *tab == id).and_then(|index| {
            tabs.get(index + 1)
                .or_else(|| index.checked_sub(1).and_then(|index| tabs.get(index)))
                .copied()
        });
        self.layout.update(|layout| layout.remove(id));
        self.panes
            .update(|panes| panes.retain(|pane| pane.id != id));
        if self.maximized.get_untracked() == Some(id) {
            self.maximized.set(None);
        }
        let panes = self.panes.get_untracked();
        if panes.is_empty() {
            self.add_plot(id, false, false);
        } else if current == id {
            let layout = self.layout.get_untracked();
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
        self.maximized
            .update(|current| *current = if *current == Some(id) { None } else { Some(id) });
    }
    pub fn hit_plot(self, x: f32, y: f32) -> Option<u64> {
        if self.filters_open.get_untracked() {
            return None;
        }
        let layout = self.layout.get_untracked();
        let maximized = self.maximized.get_untracked();
        self.panes.with_untracked(|panes| {
            panes.iter().find_map(|pane| {
                if !plot_visible(pane.id, maximized, layout.clone()) {
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
    fn finish_dock(self) {
        let done = self.dock.settle_done.get_value();
        self.dock.settle_done.set_value(None);
        if let Some(done) = done {
            done();
        }
        self.dock.settle_frame.set_value(None);
        self.dock.settling.set(None);
        self.dock.rebase.set(false);
        self.dock.drag.set(None);
        self.dock.target.set(None);
        self.dock.group_target.set(None);
        self.dock.preview.set(None);
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
        self.maximized.set(None);
        self.layout.update(Layout::equalize);
    }
}
