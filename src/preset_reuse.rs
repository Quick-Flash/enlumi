//! Borrow identical live resources when opening recipes saved from this workspace.
use crate::{profile::prepare::Prepared, profile_instance::AppliedProfile, state::Workbench};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use zgui::prelude::*;

impl Workbench {
    pub(crate) fn preserve_deleted_profile_fields(
        self,
        prepared: &mut Prepared,
        previous: &AppliedProfile,
    ) {
        let live = self
            .derived
            .with_untracked(|fields| fields.iter().map(|field| field.id).collect::<BTreeSet<_>>());
        let mut deleted = previous
            .ids
            .fields
            .values()
            .filter(|id| !live.contains(*id))
            .copied()
            .collect::<BTreeSet<_>>();
        loop {
            let before = deleted.len();
            prepared.fields.retain(|field| {
                if deleted.contains(&field.id)
                    || field
                        .sources
                        .iter()
                        .filter_map(|source| crate::derived::id(source))
                        .any(|id| deleted.contains(&id))
                {
                    deleted.insert(field.id);
                    false
                } else {
                    true
                }
            });
            if deleted.len() == before {
                break;
            }
        }
        let available =
            |source: &str| crate::derived::id(source).is_none_or(|id| !deleted.contains(&id));
        prepared.metadata.retain(|source, _| available(source));
        prepared.symbols.retain(|_, source| available(source));
        for view in &mut prepared.views {
            view.fields.retain(|(source, _)| available(source));
            view.bindings.retain(|_, source| available(source));
        }
        // Keep ID reservations so a later automatic refresh recognizes the same deletions.
    }
    pub(crate) fn reuse_preset_resources(
        self,
        prepared: &mut Prepared,
        previous: Option<&AppliedProfile>,
    ) {
        let owned_fields = previous.map(AppliedProfile::field_ids).unwrap_or_default();
        let owned_geometry = previous
            .map(AppliedProfile::geometry_ids)
            .unwrap_or_default();
        let existing = self.derived.get_untracked();
        let mut sources = BTreeMap::new();
        prepared.fields.retain_mut(|field| {
            for source in &mut field.sources {
                remap(source, &sources);
            }
            let original = field.key();
            if let Some(shared) = existing.iter().find(|shared| {
                !owned_fields.contains(&shared.id)
                    && shared.name == field.name
                    && shared.filter == field.filter
                    && shared.sources == field.sources
                    && shared.data.is_some()
            }) {
                sources.insert(original, shared.key());
                return false;
            }
            if existing
                .iter()
                .any(|shared| shared.id == field.id && !owned_fields.contains(&shared.id))
            {
                field.id = self.id();
                sources.insert(original, field.key());
            }
            true
        });
        for id in prepared.ids.fields.values_mut() {
            if let Some(source) = sources.get(&crate::derived::key(*id)) {
                *id = crate::derived::id(source).unwrap();
            }
        }
        prepared.metadata = std::mem::take(&mut prepared.metadata)
            .into_iter()
            .map(|(mut source, metadata)| {
                remap(&mut source, &sources);
                (source, metadata)
            })
            .collect();
        for source in prepared.symbols.values_mut() {
            remap(source, &sources);
        }
        let mut geometry = BTreeMap::new();
        let existing = self.geometry.items.get_untracked();
        prepared.geometry.retain_mut(|item| {
            if let Some(states) = &mut item.states {
                remap(&mut Arc::make_mut(states).source, &sources);
            }
            let shared = existing.iter().find(|shared| {
                !owned_geometry.contains(&shared.id) && shared.same_definition(item)
            });
            if let Some(shared) = shared {
                geometry.insert(item.id, shared.id);
                return false;
            }
            if existing
                .iter()
                .any(|shared| shared.id == item.id && !owned_geometry.contains(&shared.id))
            {
                let original = item.id;
                item.id = self.id();
                geometry.insert(original, item.id);
            }
            true
        });
        for id in prepared.ids.geometry.values_mut() {
            if let Some(shared) = geometry.get(id) {
                *id = *shared;
            }
        }
        for view in &mut prepared.views {
            for (source, _) in &mut view.fields {
                remap(source, &sources);
            }
            for source in view.bindings.values_mut() {
                remap(source, &sources);
            }
            for id in &mut view.geometry {
                if let Some(shared) = geometry.get(id) {
                    *id = *shared;
                }
            }
        }
    }
}
fn remap(source: &mut String, sources: &BTreeMap<String, String>) {
    if let Some(replacement) = sources.get(source) {
        *source = replacement.clone();
    }
}
