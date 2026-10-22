use crate::{
    preset::PresetSource,
    state::Workbench,
    view_state::{Views, WorkspaceView},
};
use std::{collections::BTreeSet, path::PathBuf, sync::Arc};
use zgui::prelude::*;

#[derive(Clone)]
pub struct Workspace {
    pub id: u64,
    pub name: RwSignal<String, LocalStorage>,
    pub loading: RwSignal<Option<String>, LocalStorage>,
    pub source: RwSignal<Option<PresetSource>, LocalStorage>,
    pub views: Vec<WorkspaceView>,
    pub active_view: u64,
    pub mounted: BTreeSet<u64>,
    pub generation: u64,
}

#[derive(Clone)]
pub struct SavedWorkspace {
    pub path: PathBuf,
    pub name: String,
    pub views: usize,
    pub definition: Arc<crate::profile::Profile>,
}

#[derive(Clone, Copy)]
pub struct Workspaces {
    pub items: RwSignal<Vec<Workspace>, LocalStorage>,
    pub active: RwSignal<u64, LocalStorage>,
    pub saved: RwSignal<Vec<SavedWorkspace>, LocalStorage>,
    next: RwSignal<u64, LocalStorage>,
}

impl Workspaces {
    pub fn new(views: Views) -> Self {
        Self {
            items: RwSignal::new_local(vec![Workspace {
                id: 1,
                name: RwSignal::new_local("Untitled".into()),
                loading: RwSignal::new_local(None),
                source: RwSignal::new_local(None),
                views: views.items.get_untracked(),
                active_view: views.active.get_untracked(),
                mounted: views.mounted.get_untracked(),
                generation: 0,
            }]),
            active: RwSignal::new_local(1),
            saved: RwSignal::new_local(Vec::new()),
            next: RwSignal::new_local(2),
        }
    }
}

