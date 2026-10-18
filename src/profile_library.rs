use crate::{
    action_button::ActionButtonProps, profile, profile_outputs::ProfileOutputsProps,
    state::Workbench,
};
use crate::{sidebar_help::SidebarHelpProps, sidebar_search::SidebarSearchProps};
use std::{collections::BTreeSet, path::PathBuf};
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui::prelude::*;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct ProfileFile {
    pub path: PathBuf,
    pub name: String,
    pub compatible: Result<(), String>,
    pub auto_apply: bool,
    pub matched: String,
    pub views: Vec<(String, usize)>,
    pub fields: Vec<(String, String)>,
    pub geometry: Vec<(String, String)>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum LibraryRow {
    Folder(PathBuf, usize),
    File(ProfileFile, usize),
}

fn refresh(workbench: Workbench, files: RwSignal<Vec<ProfileFile>, LocalStorage>) {
    workbench.profiles.scan.update(|token| *token += 1);
    let token = workbench.profiles.scan.get_untracked();
    let flight = workbench.profiles.flight.get_untracked();
    let log = workbench.log.get_untracked();
    let generation = workbench.generation.get_untracked();
    let requested = workbench.profiles.directory.get_untracked();
    let directory = requested.clone();
    workbench.task(async move {
        let result = blocking(move || {
            let root = directory.map_or_else(profile::directory, Ok)?;
            let root = match root.canonicalize() {
                Ok(root) => root,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => root,
                Err(error) => return Err(error.to_string()),
            };
            let choices = crate::profile_preferences::load()?;
            profile::discover_in(&root).map(|paths| {
                let files = paths
                    .into_iter()
                    .map(
                        |path| match profile::load(&path).map(|p| p.for_flight(flight)) {
                            Ok(profile) => ProfileFile {
                                compatible: log
                                    .as_ref()
                                    .ok_or("Open a log first".into())
                                    .and_then(|log| profile::prepare::requirements(&profile, log)),
                                matched: format!(
                                    "Flight {flight} · {}",
                                    profile
                                        .requires_settings
                                        .iter()
                                        .map(|condition| log
                                            .as_ref()
                                            .ok_or_else(|| "Open a log first".to_string())
                                            .and_then(|log| profile::resolve::record(
                                                log,
                                                &condition.setting
                                            ))
                                            .map(|(_, s)| format!("{} = {}", s.name, s.value))
                                            .unwrap_or_else(|e| e))
                                        .collect::<Vec<_>>()
                                        .join(" · ")
                                ),
                                name: profile.name,
                                auto_apply: choices
                                    .get(&path)
                                    .copied()
                                    .unwrap_or(profile.auto_apply),
                                fields: profile
                                    .signals
                                    .iter()
                                    .map(|s| (s.id.clone(), s.metadata.name.clone()))
                                    .chain(
                                        profile
                                            .filters
                                            .iter()
                                            .map(|f| (f.id.clone(), f.metadata.name.clone())),
                                    )
                                    .collect(),
                                geometry: profile
                                    .geometry
                                    .iter()
                                    .map(|g| (g.id.clone(), g.name.clone()))
                                    .collect(),
                                views: profile
                                    .views
                                    .into_iter()
                                    .map(|view| (view.name, view.fields.len()))
                                    .collect(),
                                path,
                            },
                            Err(error) => ProfileFile {
                                name: path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into(),
                                path,
                                compatible: Err(error),
                                auto_apply: false,
                                matched: String::new(),
                                views: Vec::new(),
                                fields: Vec::new(),
                                geometry: Vec::new(),
                            },
                        },
                    )
                    .collect();
                (root, files)
            })
        })
        .await;
        if workbench.profiles.scan.get_untracked() != token
            || workbench.profiles.flight.get_untracked() != flight
            || workbench.generation.get_untracked() != generation
            || workbench.profiles.directory.get_untracked() != requested
        {
            return;
        }
        match result {
            Ok((directory, paths)) => {
                if workbench.profiles.directory.get_untracked().as_ref() != Some(&directory) {
                    workbench.profiles.directory.set(Some(directory));
                }
                let _ = files.try_set(paths);
            }
            Err(error) => workbench.profiles.error.set(error),
        }
    });
}

#[component]
pub fn ProfileLibrary(workbench: Workbench) -> impl IntoView {
    install_stylesheet("enlumi-profiles", include_str!("profile.css"));
    let files = RwSignal::new_local(Vec::<ProfileFile>::new());
    let query = RwSignal::new_local(String::new());

    let warnings_open = RwSignal::new_local(false);
    let expanded = RwSignal::new_local(BTreeSet::<PathBuf>::new());
    let effect = RenderEffect::new(move |_| {
        workbench.log.track();
        workbench.profiles.directory.track();
        workbench.profiles.flight.track();
        refresh(workbench, files);
    });
    on_cleanup_local(move || drop(effect));
    let unavailable = Signal::derive_local(move || {
        workbench.loading.get()
            || workbench.derived_busy.get()
            || workbench.profiles.busy.get()
            || workbench.log.get().is_none()
    });
    let rows = Memo::new(move |_| {
        let mut rows = Vec::new();
        let mut folders = BTreeSet::new();
        let root = workbench.profiles.directory.get().unwrap_or_default();
        let query = query.get().to_lowercase();
        let expanded = expanded.get();
        let mut listed = files.get();
        for instance in workbench
            .profiles
            .instances
            .get()
            .values()
            .filter(|p| p.key.flight == workbench.profiles.flight.get())
        {
            if let Some(file) = listed.iter_mut().find(|f| f.path == instance.key.path) {
                file.name = instance.name.clone();
                file.views = instance
                    .definition
                    .views
                    .iter()
                    .map(|v| (v.name.clone(), v.fields.len()))
                    .collect();
                file.fields = instance
                    .definition
                    .signals
                    .iter()
                    .map(|s| (s.id.clone(), s.metadata.name.clone()))
                    .chain(
                        instance
                            .definition
                            .filters
                            .iter()
                            .map(|f| (f.id.clone(), f.metadata.name.clone())),
                    )
                    .collect();
                file.geometry = instance
                    .definition
                    .geometry
                    .iter()
                    .map(|g| (g.id.clone(), g.name.clone()))
                    .collect();
            } else {
                listed.push(ProfileFile {
                    path: instance.key.path.clone(),
                    name: instance.name.clone(),
                    compatible: Ok(()),
                    auto_apply: false,
                    matched: format!("Flight {}", instance.key.flight),
                    views: instance
                        .definition
                        .views
                        .iter()
                        .map(|v| (v.name.clone(), v.fields.len()))
                        .collect(),
                    fields: instance
                        .definition
                        .signals
                        .iter()
                        .map(|s| (s.id.clone(), s.metadata.name.clone()))
                        .chain(
                            instance
                                .definition
                                .filters
                                .iter()
                                .map(|f| (f.id.clone(), f.metadata.name.clone())),
                        )
                        .collect(),
                    geometry: instance
                        .definition
                        .geometry
                        .iter()
                        .map(|g| (g.id.clone(), g.name.clone()))
                        .collect(),
                });
            }
        }
        for file in listed {
            if !query.is_empty()
                && !file.name.to_lowercase().contains(&query)
                && !file.path.to_string_lossy().to_lowercase().contains(&query)
                && !file
                    .views
                    .iter()
                    .any(|(name, _)| name.to_lowercase().contains(&query))
                && !file
                    .fields
                    .iter()
                    .any(|(_, name)| name.to_lowercase().contains(&query))
                && !file
                    .geometry
                    .iter()
                    .any(|(_, name)| name.to_lowercase().contains(&query))
            {
                continue;
            }
            let imported = !file.path.starts_with(&root);
            let relative = file
                .path
                .strip_prefix(&root)
                .unwrap_or(std::path::Path::new("Imported"));
            let mut folder = root.clone();
            let mut visible = true;
            let mut depth = 0;
            if imported {
                let imported = PathBuf::from("Imported");
                if folders.insert(imported.clone()) {
                    rows.push(LibraryRow::Folder(imported.clone(), 0));
                }
                visible = !query.is_empty() || expanded.contains(&imported);
                depth = 1;
            } else if let Some(parent) = relative.parent() {
                for part in parent.components() {
                    folder.push(part);
                    if visible && folders.insert(folder.clone()) {
                        rows.push(LibraryRow::Folder(folder.clone(), depth));
                    }
                    visible &= !query.is_empty() || expanded.contains(&folder);
                    depth += 1;
                }
            }
            if visible {
                rows.push(LibraryRow::File(file, depth));
            }
        }
        rows
    });
    view! {
        column(class = "profile-library") {
            row(class = "profile-actions") {
                ActionButton(class = "profile-choose-directory", disabled = Signal::derive_local(move || workbench.profiles.busy.get() || workbench.loading.get()), on:click = move |_| {
                    let current = workbench.profiles.directory.get_untracked();
                    workbench.task(async move {
                        let selected = blocking(move || {
                            let picker = rfd::FileDialog::new();
                            let picker = if let Some(path) = current { picker.set_directory(path) } else { picker };
                            picker.pick_folder().map(|path| profile::save_directory(&path))
                        }).await;
                        match selected {
                            Some(Ok(path)) => { let _ = files.try_set(Vec::new()); let _ = expanded.try_set(BTreeSet::new()); workbench.profiles.directory.set(Some(path)); workbench.profiles.error.set(String::new()); }
                            Some(Err(error)) => workbench.profiles.error.set(error),
                            None => (),
                        }
                    });
                }) { "Folder…" }

                ActionButton(class = "profile-browse", disabled = unavailable, on:click = move |_| {
                    workbench.task(async move {
                        let path = blocking(|| rfd::FileDialog::new().add_filter("Enlumi profile", &["json", "JSON"]).pick_file()).await;
                        if let Some(path) = path {  workbench.apply_profile(path); }
                    });
                }) { "Load file…" }
                ActionButton(class = "profile-refresh", on:click = move |_| refresh(workbench, files)) { "Refresh" }
            }
            if move ||workbench.log.with(|log|log.as_ref().is_some_and(|log|log.settings.len()>1)) {
            row(class = "profile-actions") {
                text(class = "muted") { "Flight" }
                for flight in move || workbench.log.with(|log|log.as_ref().map(|log|log.settings.keys().copied().collect::<Vec<_>>()).unwrap_or_default()), key = |flight|*flight {
                    ActionButton(class:setting-active = move ||workbench.profiles.flight.get()==flight, disabled=unavailable, on:click=move |_|{workbench.profiles.flight.set(flight);workbench.apply_default_profiles();}) {{flight.to_string()}}
                }
            }
            }
            SidebarSearch(value = query, placeholder = "Search profiles, views and fields", label = "Search profiles") {
                SidebarHelp(label = "Profile library details and help") {
                    text(class = "heading") { "Profile library" }
            text(class = "muted") { "Drag outputs onto plots, or use Add / New. Auto uses local preferences." }
            text(class = "muted") {{move || workbench.log.with(|log| log.as_ref().map_or_else(String::new, |log| log.blackbox_headers.values().map(|header| header.firmware_revision.clone()).collect::<Vec<_>>().join(" · ")))}}
            text(class = "muted") {{move || workbench.profiles.directory.get().map_or_else(|| "Locating profile directory…".into(), |path| path.display().to_string())}}
                }
            }
            scroll(class = "profile-files", a11y:label = "Profile directory tree") {
                for row in move || rows.get(), key = |row| row.clone() {
                    {match row {
                        LibraryRow::Folder(path, depth) => {
                            let label = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                            let toggle = path.clone();
                            let arrow_path = path.clone();
                            AnyView::new(view! { control(class = "profile-folder tree-branch", style:padding-left = Some(format!("{}px", depth * 10)), tabindex = Focus::Sequential,
                                a11y:label = format!("Profile folder {label}"),
                                on:click = move |_| expanded.update(|expanded| { if !expanded.remove(&toggle) { expanded.insert(toggle.clone()); } }),
                                on:key_down = move |event| {
                                    if matches!(event.key, zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter | zgui::vocab::NamedKey::Space)) {
                                        expanded.update(|expanded| { if !expanded.remove(&path) { expanded.insert(path.clone()); } }); event.prevent_default();
                                    }
                                }) { text { {move || format!("{} {label}/", if expanded.with(|expanded| expanded.contains(&arrow_path)) { "▾" } else { "▸" })} } } })
                        },
                        LibraryRow::File(file, depth) => {
                            let toggle = file.path.clone();
                            let arrow_path = file.path.clone();
                            let title = file.name.clone();
                            let loaded = file.path.clone();
                            let auto = file.auto_apply;
                            let record=StoredValue::new_local(file.clone());
                            let actions_open=RwSignal::new_local(false);
                            let profile_key=Memo::new(move |_|profile::identity::ProfileKey::new(record.with_value(|f|f.path.clone()),workbench.profiles.flight.get()));
                            let compatible = file.compatible.is_ok();
                            let reason = StoredValue::new_local(file.compatible.clone().err().unwrap_or_default());
                            let path = file.path.clone();
                            AnyView::new(view! { column(class = "profile-tree-file", style:padding-left = Some(format!("{}px", depth * 10))) {
                                row(class = "profile-file") {
                                    ActionButton(class = "profile-file-name", on:click = move |_| expanded.update(|expanded| { if !expanded.remove(&toggle) { expanded.insert(toggle.clone()); } })) { {move || format!("{} {title}", if expanded.with(|expanded| expanded.contains(&arrow_path)) { "▾" } else { "▸" })} }
                                    ActionButton(disabled = Signal::derive_local(move || unavailable.get() || !compatible), on:click = move |_| { expanded.update(|paths| { paths.insert(loaded.clone()); }); workbench.apply_profile(loaded.clone()); }) { "Load" }
                                    Popover(open=actions_open) {
                                    PopoverTrigger(class="app-action profile-menu",size=ButtonSize::Xs,variant=ButtonVariant::Ghost,a11y:label="Profile actions") { "⋯" }
                                    PopoverContent(class="profile-menu-content") {
                                    if move ||!record.with_value(|file|workbench.profiles.directory.with(|root|root.as_ref().is_some_and(|root|file.path.starts_with(root)))) {
                                        text(class="muted") { "Move profile into library folder to enable Auto." }
                                    }
                                    ActionButton(class = "profile-auto", class:setting-active = move || auto, disabled = Signal::derive_local(move || workbench.loading.get() || workbench.profiles.busy.get() ||workbench.profiles.auto_saving.get() ||!record.with_value(|file|workbench.profiles.directory.with(|root|root.as_ref().is_some_and(|root|file.path.starts_with(root))))), a11y:label = "Toggle automatic selection for matching firmware", on:click = move |_| {
                                        workbench.profiles.auto_saving.set(true);
                                        let path = record.with_value(|f|f.path.clone());
                                        workbench.task(async move {
                                            match blocking(move || crate::profile_preferences::save_choice(&path, !auto)).await {
                                                Ok(_) => { refresh(workbench, files); workbench.status.set("Profile auto selection saved; used when opening a log".into()); }
                                                Err(error) => workbench.profiles.error.set(error),
                                            }
                                            workbench.profiles.auto_saving.set(false);
                                        });
                                    }) { {if auto { "Auto ✓" } else { "Auto" }} }
                                    ActionButton(class="profile-reload",disabled=Signal::derive_local(move ||unavailable.get()||!workbench.profiles.instances.with(|all|all.contains_key(&profile_key.get()))),on:click=move |_|{actions_open.set(false);workbench.reload_profile(profile_key.get_untracked());}) { "Reload" }
                                    ActionButton(class="profile-remove-open",disabled=Signal::derive_local(move ||unavailable.get()||!workbench.profiles.instances.with(|all|all.contains_key(&profile_key.get()))),on:click=move |_|{actions_open.set(false);workbench.profiles.removal.set(Some(profile_key.get_untracked()));}) { "Remove…" }
                                    }
                                    }
                                }
                                if move || !query.with(String::is_empty) || expanded.with(|expanded| expanded.contains(&path)) {
                                if move || !compatible { text(class = "profile-compatibility muted") { {reason.get_value()} } }
                                    ProfileOutputs(workbench=workbench, file=record,query=query.into())

                                }
                            } })
                        }
                    }}
                }
                if move || files.with(Vec::is_empty) { text(class = "muted") { "Add JSON profiles in this directory or its subdirectories, or load a file." } }
            }
            if move || workbench.profiles.removal.get().is_some() {
                column(class="profile-remove-preview") {
                    text(class="heading") { "Remove profile?" }
                    text(class="muted") {{move || workbench.profiles.removal.get().and_then(|key|workbench.profiles.instances.with(|all|all.get(&key).map(|p|format!("{}: {} fields, {} geometry, {} placed lines. Plots remain.",p.name,p.fields.len(),p.geometry.len(),p.line_ids().len())))).unwrap_or_default()}}
                    text(class="warnings") {{move ||workbench.profiles.removal.get().map(|key|workbench.profile_remove_blockers(&key).join("\n")).unwrap_or_default()}}
                    row {
                        ActionButton(disabled=Signal::derive_local(move ||unavailable.get()||workbench.profiles.removal.get().is_none_or(|key|!workbench.profile_remove_blockers(&key).is_empty())),on:click=move |_|{if let Some(key)=workbench.profiles.removal.get_untracked(){workbench.remove_profile(key);}}) { "Remove outputs" }
                        ActionButton(class="profile-remove-cancel",on:click=move |_|workbench.profiles.removal.set(None)) { "Cancel" }
                    }
                }
            }
            if move || workbench.profiles.busy.get() { text(a11y:role = Role::Status) { "Preparing profile…" } }
            text(class = "warnings", a11y:role = Role::Status) {{move || workbench.profiles.error.get()}}
            if move || !workbench.profiles.warnings.with(Vec::is_empty) {
                ActionButton(class = "profile-warnings-toggle", on:click = move |_| warnings_open.update(|open| *open = !*open)) {
                    {move || format!("{} Skipped items ({})", if warnings_open.get() { "▾" } else { "▸" }, workbench.profiles.warnings.with(Vec::len))}
                }
                if move || warnings_open.get() {
                scroll(class = "profile-warnings", a11y:label = "Skipped profile resources") {
                    text(class = "muted") {{move || workbench.profiles.warnings.with(|warnings| format!("{} skipped items:\n{}", warnings.len(), warnings.iter().take(50).cloned().collect::<Vec<_>>().join("\n")))}}
                }
                }
            }
        }
    }
}
