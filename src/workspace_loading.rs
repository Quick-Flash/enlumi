//! Workspace layout and preview lines appear before profile resources commit.
use crate::{
    preset::{PlotPreset, WorkspacePreset},
    profile::{identity::ProfileKey, progress::Progress},
    profile_instance::AppliedProfile,
    state::{Line, Workbench},
};
use std::collections::BTreeMap;
use zgui::prelude::*;

#[derive(Clone)]
struct LoadingPlot {
    view: u64,
    pane: u64,
    recipe: PlotPreset,
    lines: BTreeMap<String, u64>,
}
#[derive(Clone)]
pub(crate) struct WorkspaceLoad {
    pub workspace: u64,
    pub key: ProfileKey,
    generation: u64,
    recipe: String,
    plots: Vec<LoadingPlot>,
    finishing: bool,
}
impl Workbench {
    pub(crate) fn workspace_finishing(self) -> bool {
        self.authoring
            .workspace_load
            .with_value(|load| load.as_ref().is_some_and(|load| load.finishing))
    }
    pub(crate) fn begin_workspace_load(
        self,
        key: &ProfileKey,
        recipe: WorkspacePreset,
    ) -> Result<(), String> {
        let definition = serde_json::to_string(&recipe).map_err(|error| error.to_string())?;
        if self.authoring.workspace_load.with_value(|load| {
            load.as_ref().is_some_and(|load| {
                load.key == *key
                    && load.recipe == definition
                    && load.generation == self.generation.get_untracked()
            })
        }) {
            return Ok(());
        }
        let mut root = self;
        root.bound_view = None;
        root.open_workspace_preset(key, recipe.clone())?;
        let workspace = root.workspace();
        let mut plots = Vec::new();
        for (view, recipe) in self
            .workspace_views(workspace.id)
            .into_iter()
            .zip(recipe.views)
        {
            for (pane, recipe) in view.panes.get_untracked().into_iter().zip(recipe.plots) {
                pane.loading.set(Some("Preparing fields…".into()));
                plots.push(LoadingPlot {
                    view: view.id,
                    pane: pane.id,
                    recipe,
                    lines: BTreeMap::new(),
                });
            }
        }
        workspace.loading.set(Some("Preparing profile…".into()));
        self.authoring.workspace_load.set_value(Some(WorkspaceLoad {
            workspace: workspace.id,
            key: key.clone(),
            recipe: definition,
            generation: self.generation.get_untracked(),
            plots,
            finishing: false,
        }));
        self.status
            .set("Loading workspace: preparing fields…".into());
        Ok(())
    }

    pub(crate) fn stream_workspace(
        self,
        key: ProfileKey,
        generation: u64,
    ) -> async_channel::Sender<Progress> {
        let (sender, receiver) = async_channel::bounded::<Progress>(2);
        self.task(async move {
            while let Ok(mut progress) = receiver.recv().await {
                while let Ok(latest) = receiver.try_recv() {
                    progress = latest;
                }
                if self.generation.get_untracked() != generation {
                    break;
                }
                self.preview_workspace(&key, progress);
            }
        });
        sender
    }

    pub(crate) fn preview_workspace(self, key: &ProfileKey, progress: Progress) {
        self.authoring.workspace_load.update_value(|load| {
            let Some(load) = load else {
                return;
            };
            if load.key != *key
                || load.finishing
                || load.generation != self.generation.get_untracked()
            {
                return;
            }
            if !self
                .workspaces
                .items
                .with_untracked(|items| items.iter().any(|item| item.id == load.workspace))
            {
                return;
            }
            for target in &mut load.plots {
                let Some(workbench) = self.for_view(target.view) else {
                    continue;
                };
                let Some(pane) = workbench.find(target.pane) else {
                    continue;
                };
                if target.recipe.analysis.is_some() {
                    continue;
                }
                for style in &target.recipe.lines {
                    let source = style
                        .source
                        .strip_prefix('$')
                        .and_then(|id| progress.symbols.get(id))
                        .cloned()
                        .unwrap_or_else(|| style.source.clone());
                    let Some(data) = progress.data.get(&source) else {
                        continue;
                    };
                    let name = progress
                        .metadata
                        .get(&source)
                        .map(|metadata| metadata.name.clone())
                        .unwrap_or_else(|| source.clone());
                    let id = target
                        .lines
                        .get(&style.source)
                        .copied()
                        .or_else(|| {
                            pane.lines.with_untracked(|lines| {
                                lines
                                    .iter()
                                    .find(|line| line.source == source)
                                    .map(|line| line.id)
                            })
                        })
                        .unwrap_or_else(|| self.id());
                    let changed = pane.lines.with_untracked(|lines| {
                        lines.iter().find(|line| line.id == id).is_none_or(|line| {
                            line.source != source
                                || line.name != name
                                || !std::ptr::eq(line.data.samples(), data.samples())
                        })
                    });
                    if changed {
                        pane.lines.update(|lines| {
                            if let Some(line) = lines.iter_mut().find(|line| line.id == id) {
                                line.source = source.clone();
                                line.data = data.clone();
                                line.name = name.clone();
                                line.revision += 1;
                            } else {
                                lines.push(Line {
                                    id,
                                    source,
                                    name,
                                    data: data.clone(),
                                    revision: 0,
                                });
                            }
                        });
                    }
                    if target.lines.insert(style.source.clone(), id).is_none() {
                        crate::preset_apply::apply_line_appearance(pane, id, style);
                    }
                }
                pane.loading.set(Some(format!(
                    "Loading fields {}/{}",
                    target.lines.len(),
                    target.recipe.lines.len()
                )));
            }
            let status = format!("Loading workspace · {} fields ready", progress.data.len());
            self.workspaces.items.with_untracked(|items| {
                if let Some(workspace) = items
                    .iter()
                    .find(|workspace| workspace.id == load.workspace)
                {
                    workspace.loading.set(Some(status.clone()));
                }
            });
            if self.workspaces.active.get_untracked() == load.workspace {
                self.status.set(status);
            }
        });
    }