impl Workbench {
    pub fn workspace(self) -> Workspace {
        let id = self.workspace_id();
        self.workspaces.items.with(|items| {
            items
                .iter()
                .find(|item| item.id == id)
                .cloned()
                .expect("Workspace exists")
        })
    }
    pub fn workspace_id(self) -> u64 {
        if let Some(view) = self.bound_view
            && !self
                .views
                .items
                .with_untracked(|views| views.iter().any(|item| item.id == view.id))
        {
            return self
                .workspaces
                .items
                .with_untracked(|items| {
                    items
                        .iter()
                        .find(|item| item.views.iter().any(|item| item.id == view.id))
                        .map(|item| item.id)
                })
                .unwrap_or_else(|| self.workspaces.active.get_untracked());
        }
        self.workspaces.active.get()
    }
    pub fn workspace_views(self, id: u64) -> Vec<WorkspaceView> {
        if self.workspaces.active.get_untracked() == id {
            self.views.items.get_untracked()
        } else {
            self.workspaces.items.with_untracked(|items| {
                items
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| item.views.clone())
                    .unwrap_or_default()
            })
        }
    }
    pub fn all_views(self) -> Vec<WorkspaceView> {
        let active = self.workspaces.active.get_untracked();
        let mut views = self.views.items.get_untracked();
        self.workspaces.items.with_untracked(|items| {
            for item in items.iter().filter(|item| item.id != active) {
                views.extend_from_slice(&item.views);
            }
        });
        views
    }
    pub fn switch_workspace(self, id: u64) {
        let current = self.workspaces.active.get_untracked();
        if current == id {
            return;
        }
        let Some(target) = self
            .workspaces
            .items
            .with_untracked(|items| items.iter().find(|item| item.id == id).cloned())
        else {
            return;
        };
        self.finish_view_gestures();
        self.workspaces.items.update(|items| {
            if let Some(item) = items.iter_mut().find(|item| item.id == current) {
                item.views = self.views.items.get_untracked();
                item.active_view = self.views.active.get_untracked();
                item.mounted = self.views.mounted.get_untracked();
            }
        });
        self.workspaces.active.set(id);
        self.views.items.set(target.views);
        self.views.active.set(target.active_view);
        self.views.mounted.set(target.mounted);
    }
    pub fn new_workspace(self, name: String) -> u64 {
        let view = self.create_detached_view();
        let id = self.workspaces.next.get_untracked();
        self.workspaces.next.set(id + 1);
        let workspace = self.owner.with_value(|owner| {
            owner.with(|| Workspace {
                id,
                name: RwSignal::new_local(name),
                loading: RwSignal::new_local(None),
                source: RwSignal::new_local(None),
                views: vec![view],
                active_view: view.id,
                mounted: BTreeSet::from([view.id]),
                generation: self.generation.get_untracked(),
            })
        });
        self.workspaces.items.update(|items| items.push(workspace));
        self.switch_workspace(id);
        id
    }
    pub fn close_workspace(self, id: u64) {
        let next = self
            .workspaces
            .items
            .with_untracked(|items| items.iter().find(|item| item.id != id).map(|item| item.id));
        let Some(next) = next else {
            return;
        };
        if self
            .authoring
            .workspace_load
            .with_value(|load| load.as_ref().is_some_and(|load| load.workspace == id))
        {
            self.profiles.cancel();
            self.authoring.pending.set_value(None);
            self.clear_workspace_load();
        }
        if self.workspaces.active.get_untracked() == id {
            self.switch_workspace(next);
        }
        self.workspaces
            .items
            .update(|items| items.retain(|item| item.id != id));
    }
    pub fn duplicate_workspace(self) -> u64 {
        let mut root = self;
        root.bound_view = None;
        let workspace = root.workspace();
        let views = root.workspace_views(workspace.id);
        let active = root.views.active.get_untracked();
        let id = root.new_workspace(format!("{} copy", workspace.name.get_untracked()));
        let blank = root.views.active.get_untracked();
        let mut selected = None;
        for view in views {
            let copy = root.copy_view(view);
            if view.id == active {
                selected = Some(copy);
            }
        }
        root.close_view(blank);
        if let Some(selected) = selected {
            root.switch_view(selected);
        }
        id
    }
    pub fn select_workspace(self, id: u64) {
        self.switch_workspace(id);
        let workspace = self.workspace();
        if (workspace.generation != self.generation.get_untracked()
            || workspace.loading.with_untracked(|loading| {
                loading
                    .as_ref()
                    .is_some_and(|message| message.starts_with("Load failed:"))
            }))
            && !self.profiles.busy.get_untracked()
            && self.log.with_untracked(Option::is_some)
            && let Some(source) = workspace.source.get_untracked()
        {
            self.use_saved_preset(
                crate::profile::identity::ProfileKey::new(
                    source.path,
                    self.profiles.flight.get_untracked(),
                ),
                source.name,
                crate::preset_ui::ApplyAction::Workspace,
            );
        }
    }
    pub fn open_saved_workspace(self, saved: SavedWorkspace) {
        if let Some(id) = self.workspaces.items.with_untracked(|items| {
            items
                .iter()
                .find(|item| {
                    item.source.with_untracked(|source| {
                        source.as_ref().is_some_and(|source| {
                            source.path == saved.path && source.name == saved.name
                        })
                    })
                })
                .map(|item| item.id)
        }) {
            self.select_workspace(id);
        } else if self.log.with_untracked(Option::is_some) {
            if self.loading.get_untracked()
                || self.profiles.busy.get_untracked()
                || self.derived_busy.get_untracked()
                || self.workspace_finishing()
            {
                self.status
                    .set("Wait for loading and filters to finish".into());
                return;
            }
            let key = crate::profile::identity::ProfileKey::new(
                saved.path.clone(),
                self.profiles.flight.get_untracked(),
            );
            if !self
                .profiles
                .instances
                .with_untracked(|instances| instances.contains_key(&key))
                && let Some(recipe) = saved
                    .definition
                    .workspace_presets
                    .iter()
                    .find(|recipe| recipe.name == saved.name)
                    .cloned()
                && let Err(error) = self.begin_workspace_load(&key, recipe)
            {
                self.status.set(error);
                return;
            }
            self.use_saved_preset(
                crate::profile::identity::ProfileKey::new(
                    saved.path,
                    self.profiles.flight.get_untracked(),
                ),
                saved.name,
                crate::preset_ui::ApplyAction::Workspace,
            );
        } else if let Some(recipe) = saved
            .definition
            .workspace_presets
            .iter()
            .find(|workspace| workspace.name == saved.name)
            .cloned()
            && let Err(error) = self.open_workspace_preset(
                &crate::profile::identity::ProfileKey::new(
                    saved.path,
                    self.profiles.flight.get_untracked(),
                ),
                recipe,
            )
        {
            self.status.set(error);
        }
    }
    pub(crate) fn open_workspace_preset(
        self,
        key: &crate::profile::identity::ProfileKey,
        recipe: crate::preset::WorkspacePreset,
    ) -> Result<(), String> {
        if recipe.views.is_empty() || recipe.active >= recipe.views.len() {
            return Err("Workspace needs at least one view and a valid active view".into());
        }
        let mut root = self;
        root.bound_view = None;
        let existing = root.workspaces.items.with_untracked(|items| {
            items
                .iter()
                .find(|item| {
                    item.source.with_untracked(|source| {
                        source.as_ref().is_some_and(|source| {
                            source.path == key.path && source.name == recipe.name
                        })
                    })
                })
                .map(|item| item.id)
        });
        if let Some(id) = existing {
            root.switch_workspace(id);
            root.finish_view_gestures();
            let first = root.create_detached_view();
            root.views.items.set(vec![first]);
            root.views.active.set(first.id);
            root.views.mounted.set(BTreeSet::from([first.id]));
        } else {
            root.new_workspace(recipe.name.clone());
        }
        let blank = root.views.active.get_untracked();
        let target = root.bind_view();
        let mut views = Vec::new();
        for view in recipe.views {
            target.apply_view_preset(key, view, true)?;
            let created = root
                .views
                .items
                .with_untracked(|views| *views.last().expect("View created"));
            created.preset.set(None);
            if root.log.with_untracked(Option::is_none) {
                root.authoring.restoring.update_value(|pending| {
                    pending.remove(&created.id);
                });
            }
            views.push(created.id);
        }
        root.close_view(blank);
        root.switch_view(views[recipe.active]);
        let workspace = root.workspace();
        workspace.name.set(recipe.name.clone());
        workspace.source.set(Some(PresetSource {
            path: key.path.clone(),
            name: recipe.name,
            baseline: String::new(),
        }));
        root.workspaces.items.update(|items| {
            if let Some(item) = items.iter_mut().find(|item| item.id == workspace.id) {
                item.generation = root.generation.get_untracked();
            }
        });
        Ok(())
    }
}
