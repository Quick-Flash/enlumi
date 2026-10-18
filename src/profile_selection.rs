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
}

impl Workbench {
    pub fn visible_profile_sources(self) -> Vec<String> {
        let mut rows = self.profiles.rows.with_value(|rows| {
            rows.values()
                .filter_map(|row| {
                    let bounds = row.node.window_bounds()?;
                    (bounds.size.width.0 > 0.0 && bounds.size.height.0 > 0.0).then(|| {
                        row.source
                            .get_untracked()
                            .map(|source| (bounds.origin.y.0, bounds.origin.x.0, source))
                    })?
                })
                .collect::<Vec<_>>()
        });
        rows.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });
        rows.into_iter().map(|(_, _, source)| source).collect()
    }

    pub fn use_profile_items(self, items: Vec<Drag>, target: Destination) {
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
        if matches!(target, Destination::Filter | Destination::Bands)
            && items
                .iter()
                .any(|drag| !matches!(drag.item, Item::Field(_)))
        {
            self.profiles
                .error
                .set("Choose fields for filters or state bands".into());
            return;
        }
        if matches!(target, Destination::Bands) && items.len() != 1 {
            self.bands
                .error
                .set("Drop one discrete source field".into());
            return;
        }
        self.profiles.pending_drop.set_value(Some((items, target)));
        self.resume_profile_drop();
    }

    // Prepare missing profiles sequentially, then place complete selection.
    pub(crate) fn resume_profile_drop(self) {
        let Some((items, target)) = self.profiles.pending_drop.get_value() else {
            return;
        };
        if self.profiles.busy.get_untracked() {
            return;
        }
        let missing = self.profiles.instances.with_untracked(|instances| {
            items
                .iter()
                .find(|drag| !instances.contains_key(&drag.profile))
                .cloned()
        });
        if let Some(drag) = missing {
            self.profiles.flight.set(drag.profile.flight);
            self.load_profile(drag.profile.path, None, false);
            return;
        }
        self.profiles.pending_drop.set_value(None);
        match target {
            Destination::Analysis => {
                let sources: Vec<String> = self.profiles.instances.with_untracked(|instances| {
                    items
                        .iter()
                        .flat_map(|drag| {
                            let Some(instance) = instances.get(&drag.profile) else {
                                return Vec::new();
                            };
                            match &drag.item {
                                Item::Field(name) => instance.resolve(name).into_iter().collect(),
                                Item::View(name) => {
                                    instance.views.get(name).map_or_else(Vec::new, |view| {
                                        view.fields
                                            .iter()
                                            .map(|(source, _)| source.clone())
                                            .collect()
                                    })
                                }
                                Item::Geometry(_) => Vec::new(),
                            }
                        })
                        .collect()
                });
                if sources.is_empty() {
                    self.analysis.message.set("No sample fields available. Drop fields or a usable profile view; geometry has no samples.".into());
                } else {
                    self.add_analysis_sources(sources);
                }
            }
            Destination::Plot(_) => {
                for drag in items {
                    self.place_prepared_item(drag.profile, drag.item, target);
                }
            }
            Destination::Filter | Destination::Bands => {
                let sources = self.profiles.instances.with_untracked(|instances| {
                    items
                        .iter()
                        .filter_map(|drag| {
                            let Item::Field(name) = &drag.item else {
                                return None;
                            };
                            instances.get(&drag.profile)?.resolve(name)
                        })
                        .collect::<Vec<_>>()
                });
                if sources.len() != items.len() {
                    self.profiles
                        .error
                        .set("Some selected fields unavailable; see skipped outputs".into());
                }
                if sources.is_empty() {
                    return;
                }
                if matches!(target, Destination::Filter) {
                    self.filters_open.set(true);
                    if let Err(error) = self.accept_filter_sources(sources) {
                        self.filter_error.set(error);
                    }
                } else {
                    self.generate_bands(sources[0].clone(), None);
                }
            }
        }
    }
}
