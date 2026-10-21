use crate::profile::identity::{Drag, Item, ProfileKey};
use crate::profile_instance::AppliedProfile;
use crate::{
    derived,
    profile::{self, MetadataMap},
    state::Workbench,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use zgui::prelude::*;

#[derive(Clone, Copy)]
pub enum Destination {
    Plot(Option<u64>),
    Filter,
    Bands,
    Analysis,
    AnalysisCondition,
    SaveSelection,
}

#[derive(Clone)]
pub(crate) struct PendingDrop {
    pub view: u64,
    pub sources: Vec<String>,
    pub target: Destination,
    pub filter_slot: Option<usize>,
}

#[derive(Clone, Copy)]
pub struct ProfileUi {
    pub directory: RwSignal<Option<PathBuf>, LocalStorage>,
    pub groups: RwSignal<BTreeMap<String, String>, LocalStorage>,
    pub open: RwSignal<bool, LocalStorage>,
    pub busy: RwSignal<bool, LocalStorage>,
    pub loading_profile: RwSignal<Option<ProfileKey>, LocalStorage>,
    pub error: RwSignal<String, LocalStorage>,
    pub failures: RwSignal<BTreeMap<ProfileKey, String>, LocalStorage>,
    pub warnings: RwSignal<Vec<String>, LocalStorage>,
    pub metadata: RwSignal<MetadataMap, LocalStorage>,
    pub titles: RwSignal<BTreeMap<u64, String>, LocalStorage>,
    pub instances: RwSignal<BTreeMap<ProfileKey, AppliedProfile>, LocalStorage>,
    pub flight: RwSignal<usize, LocalStorage>,
    pub scan: RwSignal<u64, LocalStorage>,
    pub auto_saving: RwSignal<bool, LocalStorage>,
    pub overrides: RwSignal<BTreeMap<String, Option<profile::Metadata>>, LocalStorage>,
    pub removal: RwSignal<Option<ProfileKey>, LocalStorage>,
    pub rows: StoredValue<BTreeMap<u64, crate::profile_selection::ProfileSourceRow>, LocalStorage>,
    pub(crate) pending_drop: StoredValue<Option<PendingDrop>, LocalStorage>,
    pending: StoredValue<Vec<PathBuf>, LocalStorage>,
    cancel: StoredValue<Option<Arc<AtomicBool>>, LocalStorage>,
}
impl ProfileUi {
    pub fn new() -> Self {
        Self {
            directory: RwSignal::new_local(None),
            groups: RwSignal::new_local(BTreeMap::new()),
            open: RwSignal::new_local(false),
            busy: RwSignal::new_local(false),
            loading_profile: RwSignal::new_local(None),
            error: RwSignal::new_local(String::new()),
            failures: RwSignal::new_local(BTreeMap::new()),
            warnings: RwSignal::new_local(Vec::new()),
            metadata: RwSignal::new_local(BTreeMap::new()),
            titles: RwSignal::new_local(BTreeMap::new()),
            instances: RwSignal::new_local(BTreeMap::new()),
            flight: RwSignal::new_local(1),
            scan: RwSignal::new_local(0),
            auto_saving: RwSignal::new_local(false),
            overrides: RwSignal::new_local(BTreeMap::new()),
            removal: RwSignal::new_local(None),
            rows: StoredValue::new_local(BTreeMap::new()),
            pending_drop: StoredValue::new_local(None),
            pending: StoredValue::new_local(Vec::new()),
            cancel: StoredValue::new_local(None),
        }
    }
    pub fn cancel(self) {
        self.pending.set_value(Vec::new());
        self.pending_drop.set_value(None);
        if let Some(cancel) = self.cancel.get_value() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.busy.set(false);
        self.loading_profile.set(None);
    }
    pub fn reset(self) {
        self.cancel();
        self.error.set(String::new());
        self.failures.set(BTreeMap::new());
        self.warnings.set(Vec::new());
        self.metadata.set(BTreeMap::new());
        self.groups.set(BTreeMap::new());
        self.titles.set(BTreeMap::new());
        self.instances.set(BTreeMap::new());
        self.overrides.set(BTreeMap::new());
        self.removal.set(None);
        self.flight.set(1);
    }
}

impl Workbench {
    pub fn apply_default_profiles(self) {
        if self.profiles.busy.get_untracked() || self.loading.get_untracked() {
            return;
        }
        let Some(log) = self.log.get_untracked() else {
            return;
        };
        let generation = self.generation.get_untracked();
        let flight = self.profiles.flight.get_untracked();
        let cancel = Arc::new(AtomicBool::new(false));
        self.profiles.cancel.set_value(Some(cancel.clone()));
        self.profiles.busy.set(true);
        self.task(async move {
            let result = blocking(move || {
                let choices = crate::profile_preferences::load()?;
                let mut paths = Vec::new();
                let mut warnings = Vec::new();
                for path in profile::discover()? {
                    let profile = match profile::load(&path).map(|p| p.for_flight(flight)) {
                        Ok(profile) => profile,
                        Err(error) => {
                            warnings.push(format!("{}: {error}", path.display()));
                            continue;
                        }
                    };
                    if !choices.get(&path).copied().unwrap_or(profile.auto_apply) {
                        continue;
                    }
                    if profile.requires_settings.is_empty() {
                        warnings.push(format!(
                            "{}: auto_apply needs requires_settings",
                            path.display()
                        ));
                        continue;
                    }
                    if profile::prepare::requirements(&profile, &log).is_ok() {
                        paths.push(path);
                    }
                }
                Ok::<_, String>((paths, warnings))
            })
            .await;
            if self.generation.get_untracked() != generation || cancel.load(Ordering::Relaxed) {
                return;
            }
            self.profiles.busy.set(false);
            match result {
                Ok((mut paths, warnings)) => {
                    if !paths.is_empty() {
                        self.profiles.open.set(true);
                    }
                    self.profiles.warnings.update(|all| all.extend(warnings));
                    paths.reverse();
                    self.profiles.pending.set_value(paths);
                    self.next_default_profile();
                }
                Err(error) => self.profile_failed(error),
            }
        });
    }

    fn next_default_profile(self) {
        self.resume_preset_apply();
        self.resume_profile_drop();
        if self.profiles.busy.get_untracked() {
            return;
        }
        let mut path = None;
        self.profiles
            .pending
            .update_value(|paths| path = paths.pop());
        if let Some(path) = path {
            self.apply_profile(path);
        }
    }

    pub fn apply_profile(self, path: PathBuf) {
        self.load_profile(path, None, false);
    }

    pub fn reload_profile(self, key: ProfileKey) {
        self.profiles.flight.set(key.flight);
        self.load_profile(key.path, None, true);
    }

    pub(crate) fn load_profile(
        self,
        path: PathBuf,
        destination: Option<(Item, Destination)>,
        reload: bool,
    ) {
        if self.profiles.busy.get_untracked()
            || self.derived_busy.get_untracked()
            || self.loading.get_untracked()
        {
            self.profiles
                .error
                .set("Wait for current loading or filters to finish".into());
            return;
        }
        let Some(log) = self.log.get_untracked() else {
            self.profiles.error.set("Open a log first".into());
            return;
        };
        let generation = self.generation.get_untracked();
        let flight = self.profiles.flight.get_untracked();
        let cancel = Arc::new(AtomicBool::new(false));
        self.profiles.cancel.set_value(Some(cancel.clone()));
        self.profiles.busy.set(true);
        self.profiles.error.set(String::new());
        self.profiles
            .loading_profile
            .set(Some(ProfileKey::new(path.clone(), flight)));
        let requested_path = path.clone();
        self.task(async move {
            let loaded = blocking(move || {
                let path = path.canonicalize().map_err(|e| e.to_string())?;
                let profile = profile::load(&path)?.for_flight(flight);
                Ok::<_, String>((path, profile))
            })
            .await;
            if self.generation.get_untracked() != generation || cancel.load(Ordering::Relaxed) {
                return;
            }
            let (path, profile) = match loaded {
                Ok(value) => value,
                Err(error) => {
                    self.profile_failed(error);
                    return;
                }
            };
            let key = ProfileKey::new(path.clone(), flight);
            self.authoring.pending.update_value(|pending| {
                if let Some(request) = pending
                    && request.key.path == requested_path
                    && request.key.flight == flight
                {
                    request.key = key.clone();
                }
            });
            self.workspaces.items.with_untracked(|items| {
                for workspace in items {
                    workspace.source.update(|source| {
                        if let Some(source) = source
                            && source.path == requested_path
                        {
                            source.path = path.clone();
                        }
                    });
                }
            });
            self.profiles.loading_profile.set(Some(key.clone()));
            let previous = self
                .profiles
                .instances
                .with_untracked(|instances| instances.get(&key).cloned());
            if previous.is_some() && !reload {
                self.profiles.busy.set(false);
                self.profiles.loading_profile.set(None);
                if let Some((item, target)) = destination {
                    self.place_prepared_item(key, item, target);
                } else {
                    self.status
                        .set("Profile ready; drag outputs onto a plot".into());
                }
                self.next_default_profile();
                return;
            }
            let mut ids = previous
                .as_ref()
                .map(|old| old.ids.clone())
                .unwrap_or_default();
            ids.fields
                .retain(|id, _| profile.filters.iter().any(|field| &field.id == id));
            ids.geometry
                .retain(|id, _| profile.geometry.iter().any(|geometry| &geometry.id == id));
            for field in &profile.filters {
                ids.fields
                    .entry(field.id.clone())
                    .or_insert_with(|| self.id());
            }
            for geometry in &profile.geometry {
                ids.geometry
                    .entry(geometry.id.clone())
                    .or_insert_with(|| self.id());
            }
            let definition = profile.clone();
            let base = path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            let cancelled = cancel.clone();
            let prepare_log = log.clone();
            let result = blocking(move || {
                profile::prepare::prepare_with_ids(profile, &prepare_log, &base, ids, cancelled)
            })
            .await;
            if self.generation.get_untracked() != generation || cancel.load(Ordering::Relaxed) {
                return;
            }
            let mut prepared = match result {
                Ok(prepared) => prepared,
                Err(error) => {
                    self.profile_failed(error);
                    return;
                }
            };
            if log.settings.len() > 1 {
                for field in &mut prepared.fields {
                    field.name = format!("{} · flight {flight}", field.name);
                    if let Some(metadata) = prepared.metadata.get_mut(&field.key()) {
                        metadata.name = format!("{} · flight {flight}", metadata.name);
                    }
                }
                for geometry in &mut prepared.geometry {
                    geometry.name = format!("{} · flight {flight}", geometry.name);
                }
            }
            if definition.version >= 2 {
                self.reuse_preset_resources(&mut prepared, previous.as_ref());
            }
            if self
                .authoring
                .saved
                .with_value(|saved| saved.as_ref().is_some_and(|saved| saved.path == key.path))
                && let Some(previous) = &previous
            {
                self.preserve_deleted_profile_fields(&mut prepared, previous);
            }
            let instance = AppliedProfile::from_prepared(key.clone(), definition, prepared);
            let baseline = self.derived.get_untracked();
            let mut combined = baseline.clone();
            let mut geometry_inputs = Vec::new();
            if let Some(old) = &previous {
                // Previously working branches must remain working on reload. Intentional deletions are allowed.
                for field in &old.fields {
                    if baseline.iter().any(|active| active.id == field.id)
                        && instance.ids.fields.values().any(|id| *id == field.id)
                        && instance
                            .definition
                            .filters
                            .iter()
                            .any(|f| instance.ids.fields.get(&f.id) == Some(&field.id))
                        && !instance.fields.iter().any(|f| f.id == field.id)
                    {
                        self.profile_failed(format!(
                            "Reload kept previous setup: {} failed",
                            field.name
                        ));
                        return;
                    }
                }
                for geometry in &old.geometry {
                    if instance
                        .definition
                        .geometry
                        .iter()
                        .any(|g| instance.ids.geometry.get(&g.id) == Some(&geometry.id))
                        && !instance.geometry.iter().any(|g| g.id == geometry.id)
                    {
                        self.profile_failed(format!(
                            "Reload kept previous setup: {} failed",
                            geometry.name
                        ));
                        return;
                    }
                }
                let changed = old.field_ids();
                combined.retain(|field| !changed.contains(&field.id));
                let mut dirty = changed;
                loop {
                    let before = dirty.len();
                    for field in &mut combined {
                        if field
                            .sources
                            .iter()
                            .filter_map(|s| derived::id(s))
                            .any(|id| dirty.contains(&id))
                        {
                            dirty.insert(field.id);
                            field.data = None;
                        }
                    }
                    if before == dirty.len() {
                        break;
                    }
                }
                let owned = old.geometry_ids();
                geometry_inputs = self.geometry.items.with_untracked(|items| {
                    items
                        .iter()
                        .filter(|g| {
                            !owned.contains(&g.id)
                                && g.states.as_ref().is_some_and(|state| {
                                    derived::id(&state.source).is_some_and(|id| dirty.contains(&id))
                                })
                        })
                        .cloned()
                        .collect()
                });
            }
            combined.extend(instance.fields.clone());
            let cancelled = cancel.clone();
            let computed = blocking(move || {
                let fields =
                    derived::compute_cancelled(combined, &log, BTreeMap::new(), cancelled)?;
                let mut geometry_updates = Vec::new();
                for original in geometry_inputs {
                    let mut updated = original.clone();
                    let old = original.states.as_ref().unwrap();
                    let data = fields
                        .iter()
                        .find(|f| f.key() == old.source)
                        .and_then(|f| f.data.clone())
                        .ok_or_else(|| {
                            format!(
                                "Geometry {} depends on removed field {}",
                                original.name, old.source
                            )
                        })?;
                    let end = if log.blackbox_headers.is_empty() {
                        log.duration
                    } else {
                        data.samples().last().map_or(log.duration, |point| point.x)
                    };
                    let mut states =
                        crate::state_bands::StateBands::build(old.source.clone(), data, end)?;
                    states.preserve_rules(old);
                    updated.states = Some(Arc::new(states));
                    geometry_updates.push((original, updated));
                }
                Ok::<_, String>((fields, geometry_updates))
            })
            .await;
            if self.generation.get_untracked() != generation || cancel.load(Ordering::Relaxed) {
                return;
            }
            let computed = match computed {
                Ok(fields) => fields,
                Err(error) => {
                    self.profile_failed(format!("Reload kept previous setup: {error}"));
                    return;
                }
            };
            let unchanged = self.derived.with_untracked(|current| {
                current.len() == baseline.len()
                    && current.iter().zip(&baseline).all(|(a, b)| {
                        a.id == b.id
                            && a.name == b.name
                            && a.sources == b.sources
                            && a.group == b.group
                            && a.filter == b.filter
                            && match (&a.data, &b.data) {
                                (Some(a), Some(b)) => std::ptr::eq(a.samples(), b.samples()),
                                (None, None) => true,
                                _ => false,
                            }
                    })
            });
            if self.derived_busy.get_untracked() || !unchanged {
                self.profile_failed(
                    "Filters changed during preparation; previous setup kept. Retry reload.".into(),
                );
                return;
            }
            match self.commit_instance(instance, computed.0, computed.1) {
                Ok(name) => {
                    self.profiles.busy.set(false);
                    self.profiles.loading_profile.set(None);
                    self.profiles.failures.update(|failures| {
                        failures.remove(&key);
                    });
                    self.status.set(format!("Applied profile: {name}"));
                    self.refresh_saved_capture(&key);
                    if let Some((item, target)) = destination {
                        self.place_prepared_item(key, item, target);
                    }
                    self.next_default_profile();
                }
                Err(error) => self.profile_failed(error),
            }
        });
    }

    fn profile_failed(self, error: String) {
        self.authoring.saved.set_value(None);
        if let Some(key) = self.profiles.loading_profile.get_untracked() {
            self.authoring.pending.update_value(|pending| {
                if pending.as_ref().is_some_and(|pending| pending.key == key) {
                    *pending = None;
                }
            });
            self.profiles.pending_drop.update_value(|pending| {
                if let Some(pending) = pending {
                    pending.sources.retain(|source| {
                        Drag::parse(source).is_none_or(|drag| drag.profile != key)
                    });
                }
            });
            self.profiles.failures.update(|failures| {
                failures.insert(key, error.clone());
            });
        }
        self.profiles.busy.set(false);
        self.profiles.loading_profile.set(None);
        self.profiles
            .warnings
            .update(|warnings| warnings.push(error.clone()));
        self.profiles.error.set(error.clone());
        self.status.set(format!("Profile: {error}"));
        self.next_default_profile();
    }
}

#[cfg(test)]
#[path = "profile_state_tests.rs"]
mod tests;
