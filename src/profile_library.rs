use crate::{action_button::ActionButtonProps, profile, state::Workbench};
use std::path::PathBuf;
use zgui::prelude::*;

#[derive(Clone)]
struct ProfileFile {
    path: PathBuf,
    compatible: Result<(), String>,
    auto_apply: bool,
}

fn refresh(workbench: Workbench, files: RwSignal<Vec<ProfileFile>, LocalStorage>) {
    let log = workbench.log.get_untracked();
    let generation = workbench.generation.get_untracked();
    workbench.task(async move {
        let result = blocking(move || {
            profile::discover().map(|paths| {
                paths
                    .into_iter()
                    .map(|path| {
                        let loaded = profile::load(&path);
                        let auto_apply = loaded.as_ref().is_ok_and(|p| p.auto_apply);
                        let compatible = loaded.and_then(|profile| {
                            let log = log.as_ref().ok_or("Open a log first")?;
                            profile::prepare::requirements(&profile, log)
                        });
                        ProfileFile {
                            path,
                            compatible,
                            auto_apply,
                        }
                    })
                    .collect()
            })
        })
        .await;
        if workbench.generation.get_untracked() != generation {
            return;
        }
        match result {
            Ok(paths) => {
                let _ = files.try_set(paths);
            }
            Err(error) => workbench.profiles.error.set(error),
        }
    });
}

#[component]
pub fn ProfileLibrary(workbench: Workbench) -> impl IntoView {
    let files = RwSignal::new_local(Vec::<ProfileFile>::new());
    refresh(workbench, files);
    let unavailable = Signal::derive_local(move || {
        workbench.loading.get()
            || workbench.derived_busy.get()
            || workbench.profiles.busy.get()
            || workbench.log.get().is_none()
    });
    view! {
        column(class = "profile-library") {
            text(class = "muted") { "Place JSON profiles in this directory. List refreshes each time this window opens." }
            text(class = "muted") { "Default mappings: set auto_apply to true in a profile here. Log loading matches its recorded firmware/settings conditions. Reload log after changes." }
            text(class = "profile-directory") { {profile::directory().map_or_else(|error| error, |path| path.display().to_string())} }
            row(class = "profile-actions") {
                ActionButton(class = "profile-browse", disabled = unavailable, on:click = move |_| {
                    workbench.task(async move {
                        let path = blocking(|| rfd::FileDialog::new().add_filter("Enlumi profile", &["json", "JSON"]).pick_file()).await;
                        if let Some(path) = path { workbench.apply_profile(path); }
                    });
                }) { "Load file…" }
                ActionButton(class = "profile-refresh", on:click = move |_| refresh(workbench, files)) { "Refresh list" }

            }
            text(class = "muted") {{move || workbench.log.with(|log| log.as_ref().map_or_else(String::new, |log| log.blackbox_headers.iter().map(|(flight, header)| format!("Flight {flight}: {}", header.firmware_revision)).collect::<Vec<_>>().join(" · ")))}}
            text(class = "muted") {{move || format!("Destination: {}", workbench.profiles.titles.with(|titles| titles.get(&workbench.active.get()).cloned().unwrap_or_else(|| format!("Plot {}", workbench.active.get()))))}}
            scroll(class = "profile-files", a11y:label = "Available profiles") {
                for file in move || files.get(), key = |file| file.path.clone() {
                    row(class = "profile-file") {
                        column(class = "profile-file-info") {
                            text(class = "profile-file-name") { {file.path.file_name().unwrap_or_default().to_string_lossy().into_owned()} }
                            text(class = "profile-compatibility muted") { {file.compatible.as_ref().map_or_else(|error| error.clone(), |()| if file.auto_apply { "Requirements match · auto-apply enabled".into() } else { "Requirements match".into() })} }
                        }
                        ActionButton(disabled = Signal::derive_local({ let compatible = file.compatible.is_ok(); move || unavailable.get() || !compatible }), on:click = move |_| workbench.apply_profile(file.path.clone())) { "Apply" }
                    }
                }
                if move || files.with(Vec::is_empty) { text(class = "muted") { "No profiles found. Load a file or add JSON files to the directory above." } }
            }
            text(class = "heading") { "Plot views" }
            scroll(class = "profile-views", a11y:label = "Profile plot views") {
                for entry in move || workbench.profiles.views.with(|views| views.iter().enumerate().map(|(index, view)| (index, view.name.clone())).collect::<Vec<_>>()), key = |view| view.0 {
                    row(class = "profile-file") {
                        text(class = "profile-file-name") { {entry.1.clone()} }
                        ActionButton(class = "profile-view-new", disabled = unavailable, on:click = move |_| workbench.add_profile_view(entry.0, true)) { "New panel" }
                        ActionButton(class = "profile-view-add", disabled = unavailable, on:click = move |_| workbench.add_profile_view(entry.0, false)) { "Add to active plot" }
                    }
                }
                if move || workbench.profiles.views.with(Vec::is_empty) { text(class = "muted") { "Load a profile to make its plot views available." } }
            }
            if move || workbench.profiles.busy.get() { text(a11y:role = Role::Status) { "Preparing profile…" } }
            text(class = "warnings", a11y:role = Role::Status) {{move || workbench.profiles.error.get()}}
            text(class = "muted") { "Loading adds fields and geometry. Choose where to place each view. Validation or filter errors leave current work intact. Reload log to start fresh." }
        }
    }
}
