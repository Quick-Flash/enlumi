use crate::{action_button::ActionButtonProps, preset_capture::CaptureKind, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn WorkspacePicker(workbench: Workbench) -> impl IntoView {
    crate::workspace_catalogue::watch(workbench);
    let open = RwSignal::new_local(false);
    let actions = RwSignal::new_local(false);
    let name = RwSignal::new_local(String::new());
    let available = Signal::derive_local(move || {
        workbench.workspaces.saved.with(|saved| {
            saved
                .iter()
                .filter(|saved| {
                    !workbench.workspaces.items.with(|items| {
                        items.iter().any(|item| {
                            item.source.with(|source| {
                                source.as_ref().is_some_and(|source| {
                                    source.path == saved.path && source.name == saved.name
                                })
                            })
                        })
                    })
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    view! {
        row(class="workspace-picker view-picker") {
            Popover(open=open) {
                PopoverTrigger(class="app-action workspace-select view-select",size=ButtonSize::Xs,variant=ButtonVariant::Ghost,a11y:label="Choose workspace") {
                    text(class="view-caption") {{move || {let workspace=workbench.workspace(); format!("Workspace: {}{}",workspace.name.get(),if workspace.loading.with(Option::is_some){" · loading"}else{""})}}}
                    vector(class="view-select-mark",prop:viewBox="0 0 12 12",prop:d="M2 4 L6 8 L10 4",a11y:hidden=true)
                }
                PopoverContent(class="view-list workspace-list") {
                    text(class="muted") {"Open workspaces"}
                    for workspace in move ||workbench.workspaces.items.get(),key=|workspace|workspace.id {
                        row(class="view-choice",class:view-current=move ||workbench.workspaces.active.get()==workspace.id) {
                            ActionButton(class="workspace-choice view-choice-select",on:click=move |_|{open.set(false);workbench.select_workspace(workspace.id);}) {{move ||format!("{}{}",workspace.name.get(),if workspace.loading.with(Option::is_some){" · loading"}else{""})}}
                            if move ||workbench.workspaces.active.get()==workspace.id {
                                vector(class="view-current-icon",prop:viewBox="0 0 12 12",prop:d="M2 6.2 L4.7 9 L10 3",a11y:label="Current workspace")
                            } else {
                                ActionButton(class="view-choice-remove workspace-close",a11y:label="Close workspace",on:click=move |_|workbench.close_workspace(workspace.id)) {vector(class="view-remove-icon",prop:viewBox="0 0 12 12",prop:d="M3 3 L9 9 M9 3 L3 9",a11y:hidden=true)}
                            }
                        }
                    }
                    if move ||!available.with(Vec::is_empty) {
                        text(class="workspace-saved-heading muted") {"Saved workspaces"}
                        for saved in move ||available.get(),key=|saved|(saved.path.clone(),saved.name.clone()) {
                            {{let saved=StoredValue::new_local(saved);view! {
                                ActionButton(class="workspace-saved-choice view-choice-select",disabled=Signal::derive_local(move ||workbench.loading.get()||workbench.profiles.busy.get()),on:click=move |_|{open.set(false);workbench.open_saved_workspace(saved.get_value());}) {{saved.with_value(|saved|format!("{} ({} views)",saved.name,saved.views))}}
                            }}}
                        }
                    }
                    ActionButton(class="view-new-choice workspace-new",on:click=move |_|{open.set(false);workbench.new_blank_workspace();}) {"+ New workspace"}
                }
            }
            Popover(open=actions,on_open_change=UnsyncCallback::new(move |is_open|if is_open{name.set(workbench.workspace().name.get_untracked());})) {
                PopoverTrigger(class="app-action view-menu workspace-menu",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label="Workspace actions") {vector(class="view-menu-icon",prop:viewBox="0 0 16 16",prop:d="M3 7 H4 V8 H3 Z M7.5 7 H8.5 V8 H7.5 Z M12 7 H13 V8 H12 Z",a11y:hidden=true)}
                PopoverContent(class="view-menu-content workspace-menu-content") {
                    text(class="heading") {"Workspace"}
                    Input(value=name,a11y:label="Workspace name")
                    ActionButton(class="workspace-rename",disabled=Signal::derive_local(move ||name.with(|name|name.trim().is_empty()||name.trim().len()>256)),on:click=move |_|{workbench.workspace().name.set(name.get_untracked().trim().to_owned());actions.set(false);}) {"Rename"}
                    ActionButton(class="workspace-save",disabled=Signal::derive_local(move ||workbench.log.with(Option::is_none)),on:click=move |_|{actions.set(false);workbench.save_preset(CaptureKind::Workspace);}) {"Save"}
                    ActionButton(class="workspace-save-as",disabled=Signal::derive_local(move ||workbench.log.with(Option::is_none)),on:click=move |_|{actions.set(false);workbench.save_preset_as(CaptureKind::Workspace);}) {"Save as..."}
                    ActionButton(class="workspace-duplicate",on:click=move |_|{actions.set(false);workbench.duplicate_workspace();}) {"Duplicate"}
                }
            }
        }
    }
}
