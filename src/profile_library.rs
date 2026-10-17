use crate::{
    action_button::ActionButtonProps, profile, profile_view_source::ProfileViewSourceProps,
    state::Workbench,
};
use std::{collections::BTreeSet, path::PathBuf};
use zgui::prelude::*;
use zgui::reactive::RenderEffect;
use zgui_ui::prelude::*;

#[derive(Clone, PartialEq, Eq, Hash)]
struct ProfileFile {
    path: PathBuf,
    name: String,
    compatible: Result<(), String>,
    auto_apply: bool,
    views: Vec<(String, usize)>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum LibraryRow {
    Folder(PathBuf, usize),
    File(ProfileFile, usize),
}

fn refresh(workbench: Workbench, files: RwSignal<Vec<ProfileFile>, LocalStorage>) {
    let log = workbench.log.get_untracked();
    let generation = workbench.generation.get_untracked();
    let requested = workbench.profiles.directory.get_untracked();
    let directory = requested.clone();
    workbench.task(async move {
        let result = blocking(move || {
            let root = directory.map_or_else(profile::directory, Ok)?;
            profile::discover_in(&root).map(|paths| {
                let files = paths
                    .into_iter()
                    .map(|path| match profile::load(&path) {
                        Ok(profile) => ProfileFile {
                            compatible: log
                                .as_ref()
                                .ok_or("Open a log first".into())
                                .and_then(|log| profile::prepare::requirements(&profile, log)),
                            name: profile.name,
                            auto_apply: profile.auto_apply,
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
                            views: Vec::new(),
                        },
                    })
                    .collect();
                (root, files)
            })
        })
        .await;
        if workbench.generation.get_untracked() != generation
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
    let loaded_open = RwSignal::new_local(false);
    let warnings_open = RwSignal::new_local(false);
    let expanded = RwSignal::new_local(BTreeSet::<PathBuf>::new());
    let effect = RenderEffect::new(move |_| {
        workbench.log.track();
        workbench.profiles.directory.track();
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
        for file in files.get() {
            if !query.is_empty()
                && !file.name.to_lowercase().contains(&query)
                && !file.path.to_string_lossy().to_lowercase().contains(&query)
                && !file
                    .views
                    .iter()
                    .any(|(name, _)| name.to_lowercase().contains(&query))
            {
                continue;
            }
            let relative = file.path.strip_prefix(&root).unwrap_or(&file.path);
            let mut folder = root.clone();
            let mut visible = true;
            let mut depth = 0;
            if let Some(parent) = relative.parent() {
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
                        if let Some(path) = path { let _ = loaded_open.try_set(true); workbench.apply_profile(path); }
                    });
                }) { "Load file…" }
                ActionButton(class = "profile-refresh", on:click = move |_| refresh(workbench, files)) { "Refresh" }
            }
            Input(value = query, placeholder = "Search profiles and views", a11y:label = "Search profiles")
            text(class = "muted") { "Drag views onto plots. Auto matches firmware when logs open." }
            text(class = "muted") {{move || workbench.log.with(|log| log.as_ref().map_or_else(String::new, |log| log.blackbox_headers.values().map(|header| header.firmware_revision.clone()).collect::<Vec<_>>().join(" · ")))}}
            Input(class = "profile-directory", read_only = true, a11y:label = "Profile directory",
                value = Binding::controlled(Signal::derive_local(move || workbench.profiles.directory.get().map_or_else(|| "Locating profile directory…".into(), |path| path.display().to_string())), |_| {}))
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
                            let auto_path = file.path.clone();
                            let auto = file.auto_apply;
                            let compatible = file.compatible.is_ok();
                            let reason = StoredValue::new_local(file.compatible.clone().err().unwrap_or_default());
                            let path = file.path.clone();
                            let views = StoredValue::new_local(file.views.clone());
                            let view_path = StoredValue::new_local(file.path.clone());
                            AnyView::new(view! { column(class = "profile-tree-file", style:padding-left = Some(format!("{}px", depth * 10))) {
                                row(class = "profile-file") {
                                    ActionButton(class = "profile-file-name", on:click = move |_| expanded.update(|expanded| { if !expanded.remove(&toggle) { expanded.insert(toggle.clone()); } })) { {move || format!("{} {title}", if expanded.with(|expanded| expanded.contains(&arrow_path)) { "▾" } else { "▸" })} }
                                    ActionButton(disabled = Signal::derive_local(move || unavailable.get() || !compatible), on:click = move |_| { expanded.update(|paths| { paths.insert(loaded.clone()); }); workbench.apply_profile(loaded.clone()); }) { "Load" }
                                    ActionButton(class = "profile-auto", class:setting-active = move || auto, disabled = Signal::derive_local(move || workbench.loading.get() || workbench.profiles.busy.get()), a11y:label = "Toggle automatic selection for matching firmware", on:click = move |_| {
                                        let path = auto_path.clone();
                                        workbench.task(async move {
                                            match blocking(move || profile::set_auto_apply(&path, !auto)).await {
                                                Ok(()) => { refresh(workbench, files); workbench.status.set("Profile auto selection saved; used when opening a log".into()); }
                                                Err(error) => workbench.profiles.error.set(error),
                                            }
                                        });
                                    }) { {if auto { "Auto ✓" } else { "Auto" }} }
                                }
                                if move || !query.with(String::is_empty) || expanded.with(|expanded| expanded.contains(&path)) {
                                if move || !compatible { text(class = "profile-compatibility muted") { {reason.get_value()} } }
                                    for entry in move || views.get_value(), key = |entry| entry.0.clone() {
                                        ProfileViewSource(workbench = workbench, source = format!("@profile-file-view/{}", serde_json::to_string(&(view_path.get_value(), entry.0.clone())).unwrap()),
                                            name = entry.0, count = entry.1, enabled = compatible)
                                    }
                                }
                            } })
                        }
                    }}
                }
                if move || files.with(Vec::is_empty) { text(class = "muted") { "Add JSON profiles in this directory or its subdirectories, or load a file." } }
            }
            if move || !workbench.profiles.views.with(Vec::is_empty) {
            ActionButton(class = "profile-loaded-toggle", on:click = move |_| loaded_open.update(|open| *open = !*open)) {
                {move || format!("{} Loaded views ({})", if loaded_open.get() { "▾" } else { "▸" }, workbench.profiles.views.with(Vec::len))}
            }
            if move || loaded_open.get() {
            scroll(class = "profile-views", a11y:label = "Loaded profile plot views") {
                for entry in move || workbench.profiles.views.with(|views| views.iter().enumerate().map(|(index, view)| (index, view.name.clone(), view.fields.len())).collect::<Vec<_>>()), key = |view| view.0 {
                    column(class = "profile-loaded-view") {
                        ProfileViewSource(workbench = workbench, source = format!("@profile-view/{}", entry.0), name = entry.1, count = entry.2, enabled = true)
                        row(class = "profile-view-actions") {
                            ActionButton(class = "profile-view-new", disabled = unavailable, on:click = move |_| workbench.add_profile_view(entry.0, true)) { "New panel" }
                            ActionButton(class = "profile-view-add", disabled = unavailable, on:click = move |_| workbench.add_profile_view(entry.0, false)) { "Add to active" }
                        }
                    }
                }
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