    pub(crate) fn complete_workspace_load(self, instance: &AppliedProfile) {
        let Some(mut load) = self.authoring.workspace_load.get_value() else {
            return;
        };
        if load.finishing {
            return;
        }
        load.finishing = true;
        self.authoring.workspace_load.set_value(Some(load.clone()));
        for target in load.plots {
            let Some(workbench) = self.for_view(target.view) else {
                continue;
            };
            let Some(pane) = workbench.find(target.pane) else {
                continue;
            };
            pane.lines.update(|lines| {
                for (token, id) in &target.lines {
                    if let Some(source) = instance.resolve(token) {
                        if let Some(line) = lines.iter_mut().find(|line| line.id == *id) {
                            line.name = workbench.field_name_untracked(&source);
                            if let Some(data) = instance
                                .fields
                                .iter()
                                .find(|field| field.key() == source)
                                .and_then(|field| field.data.clone())
                                .or_else(|| {
                                    instance
                                        .views
                                        .values()
                                        .flat_map(|view| &view.fields)
                                        .find(|(id, _)| id == &source)
                                        .map(|(_, data)| data.clone())
                                })
                            {
                                line.data = data;
                                line.revision += 1;
                            }
                            line.source = source;
                        }
                    } else {
                        lines.retain(|line| line.id != *id);
                    }
                }
            });
            self.profiles.instances.update(|instances| {
                if let Some(instance) = instances.get_mut(&instance.key) {
                    for (token, id) in &target.lines {
                        if pane
                            .lines
                            .with_untracked(|lines| lines.iter().any(|line| line.id == *id))
                        {
                            instance
                                .placements
                                .push(crate::profile_instance::Placement {
                                    pane: pane.id,
                                    item: crate::profile::identity::Item::Field(token.clone()),
                                    lines: BTreeMap::from([(token.clone(), *id)]),
                                });
                        }
                    }
                }
            });
            pane.loading.set(Some(
                if target.recipe.analysis.is_some() {
                    "Computing analysis…"
                } else {
                    "Finishing plot…"
                }
                .into(),
            ));
            workbench.populate_preset(&instance.key, target.recipe, pane, false);
        }
        self.finish_workspace_load();
    }

    pub(crate) fn finish_workspace_load(self) {
        let done = self.authoring.workspace_load.with_value(|load| {
            load.as_ref().is_some_and(|load| {
                load.finishing
                    && load.plots.iter().all(|target| {
                        self.for_view(target.view)
                            .and_then(|workbench| workbench.find(target.pane))
                            .is_none_or(|pane| pane.loading.with_untracked(Option::is_none))
                    })
            })
        });
        if done {
            self.clear_workspace_load();
        }
    }

    pub(crate) fn clear_workspace_load(self) {
        if let Some(load) = self.authoring.workspace_load.get_value() {
            self.workspaces.items.with_untracked(|items| {
                if let Some(workspace) = items
                    .iter()
                    .find(|workspace| workspace.id == load.workspace)
                {
                    workspace.loading.set(None);
                }
            });
            for target in load.plots {
                if let Some(pane) = self
                    .for_view(target.view)
                    .and_then(|workbench| workbench.find(target.pane))
                {
                    pane.loading.set(None);
                }
            }
        }
        self.authoring.workspace_load.set_value(None);
    }

    pub(crate) fn fail_workspace_load(self, error: &str) {
        if let Some(load) = self.authoring.workspace_load.get_value() {
            if load.finishing
                || self
                    .profiles
                    .loading_profile
                    .with_untracked(|key| key.as_ref().is_some_and(|key| key != &load.key))
            {
                return;
            }
            let message = format!("Load failed: {error} · select workspace to retry");
            self.workspaces.items.with_untracked(|items| {
                if let Some(workspace) = items
                    .iter()
                    .find(|workspace| workspace.id == load.workspace)
                {
                    workspace.loading.set(Some(message.clone()));
                }
            });
            for target in load.plots {
                if let Some(pane) = self
                    .for_view(target.view)
                    .and_then(|workbench| workbench.find(target.pane))
                {
                    pane.loading.set(Some(message.clone()));
                }
            }
        }
        self.authoring.workspace_load.set_value(None);
    }
}
