use crate::{
    derived,
    profile::{
        self, MetadataMap,
        prepare::{Prepared, PreparedView},
    },
    state::{Line, Workbench},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use zgui::prelude::*;

#[derive(Clone, Copy)]
pub struct ProfileUi {
    pub directory: RwSignal<Option<PathBuf>, LocalStorage>,
    pub groups: RwSignal<BTreeMap<String, String>, LocalStorage>,
    pub browsing: RwSignal<bool, LocalStorage>,
    pub busy: RwSignal<bool, LocalStorage>,
    pub error: RwSignal<String, LocalStorage>,
    pub warnings: RwSignal<Vec<String>, LocalStorage>,
    pub metadata: RwSignal<MetadataMap, LocalStorage>,
    pub titles: RwSignal<BTreeMap<u64, String>, LocalStorage>,
    pub views: RwSignal<Vec<PreparedView>, LocalStorage>,
    pub applied: RwSignal<BTreeSet<PathBuf>, LocalStorage>,
    loaded_views: StoredValue<BTreeMap<PathBuf, Vec<(String, usize)>>, LocalStorage>,
    pending: StoredValue<Vec<PathBuf>, LocalStorage>,
    cancel: StoredValue<Option<Arc<AtomicBool>>, LocalStorage>,
}
impl ProfileUi {
    pub fn new() -> Self {
        Self {
            directory: RwSignal::new_local(None),
            groups: RwSignal::new_local(BTreeMap::new()),
            browsing: RwSignal::new_local(false),
            busy: RwSignal::new_local(false),
            error: RwSignal::new_local(String::new()),
            warnings: RwSignal::new_local(Vec::new()),
            metadata: RwSignal::new_local(BTreeMap::new()),
            titles: RwSignal::new_local(BTreeMap::new()),
            views: RwSignal::new_local(Vec::new()),
            applied: RwSignal::new_local(BTreeSet::new()),
            loaded_views: StoredValue::new_local(BTreeMap::new()),
            pending: StoredValue::new_local(Vec::new()),
            cancel: StoredValue::new_local(None),
        }
    }
    pub fn cancel(self) {
        self.pending.set_value(Vec::new());
        if let Some(cancel) = self.cancel.get_value() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.busy.set(false);
    }
    pub fn reset(self) {
        self.cancel();
        self.error.set(String::new());
        self.warnings.set(Vec::new());
        self.metadata.set(BTreeMap::new());
        self.groups.set(BTreeMap::new());
        self.titles.set(BTreeMap::new());
        self.applied.set(BTreeSet::new());
        self.views.set(Vec::new());
        self.loaded_views.set_value(BTreeMap::new());
    }
}

impl Workbench {
    pub fn apply_default_profiles(self) {
        let Some(log) = self.log.get_untracked() else {
            return;
        };
        let generation = self.generation.get_untracked();
        let cancel = Arc::new(AtomicBool::new(false));
        self.profiles.cancel.set_value(Some(cancel.clone()));
        self.profiles.busy.set(true);
        self.task(async move {
            let result = blocking(move || {
                let mut paths = Vec::new();
                let mut warnings = Vec::new();
                for path in profile::discover()? {
                    let profile = match profile::load(&path) {
                        Ok(profile) => profile,
                        Err(error) => {
                            warnings.push(format!("{}: {error}", path.display()));
                            continue;
                        }
                    };
                    if !profile.auto_apply {
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
                        self.profiles.browsing.set(true);
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
        let mut path = None;
        self.profiles
            .pending
            .update_value(|paths| path = paths.pop());
        if let Some(path) = path {
            self.apply_profile(path);
        }
    }

    pub fn apply_profile(self, path: PathBuf) {
        self.load_profile(path, None);
    }

    pub fn apply_profile_view(self, path: PathBuf, name: String, pane: u64) {
        self.load_profile(path, Some((name, pane)));
    }

    fn load_profile(self, path: PathBuf, destination: Option<(String, u64)>) {
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
        let cancel = Arc::new(AtomicBool::new(false));
        self.profiles.cancel.set_value(Some(cancel.clone()));
        self.profiles.busy.set(true);
        self.profiles.error.set(String::new());
        self.task(async move {
            let loaded = blocking(move || {
                let path = path.canonicalize().map_err(|e| e.to_string())?;
                let profile = profile::load(&path)?;
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
            if self
                .profiles
                .applied
                .with_untracked(|applied| applied.contains(&path))
            {
                self.profiles.busy.set(false);
                if let Some((name, pane)) = destination {
                    self.place_file_view(&path, &name, pane);
                } else {
                    self.status
                        .set("Profile already loaded; drag a view onto a plot".into());
                }
                return;
            }
            let count = (profile.filters.len() + profile.geometry.len()).max(1);
            let first_id = self.id();
            for _ in 1..count {
                self.id();
            }
            let base = path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            let cancelled = cancel.clone();
            let result = blocking(move || {
                profile::prepare::prepare(profile, &log, &base, first_id, cancelled)
            })
            .await;
            if self.generation.get_untracked() != generation || cancel.load(Ordering::Relaxed) {
                return;
            }
            let view_start = self.profiles.views.with_untracked(Vec::len);
            let view_names = result
                .as_ref()
                .map(|prepared| {
                    prepared
                        .views
                        .iter()
                        .map(|view| view.name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            match result.and_then(|prepared| self.commit_profile(prepared)) {
                Ok(name) => {
                    self.profiles.applied.update(|applied| {
                        applied.insert(path.clone());
                    });
                    self.profiles.loaded_views.update_value(|loaded| {
                        loaded.insert(
                            path.clone(),
                            view_names
                                .into_iter()
                                .enumerate()
                                .map(|(index, name)| (name, view_start + index))
                                .collect(),
                        );
                    });
                    self.profiles.busy.set(false);
                    self.status.set(format!("Applied profile: {name}"));
                    if let Some((name, pane)) = destination {
                        self.place_file_view(&path, &name, pane);
                    }
                    self.next_default_profile();
                }
                Err(error) => self.profile_failed(error),
            }
        });
    }

    fn place_file_view(self, path: &std::path::Path, name: &str, pane: u64) {
        let index = self.profiles.loaded_views.with_value(|loaded| {
            loaded.get(path).and_then(|views| {
                views
                    .iter()
                    .find(|(label, _)| label == name)
                    .map(|(_, index)| *index)
            })
        });
        match index {
            Some(index) => self.add_profile_view_to(index, pane),
            None => self.profiles.error.set(format!(
                "View {name} has no usable fields or geometry; other profile resources loaded"
            )),
        }
    }

    pub fn add_profile_view_to(self, index: usize, pane: u64) {
        if self.find(pane).is_none() {
            self.profiles
                .error
                .set("Destination plot was closed".into());
            return;
        }
        let unnamed_empty = self
            .find(pane)
            .is_some_and(|pane| pane.lines.with_untracked(Vec::is_empty));
        let name = self
            .profiles
            .views
            .with_untracked(|views| views.get(index).map(|view| view.name.clone()));
        self.activate(pane);
        match self.place_profile_view(index, false) {
            Ok(()) => {
                if unnamed_empty && let Some(name) = name {
                    self.profiles.titles.update(|titles| {
                        titles.entry(pane).or_insert(name);
                    });
                }
            }
            Err(error) => self.profiles.error.set(error),
        }
    }

    fn profile_failed(self, error: String) {
        self.profiles.busy.set(false);
        self.profiles
            .warnings
            .update(|warnings| warnings.push(error.clone()));
        self.profiles.error.set(error.clone());
        self.status.set(format!("Profile: {error}"));
        self.next_default_profile();
    }

    fn commit_profile(self, prepared: Prepared) -> Result<String, String> {
        if self.derived_busy.get_untracked() {
            return Err("Filters changed during profile preparation; retry".into());
        }
        let log = self.log.get_untracked().ok_or("Log no longer available")?;
        let mut fields = self.derived.get_untracked();
        fields.extend(prepared.fields);
        derived::validate(&fields, &log)?;
        for (source, metadata) in &prepared.metadata {
            if self
                .profiles
                .metadata
                .with_untracked(|existing| existing.get(source).is_some_and(|old| old != metadata))
            {
                return Err(format!("Conflicting signal metadata for {source}"));
            }
        }
        for (source, label) in &prepared.groups {
            if self
                .profiles
                .groups
                .with_untracked(|groups| groups.get(source).is_some_and(|old| old != label))
            {
                return Err(format!("Conflicting field group label for {source}"));
            }
        }
        let mut names: BTreeSet<_> = self
            .geometry
            .items
            .with_untracked(|items| items.iter().map(|item| item.name.to_lowercase()).collect());
        for item in &prepared.geometry {
            if !names.insert(item.name.to_lowercase()) {
                return Err(format!("Geometry name already exists: {}", item.name));
            }
        }
        self.profiles
            .warnings
            .update(|warnings| warnings.extend(prepared.warnings));
        // All fallible decoding, compilation, computation and validation finished.
        self.derived.set(fields);
        self.profiles
            .groups
            .update(|groups| groups.extend(prepared.groups));
        self.profiles
            .metadata
            .update(|metadata| metadata.extend(prepared.metadata));
        self.geometry
            .items
            .update(|items| items.extend(prepared.geometry));
        self.expanded.update(|expanded| {
            expanded.insert("@derived".into());
        });
        self.finish_filter_editing();
        for pane in self.panes.get_untracked() {
            pane.lines.update(|lines| {
                for line in lines {
                    line.name = self.field_name(&line.source);
                }
            });
        }
        self.profiles.views.update(|views| {
            views.extend(prepared.views.into_iter().map(|mut view| {
                view.name = format!("{} / {}", prepared.name, view.name);
                view
            }));
        });
        self.filters_open.set(false);
        Ok(prepared.name)
    }
    pub fn add_profile_view(self, index: usize, new_panel: bool) {
        let result = self.place_profile_view(index, new_panel);
        if let Err(error) = result {
            self.profiles.error.set(error);
        }
    }

    fn place_profile_view(self, index: usize, new_panel: bool) -> Result<(), String> {
        if self.loading.get_untracked()
            || self.derived_busy.get_untracked()
            || self.profiles.busy.get_untracked()
        {
            return Err("Wait for loading and filters to finish".into());
        }
        let view = self
            .profiles
            .views
            .with_untracked(|views| views.get(index).cloned())
            .ok_or("View no longer available")?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        geometry::{Geometry, GeometryKind},
        log::LogStore,
        profile::{Metadata, prepare::PreparedView},
    };

    fn prepared(workbench: Workbench) -> Prepared {
        let log = workbench.log.get_untracked().unwrap();
        let source = "esc_status/0/esc[5].esc_rpm".to_owned();
        let data = log.prepare(&source).unwrap();
        let id = workbench.id();
        let mut item = Geometry::new(id, GeometryKind::Band, kurbo::Point::ZERO);
        item.name = "Flight interval".into();
        item.coordinates = [0., 0., 1., 1.];
        Prepared {
            name: "Test setup".into(),
            warnings: Vec::new(),
            groups: BTreeMap::new(),
            fields: Vec::new(),
            metadata: BTreeMap::from([(
                source.clone(),
                Metadata {
                    name: "ESC 6".into(),
                    unit: "rpm".into(),
                    axis: "motor6".into(),
                },
            )]),
            geometry: vec![item],
            views: vec![PreparedView {
                name: "ESC view".into(),
                fields: vec![(source, data)],
                geometry: vec![id],
                tab: false,
                stacked: true,
            }],
        }
    }

    #[test]
    fn profile_views_add_to_active_or_new_panel_without_duplicate_lines() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        workbench.log.set(Some(Arc::new(
            LogStore::read(std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/esc_status_log.ulg"
            )))
            .unwrap(),
        )));
        workbench.commit_profile(prepared(workbench)).unwrap();
        assert_eq!(
            workbench.panes.with_untracked(Vec::len),
            1,
            "Loading must not create panels"
        );
        assert_eq!(workbench.profiles.views.with_untracked(Vec::len), 1);
        workbench.place_profile_view(0, false).unwrap();
        workbench.place_profile_view(0, false).unwrap();
        assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 1);
        assert_eq!(workbench.pane().geometry.with_untracked(BTreeSet::len), 1);
        assert_eq!(
            workbench.pane().lines.get_untracked()[0].name,
            "ESC 6 [rpm]"
        );
        workbench.place_profile_view(0, true).unwrap();
        assert_eq!(workbench.panes.with_untracked(Vec::len), 2);
        assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 1);
        assert_eq!(
            workbench.profiles.titles.get_untracked()[&workbench.pane().id],
            "Test setup / ESC view"
        );
    }

    #[test]
    fn profile_conflict_and_deleted_geometry_do_not_change_existing_work() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        workbench.log.set(Some(Arc::new(
            LogStore::read(std::path::Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/esc_status_log.ulg"
            )))
            .unwrap(),
        )));
        workbench.commit_profile(prepared(workbench)).unwrap();
        let before = workbench.geometry.items.with_untracked(Vec::len);
        assert!(workbench.commit_profile(prepared(workbench)).is_err());
        assert_eq!(workbench.geometry.items.with_untracked(Vec::len), before);
        assert_eq!(workbench.profiles.views.with_untracked(Vec::len), 1);
        workbench.geometry.items.set(Vec::new());
        assert!(workbench.place_profile_view(0, true).is_err());
        assert_eq!(workbench.panes.with_untracked(Vec::len), 1);
        assert_eq!(workbench.pane().lines.with_untracked(Vec::len), 0);
    }
}
