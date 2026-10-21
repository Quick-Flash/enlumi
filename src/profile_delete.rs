use crate::state::Workbench;
use std::path::PathBuf;
use zgui::prelude::*;

impl Workbench {
    pub fn delete_profile_file(self, path: PathBuf) {
        if self.authoring.busy.get_untracked() || self.profiles.busy.get_untracked() {
            self.status.set("Wait for profile changes to finish".into());
            return;
        }
        self.authoring.busy.set(true);
        self.task(async move {
            let selected = path.clone();
            let result = blocking(move || {
                let confirm = rfd::MessageDialog::new()
                    .set_title("Delete profile file?")
                    .set_description(format!("Delete {}?\nLoaded fields and plots stay in this session. File deletion cannot be undone.", selected.display()))
                    .set_level(rfd::MessageLevel::Warning)
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if confirm != rfd::MessageDialogResult::Yes {
                    return Ok(false);
                }
                std::fs::remove_file(selected).map(|()| true).map_err(|error| error.to_string())
            }).await;
            self.authoring.busy.set(false);
            match result {
                Ok(true) => {
                    self.profiles.instances.update(|instances| instances.retain(|key, _| key.path != path));
                    self.profiles.failures.update(|failures| failures.retain(|key, _| key.path != path));
                    for workspace in self.workspaces.items.get_untracked() {
                        if workspace.source.with_untracked(|source|source.as_ref().is_some_and(|source|source.path==path)){workspace.source.set(None);}
                    }
                    for view in self.all_views() {
                        if view.preset.with_untracked(|source| source.as_ref().is_some_and(|source| source.path == path)) { view.preset.set(None); }
                        for pane in view.panes.get_untracked() {
                            if pane.preset.with_untracked(|source| source.as_ref().is_some_and(|source| source.path == path)) { pane.preset.set(None); }
                        }
                    }
                    self.authoring.revision.update(|revision| *revision += 1);
                    self.status.set("Deleted profile file; loaded fields and plots kept in session".into());
                }
                Ok(false) => {}
                Err(error) => self.profiles.error.set(error),
            }
        });
    }
}
