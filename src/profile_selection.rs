use crate::{
    profile::identity::{Drag, Item},
    profile_state::Destination,
    state::Workbench,
};
use zgui::prelude::*;

#[derive(Clone, Copy)]
pub struct ProfileSourceRow {
    pub source: Signal<Option<String>, LocalStorage>,
    pub node: NodeRef,
    pub range: Option<Signal<Vec<String>, LocalStorage>>,
}

impl Workbench {
    pub fn visible_profile_sources(self) -> Vec<String> {
        let mut rows = self.profiles.rows.with_value(|rows| {
            rows.values()
                .filter_map(|row| {
                    let bounds = row.node.window_bounds()?;
                    (bounds.size.width.0 > 0.0 && bounds.size.height.0 > 0.0).then(|| {
                        let sources = row.range.map_or_else(
                            || row.source.get_untracked().into_iter().collect(),
                            |range| range.get_untracked(),
                        );
                        (!sources.is_empty()).then_some((
                            bounds.origin.y.0,
                            bounds.origin.x.0,
                            sources,
                        ))
                    })?
                })
                .collect::<Vec<_>>()
        });
        rows.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });
        rows.into_iter()
            .flat_map(|(_, _, sources)| sources)
            .collect()
    }

    pub fn sidebar_sources(self) -> Vec<String> {
        let mut sources = self.browser_fields.get_untracked();
        sources.extend(self.visible_profile_sources());
        sources
    }

    pub fn use_profile_items(self, items: Vec<Drag>, target: Destination) {
        self.queue_field_sources(
            items.into_iter().map(|drag| drag.source()).collect(),
            target,
            None,
        );
    }

    pub(crate) fn queue_field_sources(
        self,
        sources: Vec<String>,
        target: Destination,
        filter_slot: Option<usize>,
    ) {
        self.queue_field_sources_at_axis(sources, target, filter_slot, None);
    }
    pub(crate) fn queue_field_sources_at_axis(
        self,
        sources: Vec<String>,
        target: Destination,
        filter_slot: Option<usize>,
        trajectory_axis: Option<(usize, bool)>,
    ) {
        if self.profiles.busy.get_untracked()
            || self.derived_busy.get_untracked()
            || self.loading.get_untracked()
        {
            self.profiles
                .error
                .set("Wait for loading and filters to finish".into());
            return;
        }
        if self.log.with_untracked(Option::is_none) {
            self.profiles.error.set("Open a log first".into());
            return;
        }
        if matches!(
            target,
            Destination::Filter | Destination::Bands | Destination::AnalysisCondition
        ) && sources
            .iter()
            .filter_map(|source| Drag::parse(source))
            .any(|drag| !matches!(drag.item, Item::Field(_)))
        {
            self.profiles
                .error
                .set("Choose fields for filters or state bands".into());
            return;
        }
        if matches!(target, Destination::Bands | Destination::AnalysisCondition)
            && sources.len() != 1
        {
            if matches!(target, Destination::Bands) {
                self.bands
                    .error
                    .set("Drop one discrete source field".into());
            } else {
                self.analysis()
                    .message
                    .set("Drop one conditioning field".into());
            }
            return;
        }
        self.profiles
            .pending_drop
            .set_value(Some(crate::profile_state::PendingDrop {
                view: self.bind_view().view().id,
                sources,
                target,
                filter_slot,
                trajectory_axis,
            }));
        self.resume_profile_drop();
    }

    // Prepare missing profiles sequentially, then place entire mixed selection.
    pub(crate) fn resume_profile_drop(self) {
        let Some(pending) = self.profiles.pending_drop.get_value() else {
            return;
        };
        if self.profiles.busy.get_untracked() {
            return;
        }
        let Some(workbench) = self.for_view(pending.view) else {
            self.profiles.pending_drop.set_value(None);
            return;
        };
        workbench.place_field_sources(pending);
    }
    fn place_field_sources(self, pending: crate::profile_state::PendingDrop) {
        let missing = self.profiles.instances.with_untracked(|instances| {
            pending
                .sources
                .iter()
                .filter_map(|source| Drag::parse(source))
                .find(|drag| !instances.contains_key(&drag.profile))
        });
        if let Some(drag) = missing {
            self.profiles.flight.set(drag.profile.flight);
            self.load_profile(drag.profile.path, None, false);
            return;
        }
        self.profiles.pending_drop.set_value(None);
        let sources = self.resolve_field_sources(&pending.sources);
        match pending.target {
            Destination::TrajectoryAxis(index, y) => {
                if let Err(error) = crate::trajectory_axes::assign(
                    self.trajectory_builder().pane,
                    index,
                    y,
                    &sources,
                ) {
                    self.status.set(error);
                }
            }
            Destination::SaveSelection => {
                self.begin_save(crate::preset_capture::CaptureKind::Selection(sources))
            }
            Destination::Analysis | Destination::AnalysisCondition => {
                if sources.is_empty() {
                    self.analysis().run_selection.set(false);
                    self.analysis().message.set("No sample fields available. Drop fields or a usable profile view; geometry has no samples.".into());
                } else if matches!(pending.target, Destination::AnalysisCondition) {
                    self.set_analysis_condition(sources);
                } else {
                    self.add_analysis_sources(sources);
                    if self.analysis().run_selection.get_untracked() {
                        self.analysis().run_selection.set(false);
                        self.run_analysis();
                    }
                }
            }
            Destination::Plot(pane) => {
                let mut destination = pending.target;
                if let Some(id) = pane {
                    let Some(target) = self.find(id) else {
                        self.status.set("Destination plot was closed".into());
                        return;
                    };
                    let id = if pending.trajectory_axis.is_none()
                        && target.trajectory.with_untracked(Option::is_some)
                        && !sources.is_empty()
                        && pending.sources.iter().all(|source| {
                            Drag::parse(source)
                                .is_none_or(|drag| matches!(drag.item, Item::Field(_)))
                        }) {
                        self.add_plot(id, false, false).id
                    } else {
                        id
                    };
                    self.activate(id);
                    destination = Destination::Plot(Some(id));
                }
                if let Some((index, y)) = pending.trajectory_axis {
                    let pane = self.pane();
                    if let Err(error) = crate::trajectory_axes::assign(pane, index, y, &sources) {
                        self.status.set(error);
                        return;
                    }
                }
                for source in pending.sources {
                    if let Some(drag) = Drag::parse(&source) {
                        self.place_prepared_item(drag.profile, drag.item, destination);
                    } else {
                        self.add(source);
                    }
                }
            }
            Destination::Filter | Destination::Bands => {
                if sources.is_empty() {
                    return;
                }
                if matches!(pending.target, Destination::Filter) {
                    if self.is_current_view() {
                        self.filters_open.set(true);
                    }
                    if let Err(error) = self.accept_filter_sources_at(sources, pending.filter_slot)
                    {
                        self.filter_error.set(error);
                    }
                } else {
                    self.generate_bands(sources[0].clone(), None);
                }
            }
        }
    }

    fn resolve_field_sources(self, selected: &[String]) -> Vec<String> {
        let mut sources = Vec::new();
        self.profiles.instances.with_untracked(|instances| {
            for source in selected {
                let resolved = if let Some(drag) = Drag::parse(source) {
                    instances
                        .get(&drag.profile)
                        .map_or_else(Vec::new, |instance| match &drag.item {
                            Item::Field(name) => instance.resolve(name).into_iter().collect(),
                            Item::View(name) => {
                                instance.views.get(name).map_or_else(Vec::new, |view| {
                                    view.fields
                                        .iter()
                                        .map(|(source, _)| source.clone())
                                        .collect()
                                })
                            }
                            Item::Workspace(name) => instance
                                .definition
                                .view_presets
                                .iter()
                                .find(|view| &view.name == name)
                                .map_or_else(Vec::new, |view| {
                                    view.plots
                                        .iter()
                                        .flat_map(|plot| plot.lines.iter())
                                        .filter_map(|line| instance.resolve(&line.source))
                                        .collect()
                                }),
                            Item::WorkspacePreset(name) => instance
                                .definition
                                .workspace_presets
                                .iter()
                                .find(|workspace| &workspace.name == name)
                                .map_or_else(Vec::new, |workspace| {
                                    workspace
                                        .views
                                        .iter()
                                        .flat_map(|view| view.plots.iter())
                                        .flat_map(|plot| plot.lines.iter())
                                        .filter_map(|line| instance.resolve(&line.source))
                                        .collect()
                                }),
                            Item::Geometry(_) => Vec::new(),
                        })
                } else {
                    vec![source.clone()]
                };
                for source in resolved {
                    if !sources.contains(&source) {
                        sources.push(source);
                    }
                }
            }
        });
        sources
    }
}
