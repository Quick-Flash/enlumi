use crate::{
    profile::{
        self,
        identity::{Drag, Item},
    },
    state::Workbench,
};
use zgui::prelude::*;

impl Workbench {
    pub fn delete_saved_preset(self, drag: Drag) {
        if self.authoring.busy.get_untracked() || self.profiles.busy.get_untracked() {
            self.status.set("Wait for profile changes to finish".into());
            return;
        }
        let collection = match &drag.item {
            Item::Workspace(_) => "view_presets",
            Item::WorkspacePreset(_) => "workspace_presets",
            Item::View(name) if !crate::preset_lookup::is_nested_plot(name) => "plot_presets",
            _ => return,
        };
        self.authoring.busy.set(true);
        self.task(async move {
            let selected = drag.clone();
            let result = blocking(move || {
                let path = &selected.profile.path;
                let previous = std::fs::read(path).map_err(|error| error.to_string())?;
                let mut document: serde_json::Value = serde_json::from_slice(&previous).map_err(|error| error.to_string())?;
                let name = selected.item.name();
                let collection = if collection == "plot_presets" && !document[collection].as_array().is_some_and(|entries| entries.iter().any(|entry| entry["name"] == name)) { "views" } else { collection };
                let entries = document[collection].as_array_mut().ok_or("Preset no longer exists")?;
                let index = entries.iter().position(|entry| entry["name"] == name).ok_or("Preset no longer exists")?;
                entries.remove(index);
                let confirm = rfd::MessageDialog::new()
                    .set_title("Delete saved preset?")
                    .set_description(format!("Delete preset “{name}” from {}?\nExisting plots and shared fields stay available.", path.display()))
                    .set_level(rfd::MessageLevel::Warning)
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if confirm != rfd::MessageDialogResult::Yes { return Ok(None); }
                crate::preset_store::write_document(document, path, Some(previous), Default::default(), Some(path))?;
                profile::load(path).map(Some)
            }).await;
            self.authoring.busy.set(false);
            match result {
                Ok(Some(definition)) => {
                    self.profiles.instances.update(|instances| {
                        for (key, instance) in instances.iter_mut().filter(|(key, _)| key.path == drag.profile.path) {
                            instance.definition = definition.clone().for_flight(key.flight);
                            instance.views.retain(|name, _| match &drag.item {
                                Item::Workspace(view) => name.strip_prefix("@workspace/").and_then(|name|name.rsplit_once('/')).is_none_or(|(name, _)|name != view),
                                Item::View(view) => name != view,
                                Item::WorkspacePreset(workspace) => name.strip_prefix("@workspace-set/").and_then(|name|name.rsplit_once('/')).and_then(|(name,_)|name.rsplit_once('/')).is_none_or(|(name,_)|name!=workspace),
                                _ => true,
                            });
                            instance.placements.retain(|placement| placement.item != drag.item);
                        }
                    });
                    if matches!(drag.item,Item::WorkspacePreset(_)) {
                        for workspace in self.workspaces.items.get_untracked() {
                            if workspace.source.with_untracked(|source|source.as_ref().is_some_and(|source|source.path==drag.profile.path&&source.name==drag.item.name())){workspace.source.set(None);}
                        }
                    }
                    for view in self.all_views() {
                        if matches!(drag.item, Item::Workspace(_)) && view.preset.with_untracked(|source| source.as_ref().is_some_and(|source| source.path == drag.profile.path && source.name == drag.item.name())) { view.preset.set(None); }
                        if matches!(drag.item, Item::View(_)) {
                            for pane in view.panes.get_untracked() {
                                if pane.preset.with_untracked(|source| source.as_ref().is_some_and(|source|source.path == drag.profile.path && source.name == drag.item.name())) { pane.preset.set(None); }
                            }
                        }
                    }
                    self.authoring.revision.update(|revision| *revision += 1);
                    self.status.set(format!("Deleted preset: {}", drag.item.name()));
                }
                Ok(None) => {}
                Err(error) => self.profiles.error.set(error),
            }
        });
    }
}
