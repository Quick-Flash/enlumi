use crate::{state::Workbench, workspace_state::SavedWorkspace};
use std::sync::Arc;
use zgui::prelude::*;

pub fn watch(workbench: Workbench) {
    let scan = RwSignal::new_local(0u64);
    let imported = Memo::new(move |_| {
        workbench.profiles.instances.with(|instances| {
            instances
                .keys()
                .map(|key| key.path.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
    });
    let effect = zgui::reactive::RenderEffect::new(move |_| {
        workbench.authoring.revision.track();
        workbench.profiles.scan.track();
        let directory = workbench.profiles.directory.get();
        let imported = imported.get();
        scan.update(|value| *value += 1);
        let token = scan.get_untracked();
        workbench.task(async move {
            let result = blocking(move || {
                let directory = directory.map_or_else(crate::profile::directory, Ok)?;
                let mut paths = crate::profile::discover_in(&directory)?;
                paths.extend(imported);
                paths.sort();
                paths.dedup();
                let mut saved = Vec::new();
                for path in paths {
                    let Ok(definition) = crate::profile::load(&path) else {
                        continue;
                    };
                    if definition.workspace_presets.is_empty() {
                        continue;
                    }
                    let definition = Arc::new(definition);
                    for workspace in &definition.workspace_presets {
                        saved.push(SavedWorkspace {
                            path: path.clone(),
                            name: workspace.name.clone(),
                            views: workspace.views.len(),
                            definition: definition.clone(),
                        });
                    }
                }
                saved.sort_by(|a, b| {
                    a.name
                        .to_lowercase()
                        .cmp(&b.name.to_lowercase())
                        .then(a.path.cmp(&b.path))
                });
                Ok::<_, String>(saved)
            })
            .await;
            if scan.try_get_untracked() != Some(token) {
                return;
            }
            match result {
                Ok(saved) => {
                    let _ = workbench.workspaces.saved.try_set(saved);
                }
                Err(error) => {
                    let _ = workbench.profiles.error.try_set(error);
                }
            }
        });
    });
    on_cleanup_local(move || drop(effect));
}
