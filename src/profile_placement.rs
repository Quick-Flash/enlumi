use crate::{
    derived,
    profile::{
        self,
        identity::{Item, ProfileKey},
        prepare::PreparedView,
    },
    profile_state::Destination,
    state::{Line, Workbench},
};
use std::collections::{BTreeMap, BTreeSet};
use zgui::prelude::*;
impl Workbench {
    pub fn use_profile_item(self, drag: profile::identity::Drag, target: Destination) {
        if self.profiles.busy.get_untracked()
            || self.loading.get_untracked()
            || self.derived_busy.get_untracked()
        {
            self.profiles
                .error
                .set("Wait for loading and filters to finish".into());
            return;
        }
        self.profiles.flight.set(drag.profile.flight);
        if self
            .profiles
            .instances
            .with_untracked(|all| all.contains_key(&drag.profile))
        {
            self.place_prepared_item(drag.profile, drag.item, target);
            return;
        }
        self.load_profile(drag.profile.path, Some((drag.item, target)), false);
    }
    pub(crate) fn place_prepared_item(self, key: ProfileKey, item: Item, target: Destination) {
        let Some(instance) = self
            .profiles
            .instances
            .with_untracked(|all| all.get(&key).cloned())
        else {
            return;
        };
        let pane = match target {
            Destination::Plot(pane) => pane,
            Destination::Filter | Destination::Bands => {
                let Item::Field(name) = &item else {
                    self.profiles
                        .error
                        .set("Choose individual field for filters or state bands".into());
                    return;
                };
                let Some(source) = instance.resolve(name) else {
                    self.profiles
                        .error
                        .set("Profile field unavailable; see skipped outputs".into());
                    return;
                };
                if matches!(target, Destination::Filter) {
                    self.filters_open.set(true);
                    if let Err(error) = self.accept_filter_sources(vec![source]) {
                        self.filter_error.set(error);
                    }
                } else {
                    self.generate_bands(source, None);
                }
                return;
            }
        };
        let placed_item = item.clone();
        let view = match item {
            Item::View(name) => instance.views.get(&name).cloned(),
            Item::Field(name) => instance.resolve(&name).and_then(|source| {
                let data = if derived::id(&source).is_some() {
                    self.derived.with_untracked(|fields| {
                        fields
                            .iter()
                            .find(|f| f.key() == source)
                            .and_then(|f| f.data.clone())
                    })
                } else {
                    self.log.with_untracked(|log| {
                        log.as_ref().and_then(|log| log.prepare(&source).ok())
                    })
                }?;
                Some(PreparedView {
                    name: self.field_name(&source),
                    bindings: BTreeMap::from([(name, source.clone())]),
                    fields: vec![(source, data)],
                    geometry: vec![],
                    tab: false,
                    stacked: true,
                })
            }),
            Item::Geometry(name) => instance
                .ids
                .geometry
                .get(&name)
                .copied()
                .filter(|id| self.geometry_item(*id).is_some())
                .map(|id| PreparedView {
                    name: name.clone(),
                    bindings: BTreeMap::new(),
                    fields: vec![],
                    geometry: vec![id],
                    tab: false,
                    stacked: true,
                }),
        };
        let Some(view) = view else {
            self.profiles
                .error
                .set("Resource has no usable outputs; expand skipped items for reason".into());
            return;
        };
        let previous_lines = self
            .panes
            .get_untracked()
            .iter()
            .flat_map(|pane| {
                pane.lines
                    .with_untracked(|lines| lines.iter().map(|l| l.id).collect::<Vec<_>>())
            })
            .collect::<BTreeSet<_>>();
        let bindings = view.bindings.clone();
        let result = if let Some(pane) = pane {
            if self.find(pane).is_none() {
                Err("Destination plot was closed".into())
            } else {
                let unnamed_empty = self
                    .find(pane)
                    .is_some_and(|pane| pane.lines.with_untracked(Vec::is_empty));
                let title = view.name.clone();
                self.activate(pane);
                let result = self.place_profile_view(view, false);
                if result.is_ok() && unnamed_empty {
                    self.profiles.titles.update(|titles| {
                        titles.entry(pane).or_insert(title);
                    });
                }
                result
            }
        } else {
            self.place_profile_view(view, true)
        };
        if let Err(error) = result {
            self.profiles.error.set(error);
            return;
        }
        let target = self.pane();
        let lines = target.lines.with_untracked(|lines| {
            lines
                .iter()
                .filter(|l| !previous_lines.contains(&l.id))
                .map(|l| {
                    (
                        bindings
                            .iter()
                            .find(|(_, source)| **source == l.source)
                            .map(|(token, _)| token.clone())
                            .unwrap_or_else(|| l.source.clone()),
                        l.id,
                    )
                })
                .collect()
        });
        self.profiles.instances.update(|all| {
            if let Some(instance) = all.get_mut(&key) {
                instance
                    .placements
                    .push(crate::profile_instance::Placement {
                        pane: target.id,
                        item: placed_item,
                        lines,
                    });
            }
        });
    }

    pub(crate) fn place_profile_view(
        self,
        view: PreparedView,
        new_panel: bool,
    ) -> Result<(), String> {
        if self.loading.get_untracked()
            || self.derived_busy.get_untracked()
            || self.profiles.busy.get_untracked()
        {
            return Err("Wait for loading and filters to finish".into());
        }
        for id in &view.geometry {
            if self.geometry_item(*id).is_none() {
                return Err("View geometry was deleted; reload log and profile".into());
            }
        }
        let mut inputs = Vec::new();
        for (source, original) in view.fields {
            let data = if derived::id(&source).is_some() {
                self.derived
                    .with_untracked(|fields| {
                        fields
                            .iter()
                            .find(|field| field.key() == source)
                            .and_then(|field| field.data.clone())
                    })
                    .ok_or_else(|| format!("View field unavailable: {source}"))?
            } else {
                original
            };
            inputs.push((source, data));
        }
        let pane = if new_panel {
            let pane = self.add_plot(self.active.get_untracked(), view.stacked, view.tab);
            self.profiles.titles.update(|titles| {
                titles.insert(pane.id, view.name.clone());
            });
            pane
        } else {
            self.pane()
        };
        let geometry_ids = view.geometry.clone();
        pane.geometry
            .update(|attached| attached.extend(view.geometry));
        for (source, data) in inputs {
            if pane
                .lines
                .with_untracked(|lines| lines.iter().any(|line| line.source == source))
            {
                continue;
            }
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
        }
        pane.fit_axes(plot::interaction::Axes::BOTH);
        if pane.lines.with_untracked(Vec::is_empty) {
            let mut x = (
                0.0_f64,
                self.log
                    .with_untracked(|log| log.as_ref().map_or(1.0, |log| log.duration.max(1.0))),
            );
            let mut y = (0.0_f64, 1.0_f64);
            for id in geometry_ids {
                if let Some(item) = self.geometry_item(id) {
                    x.0 = x.0.min(item.coordinates[0]).min(item.coordinates[2]);
                    x.1 = x.1.max(item.coordinates[0]).max(item.coordinates[2]);
                    y.0 = y.0.min(item.coordinates[1]).min(item.coordinates[3]);
                    y.1 = y.1.max(item.coordinates[1]).max(item.coordinates[3]);
                }
            }
            let _ = pane.plot.set_view(plot::Ranges::new(x, y));
        }
        self.filters_open.set(false);
        self.status.set(format!("Added view: {}", view.name));
        self.profiles.error.set(String::new());
        Ok(())
    }
}
