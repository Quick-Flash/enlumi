use crate::{action_button::ActionButtonProps, profile::Metadata, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FieldLabel(workbench: Workbench) -> impl IntoView {
    let open = RwSignal::new_local(false);
    let details = RwSignal::new_local(false);
    let source = StoredValue::new_local(String::new());
    let scope = RwSignal::new_local(String::new());
    let name = RwSignal::new_local(String::new());
    let axis = RwSignal::new_local(String::new());
    let unit = RwSignal::new_local(String::new());
    let selected = Memo::new(move |_| {
        workbench
            .selected_field
            .get()
            .and_then(|source| workbench.editable_source(&source))
    });
    let owners = Memo::new(move |_| {
        let Some(id) = selected.get() else {
            return Vec::new();
        };
        workbench.profiles.instances.with(|instances| {
            instances
                .values()
                .filter(|instance| {
                    instance.metadata.contains_key(&id)
                        || instance.symbols.values().any(|source| source == &id)
                })
                .map(|instance| instance.key.path.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
    });
    let selection_sync = zgui::reactive::RenderEffect::new(move |_| {
        let selected = selected.get();
        if let Some(request) = workbench.authoring.field_edit.get() {
            let metadata = workbench
                .profiles
                .metadata
                .with_untracked(|metadata| metadata.get(&request).cloned())
                .unwrap_or_else(|| Metadata {
                    name: workbench.field_name_untracked(&request),
                    ..Default::default()
                });
            source.set_value(request);
            name.set(metadata.name);
            axis.set(metadata.axis);
            unit.set(metadata.unit);
            scope.set(String::new());
            details.set(false);
            open.set(true);
            workbench.authoring.field_edit.set(None);
        } else if open.get_untracked() && selected.as_ref() != Some(&source.get_value()) {
            open.set(false);
        }
    });
    on_cleanup_local(move || drop(selection_sync));
    let refresh_lines = move |id: &str| {
        for pane in workbench.all_panes() {
            pane.lines.update(|lines| {
                for line in lines {
                    if line.source == id {
                        line.name = workbench.field_name(id);
                    }
                }
            });
        }
    };
    let save = move || {
        let id = source.get_value();
        let name = name.get_untracked().trim().to_owned();
        let axis = axis.get_untracked().trim().to_owned();
        let unit = unit.get_untracked().trim().to_owned();
        if name.is_empty() || name.len() > 256 || axis.len() > 64 || unit.len() > 64 {
            workbench
                .status
                .set("Use name up to 256 bytes; component and unit up to 64 bytes".into());
            return;
        }
        if !scope.get_untracked().is_empty() {
            workbench.save_field_to_profile(
                scope.get_untracked().into(),
                id,
                Metadata { name, axis, unit },
            );
            open.set(false);
            return;
        }
        workbench.profiles.overrides.update(|metadata| {
            metadata.insert(id.clone(), Some(Metadata { name, axis, unit }));
        });
        workbench.rebuild_profile_metadata();
        refresh_lines(&id);
        open.set(false);
    };
    view! {
            Popover(open = open, on_open_change = UnsyncCallback::new(move |is_open| {
                if is_open && let Some(id) = selected.get_untracked() {
                    let metadata = workbench.profiles.metadata.with_untracked(|metadata| metadata.get(&id).cloned()).unwrap_or_else(|| Metadata {
                        name: id.rsplit('/').next().unwrap_or(&id).into(), ..Metadata::default()
                    });
                    source.set_value(id); name.set(metadata.name); axis.set(metadata.axis); unit.set(metadata.unit);scope.set(String::new());details.set(false);
                }
            })) {
                PopoverTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "app-action field-label-trigger",{..Attrs::new().state(zgui::vocab::UiState::DISABLED,move ||selected.get().is_none()).a11y_from(A11yBinding::new(Role::Button).disabled(move ||selected.get().is_none()))}) { "Labels…" }
                PopoverContent(class = "field-label-editor", on:key_down = move |event| {
                    if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter) { save(); event.prevent_default(); event.stop_propagation(); }
                }) {
                    text(class = "heading") { "Signal metadata" }
                    text(class = "muted") {{move || source.get_value()}}
                    text(class = "muted") {{move || {
                        let id=source.get_value();
                        let origin=workbench.profiles.instances.with(|all|all.values().filter(|p|p.metadata.contains_key(&id)).map(|p|format!("{} · flight {}",p.name,p.key.flight)).collect::<Vec<_>>().join(", "));
                        let override_label=if workbench.profiles.overrides.with(|all|all.contains_key(&id)){"Manual override"}else{"Profile default"};
                        format!("{override_label} · {}",if origin.is_empty(){"Raw field".into()}else{origin})
                    }}}
                    text(class = "muted") { "Display name (plots and legends)" }
                    Input(value = name, a11y:label = "Field display name")
                    text(class = "muted") { "Component (optional short name in field tree)" }
                    Input(value = axis, placeholder = "e.g. Roll", a11y:label = "Field component label")
                    text(class = "muted") { "Unit label" }
                    Input(value = unit, a11y:label = "Field unit label")
                    text(class="muted") {"Save labels to"}
                    if move ||!owners.with(Vec::is_empty) {
                    Select(value=scope) {SelectTrigger(label="Label save scope") {SelectValue()}
                        SelectContent {SelectItem(value="") {"Current log"}
                            for path in move ||owners.get(),key=|path|path.clone() {
                                SelectItem(value=path.display().to_string()) {{path.file_name().unwrap_or_default().to_string_lossy().into_owned()}}
                            }
                        }
                    }
                    } else { text(class="muted") {"Current log"} }
                    text(class="muted") {"Labels change names and units, never values or plot bounds."}
                    ActionButton(class="field-label-details",{..Attrs::new().a11y_from(A11yBinding::new(Role::Button).expanded(move ||details.get()))},on:click=move |_|details.update(|open|*open = !*open)) {"Source details"}
                    if move ||details.get() {
                        scroll(class="field-label-provenance",a11y:label="Signal sources and filter provenance") {
                            text(class="muted") {{move ||workbench.signal_provenance(&source.get_value())}}
                        }
                    }
                    row { ActionButton(on:click = move |_| save()) { "Save" }
                        ActionButton(on:click = move |_| {
                            let id = source.get_value(); workbench.profiles.overrides.update(|metadata| { metadata.remove(&id); }); workbench.rebuild_profile_metadata(); refresh_lines(&id); open.set(false);
                        }) { "Profile default" }
                        ActionButton(on:click = move |_| {
                            let id=source.get_value();workbench.profiles.overrides.update(|all|{all.insert(id.clone(),None);});workbench.rebuild_profile_metadata();refresh_lines(&id);open.set(false);
                        }) { "Raw label" }
                    }
                }
            }
    }
}
