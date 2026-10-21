//! Refresh recipe attachments by identity, preserving geometry from other placements.
use crate::{profile::identity::Item, profile_instance::AppliedProfile, state::Workbench};
use std::collections::BTreeSet;
use zgui::prelude::*;

fn attached(instance: &AppliedProfile, pane: u64) -> BTreeSet<u64> {
    instance
        .placements
        .iter()
        .filter(|placement| placement.pane == pane)
        .flat_map(|placement| match &placement.item {
            Item::View(name) => instance
                .views
                .get(name)
                .map(|view| view.geometry.clone())
                .unwrap_or_default(),
            Item::Geometry(name) => instance
                .ids
                .geometry
                .get(name.trim_start_matches('$'))
                .copied()
                .into_iter()
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

impl Workbench {
    pub(crate) fn refresh_profile_geometry(self, old: &AppliedProfile, new: &AppliedProfile) {
        for pane in self.all_panes() {
            let removed = attached(old, pane.id);
            let retained = attached(new, pane.id);
            if removed.is_empty() && retained.is_empty() {
                continue;
            }
            let foreign = self.profiles.instances.with_untracked(|instances| {
                instances
                    .values()
                    .filter(|other| other.key != old.key)
                    .flat_map(|other| attached(other, pane.id))
                    .collect::<BTreeSet<_>>()
            });
            pane.geometry.update(|ids| {
                ids.retain(|id| {
                    !removed.contains(id) || retained.contains(id) || foreign.contains(id)
                });
                ids.extend(retained);
            });
        }
    }
}
