use crate::{
    preset_capture::{self, Bundle, CaptureKind},
    state::Workbench,
};
use std::path::PathBuf;
use zgui::prelude::*;
#[derive(Clone)]
pub struct SaveSession {
    pub view: u64,
    pub kind: Option<CaptureKind>,
    pub bundle: Bundle,
    pub origin: Option<PathBuf>,
    pub previous: Option<Vec<u8>>,
}
#[derive(Clone)]
pub struct SavedCapture {
    pub view: u64,
    pub kind: CaptureKind,
    pub path: PathBuf,
    pub name: String,
}
#[derive(Clone)]
pub struct ApplyRequest {
    pub key: crate::profile::identity::ProfileKey,
    pub name: String,
    pub view: u64,
    pub action: ApplyAction,
}
#[derive(Clone, Copy)]
pub enum ApplyAction {
    Plot,
    NewView,
    AppendView,
    Workspace,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveMode {
    Review,
    Overwrite,
    Copy,
}
#[derive(Clone, Copy)]
pub struct Authoring {
    pub baseline: StoredValue<Option<serde_json::Value>, LocalStorage>,
    pub session: RwSignal<Option<SaveSession>, LocalStorage>,
    pub open: RwSignal<bool, LocalStorage>,
    pub name: RwSignal<String, LocalStorage>,
    pub path: RwSignal<String, LocalStorage>,
    pub replace: RwSignal<bool, LocalStorage>,
    pub busy: RwSignal<bool, LocalStorage>,
    pub error: RwSignal<String, LocalStorage>,
    pub revision: RwSignal<u64, LocalStorage>,
    pub browse: RwSignal<Option<crate::preset_browser::BrowseRequest>, LocalStorage>,
    pub pending: StoredValue<Option<ApplyRequest>, LocalStorage>,
    pub saved: StoredValue<Option<SavedCapture>, LocalStorage>,
    pub restoring: StoredValue<std::collections::BTreeMap<u64, usize>, LocalStorage>,
    pub field_edit: RwSignal<Option<String>, LocalStorage>,
}
impl Authoring {
    pub fn new() -> Self {
        Self {
            baseline: StoredValue::new_local(None),
            browse: RwSignal::new_local(None),
            session: RwSignal::new_local(None),
            open: RwSignal::new_local(false),
            name: RwSignal::new_local(String::new()),
            path: RwSignal::new_local(String::new()),
            replace: RwSignal::new_local(false),
            busy: RwSignal::new_local(false),
            error: RwSignal::new_local(String::new()),
            revision: RwSignal::new_local(0),
            pending: StoredValue::new_local(None),
            saved: StoredValue::new_local(None),
            restoring: StoredValue::new_local(Default::default()),
            field_edit: RwSignal::new_local(None),
        }
    }
}
impl Workbench {
    pub fn begin_save(self, kind: CaptureKind) {
        self.begin_save_from(kind, None, SaveMode::Review);
    }
    pub fn save_preset(self, kind: CaptureKind) {
        if self.authoring.busy.get_untracked() {
            return;
        }
        let source = self.preset_source(&kind);
        self.begin_save_from(kind, source, SaveMode::Overwrite);
    }
    pub fn save_preset_as(self, kind: CaptureKind) {
        if self.authoring.busy.get_untracked() {
            return;
        }
        let source = self.preset_source(&kind);
        self.begin_save_from(kind, source, SaveMode::Copy);
    }
    fn preset_source(self, kind: &CaptureKind) -> Option<crate::preset::PresetSource> {
        let workbench = self.bind_view();
        match kind {
            CaptureKind::Plot(id) => self.find(*id).and_then(|pane| pane.preset.get_untracked()),
            CaptureKind::View => workbench.view().preset.get_untracked(),
            CaptureKind::Workspace => workbench.workspace().source.get_untracked(),
            CaptureKind::Selection(_) => None,
        }
    }
    fn begin_save_from(
        self,
        kind: CaptureKind,
        source: Option<crate::preset::PresetSource>,
        mode: SaveMode,
    ) {
        let workbench = self.bind_view();
        let name = source
            .as_ref()
            .map(|source| {
                if mode == SaveMode::Copy {
                    format!("{} copy", source.name)
                } else {
                    source.name.clone()
                }
            })
            .unwrap_or_else(|| match &kind {
                CaptureKind::Selection(_) => "My fields".into(),
                CaptureKind::Plot(id) => self
                    .profiles
                    .titles
                    .with_untracked(|titles| titles.get(id).cloned())
                    .unwrap_or("My plot".into()),
                CaptureKind::View => workbench.view().name.get_untracked(),
                CaptureKind::Workspace => workbench.workspace().name.get_untracked(),
            });
        match preset_capture::capture(workbench, &kind, &name) {
            Ok(bundle) => {
                let root = self
                    .profiles
                    .directory
                    .get_untracked()
                    .or_else(|| crate::profile::default_directory().ok());
                let file = format!(
                    "{}.json",
                    name.chars()
                        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        })
                        .collect::<String>()
                );
                self.authoring.path.set(source.as_ref().map_or_else(
                    || root.map_or_else(String::new, |root| root.join(file).display().to_string()),
                    |source| source.path.display().to_string(),
                ));
                self.authoring.name.set(name);
                self.authoring.replace.set(
                    mode == SaveMode::Overwrite || (source.is_some() && mode == SaveMode::Review),
                );
                self.authoring.error.set(String::new());
                self.authoring.baseline.set_value(None);
                self.authoring.session.set(Some(SaveSession {
                    view: workbench.view().id,
                    kind: Some(kind),
                    bundle,
                    origin: None,
                    previous: None,
                }));
                self.authoring.open.set(mode != SaveMode::Overwrite);
                if mode == SaveMode::Overwrite {
                    self.save_authoring();
                }
            }
            Err(error) => self.status.set(error),
        }
    }
    pub fn save_selected_profile(self) {
        let sources = self
            .selected_field
            .get_untracked()
            .map(|source| self.drag_fields(source))
            .unwrap_or_default();
        self.queue_field_sources(
            sources,
            crate::profile_state::Destination::SaveSelection,
            None,
        );
    }
    pub fn edit_profile_document(self, path: PathBuf) {
        if self.authoring.busy.get_untracked() {
            return;
        }
        self.authoring.busy.set(true);
        self.profiles.error.set(String::new());
        self.task(async move {
            let source = path.clone();
            let result = blocking(move || {
                crate::profile::load(&source)?;
                let bytes = std::fs::read(&source).map_err(|e| e.to_string())?;
                let value: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                let baseline = value.clone();
                Ok::<_, String>((bytes, value, baseline))
            })
            .await;
            self.authoring.busy.set(false);
            match result {
                Ok((bytes, document, baseline)) => {
                    self.authoring.baseline.set_value(Some(baseline));
                    self.authoring
                        .name
                        .set(document["name"].as_str().unwrap_or("Profile").into());
                    self.authoring.path.set(path.display().to_string());
                    self.authoring.replace.set(false);
                    self.authoring.error.set(String::new());
                    self.authoring.session.set(Some(SaveSession {
                        view: self.views.active.get_untracked(),
                        kind: None,
                        bundle: Bundle {
                            document,
                            files: Default::default(),
                        },
                        origin: Some(path),
                        previous: Some(bytes),
                    }));
                    self.authoring.open.set(true);
                }
                Err(error) => {
                    self.profiles.error.set(error.clone());
                    self.status.set(error);
                }
            }
        });
    }
    pub fn save_authoring(self) {
        if self.authoring.busy.get_untracked() {
            return;
        }
        let Some(mut session) = self.authoring.session.get_untracked() else {
            return;
        };
        let name = self.authoring.name.get_untracked().trim().to_owned();
        if name.is_empty() {
            self.authoring.error.set("Choose a name".into());
            return;
        }
        let path = PathBuf::from(self.authoring.path.get_untracked());
        let replace = self.authoring.replace.get_untracked();
        session.bundle.document["name"] = serde_json::json!(name);
        let collection = match session.kind {
            Some(CaptureKind::Plot(_)) => Some("plot_presets"),
            Some(CaptureKind::View) => Some("view_presets"),
            Some(CaptureKind::Workspace) => Some("workspace_presets"),
            _ => None,
        };
        if let Some(collection) = collection {
            session.bundle.document[collection][0]["name"] = serde_json::json!(name);
        }
        self.authoring.busy.set(true);
        self.authoring.error.set(String::new());
        let resources = self.profiles.instances.with_untracked(|instances| {
            self.derived.with_untracked(|fields| {
                instances
                    .values()
                    .flat_map(|instance| {
                        instance.ids.fields.iter().filter_map(move |(name, id)| {
                            let live = fields.iter().any(|field| field.id == *id);
                            (live || instance.symbols.contains_key(name))
                                .then(|| (instance.key.path.clone(), name.clone(), live))
                        })
                    })
                    .collect::<Vec<_>>()
            })
        });
        self.task(async move {
            let destination = path.clone();
            let saved = session.clone();
            let result = blocking(move || {
                let result = if saved.kind.is_none() {
                    let previous = if saved.origin.as_ref() == Some(&destination) {
                        saved.previous
                    } else {
                        match std::fs::read(&destination) {
                            Ok(bytes) if replace => Some(bytes),
                            Ok(_) => {
                                return Err(
                                    "Destination exists. Enable Replace existing entries.".into()
                                );
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                            Err(e) => return Err(e.to_string()),
                        }
                    };
                    crate::preset_store::write_document(
                        saved.bundle.document,
                        &destination,
                        previous,
                        saved.bundle.files,
                        saved.origin.as_deref(),
                    )
                } else {
                    let canonical = destination
                        .canonicalize()
                        .unwrap_or_else(|_| destination.clone());
                    let mut deleted = resources
                        .iter()
                        .filter(|(path, _, live)| path == &canonical && !live)
                        .map(|(_, id, _)| id.clone())
                        .collect::<std::collections::BTreeSet<_>>();
                    for (_, id, _) in resources
                        .iter()
                        .filter(|(path, _, live)| path == &canonical && *live)
                    {
                        deleted.remove(id);
                    }
                    crate::preset_store::save_with_deleted_filters(
                        saved.bundle,
                        &destination,
                        replace,
                        saved.origin.as_deref(),
                        &deleted,
                    )
                };
                result?;
                destination
                    .canonicalize()
                    .map_err(|error| error.to_string())
            })
            .await;
            self.authoring.busy.set(false);
            match result {
                Ok(canonical) => {
                    let saved_capture = session.kind.clone().map(|kind| SavedCapture {
                        view: session.view,
                        kind,
                        path: canonical.clone(),
                        name: name.clone(),
                    });
                    self.authoring.open.set(false);
                    self.authoring.revision.update(|revision| *revision += 1);
                    self.status.set(format!("Saved {}", path.display()));
                    if let Some(workbench) = self.for_view(session.view)
                        && let Some(kind) = session.kind
                    {
                        let baseline = preset_capture::capture(workbench, &kind, &name)
                            .ok()
                            .and_then(|bundle| serde_json::to_string(&bundle.document).ok())
                            .unwrap_or_default();
                        let source = crate::preset::PresetSource {
                            path: canonical.clone(),
                            name: name.clone(),
                            baseline,
                        };
                        match kind {
                            CaptureKind::Plot(id) => {
                                if let Some(pane) = self.find(id) {
                                    pane.preset.set(Some(source));
                                }
                            }
                            CaptureKind::View => workbench.view().preset.set(Some(source)),
                            CaptureKind::Workspace => {
                                workbench.workspace().source.set(Some(source))
                            }
                            _ => {}
                        }
                    }
                    let keys = self.profiles.instances.with_untracked(|instances| {
                        instances
                            .keys()
                            .filter(|key| key.path == canonical)
                            .cloned()
                            .collect::<Vec<_>>()
                    });
                    if let Some(key) = keys.first() {
                        self.authoring.saved.set_value(saved_capture);
                        self.reload_profile(key.clone());
                    } else if self.log.with_untracked(Option::is_some)
                        && !self.profiles.busy.get_untracked()
                        && !self.derived_busy.get_untracked()
                    {
                        self.authoring.saved.set_value(saved_capture);
                        self.apply_profile(canonical);
                    }
                }
                Err(error) => {
                    self.status.set(format!("Could not save preset: {error}"));
                    self.authoring.error.set(error);
                }
            }
        });
    }
    pub(crate) fn refresh_saved_capture(self, key: &crate::profile::identity::ProfileKey) {
        let Some(saved) = self
            .authoring
            .saved
            .get_value()
            .filter(|saved| saved.path == key.path)
        else {
            return;
        };
        self.authoring.saved.set_value(None);
        let Some(workbench) = self.for_view(saved.view) else {
            return;
        };
        let Ok(bundle) = preset_capture::capture(workbench, &saved.kind, &saved.name) else {
            return;
        };
        let source = crate::preset::PresetSource {
            path: saved.path,
            name: saved.name,
            baseline: serde_json::to_string(&bundle.document).unwrap_or_default(),
        };
        match saved.kind {
            CaptureKind::Plot(id) => {
                if let Some(pane) = workbench.find(id) {
                    pane.preset.set(Some(source));
                }
            }
            CaptureKind::View => workbench.view().preset.set(Some(source)),
            CaptureKind::Workspace => workbench.workspace().source.set(Some(source)),
            CaptureKind::Selection(_) => {}
        }
    }
    pub fn update_preset(self, kind: CaptureKind, source: crate::preset::PresetSource) {
        self.begin_save_from(kind, Some(source), SaveMode::Review);
    }
    pub fn preset_modified(self, kind: &CaptureKind, source: &crate::preset::PresetSource) -> bool {
        preset_capture::capture_for_comparison(self.bind_view(), kind, &source.name)
            .ok()
            .and_then(|bundle| serde_json::to_string(&bundle.document).ok())
            .is_none_or(|document| document != source.baseline)
    }
}
