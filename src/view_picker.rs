use crate::{action_button::ActionButtonProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn ViewPicker(workbench: Workbench) -> impl IntoView {
    let open = RwSignal::new_local(false);
    let menu_open = RwSignal::new_local(false);
    let name = RwSignal::new_local(String::new());
    view! {
        row(class="view-picker") {
            Popover(open=open) {
                PopoverTrigger(class="app-action view-select",size=ButtonSize::Xs,variant=ButtonVariant::Ghost,a11y:label="Choose workspace view") {
                    text(class="view-caption") {{move ||format!("View: {}",workbench.view().name.get())}}
                    vector(class="view-select-mark",prop:viewBox="0 0 12 12",prop:d="M2 4 L6 8 L10 4",a11y:hidden=true)
                }
                PopoverContent(class="view-list") {
                    for view in move ||workbench.views.items.get(),key=|view|view.id {
                        row(class="view-choice",class:view-current=move ||workbench.views.active.get()==view.id) {
                            ActionButton(class="view-choice-select",on:click=move |_|{workbench.switch_view(view.id);open.set(false);}) {{move ||view.name.get()}}
                            if move ||workbench.views.active.get()==view.id {
                                vector(class="view-current-icon",prop:viewBox="0 0 12 12",prop:d="M2 6.2 L4.7 9 L10 3",a11y:label="Current view")
                            } else {
                            ActionButton(class="view-choice-remove",disabled=Signal::derive_local(move ||workbench.views.items.with(|views|views.len()<2)),a11y:label=format!("Remove workspace view {}",view.name.get_untracked()),on:click=move |_|{workbench.close_view(view.id);if workbench.views.items.with_untracked(|views|views.len())<=1{open.set(false);}}) {vector(class="view-remove-icon",prop:viewBox="0 0 12 12",prop:d="M3 3 L9 9 M9 3 L3 9",a11y:hidden=true)}
                            }
                        }
                    }
                    ActionButton(class="view-new-choice",on:click=move |_|{workbench.new_view();open.set(false);}) {"+ New blank view"}
                }
            }
            Popover(open=menu_open,on_open_change=UnsyncCallback::new(move |is_open|if is_open {name.set(workbench.bind_view().view().name.get_untracked());})) {
                PopoverTrigger(class="app-action view-menu",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label="Workspace view actions") {vector(class="view-menu-icon",prop:viewBox="0 0 12 12",prop:d="M2 6 L2.1 6 M6 6 L6.1 6 M10 6 L10.1 6",a11y:hidden=true)}
                PopoverContent(class="view-menu-content") {
                    text(class="heading") {"View"}
                    Input(value=name,a11y:label="View name",class="view-name")
                    ActionButton(class="view-rename",disabled=Signal::derive_local(move ||name.with(|name|name.trim().is_empty()||name.trim().len()>256)),on:click=move |_|{workbench.rename_view(name.get_untracked());menu_open.set(false);}) {"Rename"}
                    ActionButton(class="view-save-preset",on:click=move |_|{menu_open.set(false);workbench.save_preset(crate::preset_capture::CaptureKind::View);}) {"Save"}
                    ActionButton(class="view-save-as",on:click=move |_|{menu_open.set(false);workbench.save_preset_as(crate::preset_capture::CaptureKind::View);}) {"Save as..."}
                    ActionButton(class="view-duplicate",on:click=move |_|{menu_open.set(false);workbench.duplicate_view();}) {"Duplicate"}
                }
            }
        }
    }
}
