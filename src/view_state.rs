use crate::{
    analysis_state::AnalysisUi,
    state::{Pane, Workbench},
};
use plot::{workspace::layout::Layout, zgui_workspace::DockState};
use zgui::prelude::*;

#[derive(Clone, Copy)]
pub struct WorkspaceView {
    pub preset: RwSignal<Option<crate::preset::PresetSource>, LocalStorage>,
    pub id: u64,
    pub name: RwSignal<String, LocalStorage>,
    pub panes: RwSignal<Vec<Pane>, LocalStorage>,
    pub layout: RwSignal<Layout, LocalStorage>,
    pub active: RwSignal<u64, LocalStorage>,
    pub maximized: RwSignal<Option<u64>, LocalStorage>,
    pub dock: DockState,
    pub analysis: AnalysisUi,
}

impl WorkspaceView {
    fn new(id: u64, plot: u64, name: String) -> Self {
        let panes = RwSignal::new_local(vec![Pane::new(plot)]);
        let layout = RwSignal::new_local(Layout::Plot(plot));
        Self {
            preset: RwSignal::new_local(None),
            id,
            name: RwSignal::new_local(name),
            panes,
            layout,
            active: RwSignal::new_local(plot),
            maximized: RwSignal::new_local(None),
            analysis: AnalysisUi::new(),
            dock: DockState::new(
                layout,
                Signal::derive_local(move || {
                    panes.with(|panes| panes.iter().map(|pane| pane.dock).collect())
                }),
            ),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Views {
    pub items: RwSignal<Vec<WorkspaceView>, LocalStorage>,
    pub active: RwSignal<u64, LocalStorage>,
    pub mounted: RwSignal<std::collections::BTreeSet<u64>, LocalStorage>,
    next: RwSignal<u64, LocalStorage>,
}

impl Views {
    pub fn new() -> Self {
        Self {
            items: RwSignal::new_local(vec![WorkspaceView::new(1, 1, "Untitled".into())]),
            active: RwSignal::new_local(1),
            mounted: RwSignal::new_local(std::collections::BTreeSet::from([1])),
            next: RwSignal::new_local(2),
        }
    }
}

impl Workbench {
    pub fn view(self) -> WorkspaceView {
        self.bound_view.unwrap_or_else(|| {
            let id = self.views.active.get();
            self.views.items.with(|views| {
                views
                    .iter()
                    .find(|view| view.id == id)
                    .copied()
                    .expect("Active view exists")
            })
        })
    }
    pub fn bind_view(self) -> Self {
        if self.bound_view.is_some() {
            return self;
        }
        let id = self.views.active.get_untracked();
        self.for_view(id).expect("Active view exists")
    }
    pub fn for_view(self, id: u64) -> Option<Self> {
        self.all_views()
            .into_iter()
            .find(|view| view.id == id)
            .map(|view| {
                let mut workbench = self;
                workbench.bound_view = Some(view);
                workbench
            })
    }
    pub fn view_exists(self) -> bool {
        self.bound_view
            .is_none_or(|bound| self.all_views().iter().any(|view| view.id == bound.id))
    }
    pub fn is_current_view(self) -> bool {
        self.bound_view
            .is_none_or(|view| view.id == self.views.active.get_untracked())
    }
    pub fn panes(self) -> RwSignal<Vec<Pane>, LocalStorage> {
        self.view().panes
    }
    pub fn layout(self) -> RwSignal<Layout, LocalStorage> {
        self.view().layout
    }
    pub fn active(self) -> RwSignal<u64, LocalStorage> {
        self.view().active
    }
    pub fn maximized(self) -> RwSignal<Option<u64>, LocalStorage> {
        self.view().maximized
    }
    pub fn dock(self) -> DockState {
        self.view().dock
    }
    pub fn analysis(self) -> AnalysisUi {
        self.view().analysis
    }
    pub fn all_panes(self) -> Vec<Pane> {
        self.all_views()
            .iter()
            .flat_map(|view| view.panes.get_untracked())
            .collect()
    }
    pub fn for_plot(self, id: u64) -> Option<Self> {
        self.all_views()
            .into_iter()
            .find(|view| {
                view.panes
                    .with_untracked(|panes| panes.iter().any(|pane| pane.id == id))
            })
            .map(|view| {
                let mut workbench = self;
                workbench.bound_view = Some(view);
                workbench
            })
    }
    pub fn switch_view(self, id: u64) {
        if self.for_view(id).is_none() || self.views.active.get_untracked() == id {
            return;
        }
        self.finish_view_gestures();
        self.views.mounted.update(|mounted| {
            mounted.insert(id);
        });
        self.views.active.set(id);
    }
    pub(crate) fn finish_view_gestures(self) {
        let current = self.for_view(self.views.active.get_untracked());
        if let Some(current) = current {
            current.finish_dock();
            current.analysis().picking_range.set(false);
            current.finish_field_drag();
        }
        self.line_drag.cancel();
        self.finish_geometry_editing();
        self.cancel_geometry_drag();
        self.geometry.library_drag.set(None);
        self.geometry.target.set(None);
        self.measurement.clear();
        self.drag.state.set(None);
        self.cancel_filter_input_drag();
        self.filters_open.set(false);
    }
    pub fn new_view(self) -> u64 {
        let id = self.create_view();
        self.switch_view(id);
        id
    }
    pub(crate) fn create_view(self) -> u64 {
        let view = self.create_detached_view();
        let id = view.id;
        self.views.items.update(|views| views.push(view));
        self.views.mounted.update(|mounted| {
            mounted.insert(id);
        });
        id
    }
    pub(crate) fn create_detached_view(self) -> WorkspaceView {
        let id = self.views.next.get_untracked();
        self.views.next.set(id + 1);
        let plot = self.next_plot.get_untracked();
        self.next_plot.set(plot + 1);
        self.owner
            .with_value(|owner| owner.with(|| WorkspaceView::new(id, plot, format!("View {id}"))))
    }
    pub fn rename_view(self, name: String) {
        let name = name.trim();
        if name.is_empty() || name.len() > 256 {
            self.status
                .set("Use view name between 1 and 256 bytes".into());
            return;
        }
        self.bind_view().view().name.set(name.into());
    }
    pub fn duplicate_view(self) -> u64 {
        let source = self.bind_view().view();
        let id = self.copy_view(source);
        self.for_view(id)
            .unwrap()
            .view()
            .name
            .set(format!("{} copy", source.name.get_untracked()));
        id
    }
    pub(crate) fn copy_view(self, source: WorkspaceView) -> u64 {
        let original = source.panes.get_untracked();
        let layout = source.layout.get_untracked();
        let id = self.new_view();
        let target = self.for_view(id).unwrap();
        target.view().name.set(source.name.get_untracked());
        let mut mapped = std::collections::BTreeMap::new();
        let first = target.pane();
        for (index, pane) in original.iter().enumerate() {
            let copy = if index == 0 {
                first
            } else {
                target.add_plot(first.id, false, false)
            };
            target.copy_plot(*pane, copy);
            mapped.insert(pane.id, copy.id);
        }
        target.layout().set(remap_layout(layout, &mapped));
        if let Some(active) = mapped.get(&source.active.get_untracked()) {
            target.active().set(*active);
        }
        target.maximized().set(
            source
                .maximized
                .get_untracked()
                .and_then(|id| mapped.get(&id).copied()),
        );
        let from = source.analysis;
        let to = target.analysis();
        to.sources.set(from.sources.get_untracked());
        to.disabled.set(from.disabled.get_untracked());
        to.mode.set(from.mode.get_untracked());
        to.window.set(from.window.get_untracked());
        to.size.set(from.size.get_untracked());
        to.overlap.set(from.overlap.get_untracked());
        to.remove_dc.set(from.remove_dc.get_untracked());
        to.heatmap_bins.set(from.heatmap_bins.get_untracked());
        to.condition.set(from.condition.get_untracked());
        to.start.set(from.start.get_untracked());
        to.end.set(from.end.get_untracked());
        to.target.set(
            from.target
                .get_untracked()
                .and_then(|id| mapped.get(&id).copied()),
        );
        to.open.set(from.open.get_untracked());
        to.separate.set(from.separate.get_untracked());
        id
    }
    pub fn close_view(self, id: u64) {
        let Some(workbench) = self.for_view(id) else {
            return;
        };
        if self.views.active.get_untracked() == id {
            self.finish_view_gestures();
        }
        workbench.analysis().reset();
        for pane in workbench.panes().get_untracked() {
            pane.lines.set(Vec::new());
            pane.analysis.set(None);
            pane.snapshot.set_value(None);
            self.profiles.titles.update(|titles| {
                titles.remove(&pane.id);
            });
        }
        workbench.panes().set(Vec::new());
        self.views
            .items
            .update(|views| views.retain(|view| view.id != id));
        self.views.mounted.update(|mounted| {
            mounted.remove(&id);
        });
        if self.views.items.with_untracked(Vec::is_empty) {
            let new = self.new_view();
            self.for_view(new)
                .unwrap()
                .view()
                .name
                .set("Untitled".into());
        } else if self.views.active.get_untracked() == id {
            self.views
                .active
                .set(self.views.items.with_untracked(|views| views[0].id));
        }
    }
}

fn remap_layout(layout: Layout, ids: &std::collections::BTreeMap<u64, u64>) -> Layout {
    match layout {
        Layout::Empty => Layout::Empty,
        Layout::Plot(id) => Layout::Plot(ids[&id]),
        Layout::Tabs { plots, active } => Layout::Tabs {
            plots: plots.iter().map(|id| ids[id]).collect(),
            active: ids[&active],
        },
        Layout::Split {
            id,
            stacked,
            ratio,
            first,
            second,
        } => Layout::Split {
            id,
            stacked,
            ratio,
            first: Box::new(remap_layout(*first, ids)),
            second: Box::new(remap_layout(*second, ids)),
        },
    }
}
