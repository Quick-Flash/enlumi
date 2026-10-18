use crate::{action_button::ActionButtonProps, profile::Metadata, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FieldLabel(workbench: Workbench) -> impl IntoView {
    let open = RwSignal::new_local(false);
    let source = StoredValue::new_local(String::new());
    let name = RwSignal::new_local(String::new());
    let axis = RwSignal::new_local(String::new());
    let unit = RwSignal::new_local(String::new());
    let selected = Memo::new(move |_| {
        workbench.selected_field.get().filter(|id| {
            id.starts_with("@derived/")
                || (!id.starts_with('@')
                    && workbench.log.with(|log| {
                        log.as_ref()
                            .is_some_and(|log| log.catalogue.iter().any(|field| field.id == *id))
                    }))
        })
    });
    let refresh_lines = move |id: &str| {
        for pane in workbench.panes.get_untracked() {
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
        workbench.profiles.overrides.update(|metadata| {
            metadata.insert(id.clone(), Some(Metadata { name, axis, unit }));
        });
        workbench.rebuild_profile_metadata();
        refresh_lines(&id);
        open.set(false);
    };
    view! {
        if move || selected.get().is_some() {
            Popover(open = open, on_open_change = UnsyncCallback::new(move |is_open| {
                if is_open && let Some(id) = selected.get_untracked() {
                    let metadata = workbench.profiles.metadata.with_untracked(|metadata| metadata.get(&id).cloned()).unwrap_or_else(|| Metadata {
                        name: id.rsplit('/').next().unwrap_or(&id).into(), ..Metadata::default()
                    });
                    source.set_value(id); name.set(metadata.name); axis.set(metadata.axis); unit.set(metadata.unit);
                }
            })) {
                PopoverTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "app-action field-label-trigger") { "Labels…" }
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
                    text(class = "muted") {{move || workbench.signal_provenance(&source.get_value())}}
                    text(class = "muted") { "Display name (plots and legends)" }
                    Input(value = name, a11y:label = "Field display name")
                    text(class = "muted") { "Axis / component (field tree)" }
                    Input(value = axis, placeholder = "e.g. Roll", a11y:label = "Field component label")
                    text(class = "muted") { "Unit label" }
                    Input(value = unit, a11y:label = "Field unit label")
                    text(class = "muted") { "Labels only; values stay unchanged. Saved for this open log." }
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
}
