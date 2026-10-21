use crate::{
    action_button::ActionButtonProps,
    preset_ui::ApplyAction,
    profile::identity::{Drag, Item, ProfileKey},
    profile_library::ProfileFile,
    profile_output_row::ProfileOutputRowProps,
    profile_view_source::ProfileViewSourceProps,
    state::Workbench,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn WorkspacePreset(
    workbench: Workbench,
    file: StoredValue<ProfileFile, LocalStorage>,
    name: String,
    #[prop(optional)] source_name: Option<String>,
) -> impl IntoView {
    let label = name.clone();
    let name = StoredValue::new_local(source_name.unwrap_or(name));
    let expanded = RwSignal::new_local(false);
    let plots_open = RwSignal::new_local(true);
    let actions = RwSignal::new_local(false);
    let key = Memo::new(move |_| {
        ProfileKey::new(
            file.with_value(|file| file.path.clone()),
            workbench.profiles.flight.get(),
        )
    });
    let enabled = Signal::derive_local(move || {
        file.with_value(|file| file.compatible.is_ok())
            && !workbench.loading.get()
            && !workbench.profiles.busy.get()
            && !workbench.derived_busy.get()
            && workbench.log.with(Option::is_some)
    });
    let plots = file.with_value(|file| {
        file.workspace_plots
            .iter()
            .find(|(view, _)| view == &name.get_value())
            .map(|(_, plots)| plots.clone())
            .unwrap_or_default()
    });
    let plots = StoredValue::new_local(plots);
    view! {
        column(class="workspace-preset") {
            row(class="workspace-preset-heading") {
                ActionButton(icon=true,class="profile-members-toggle",a11y:label="Show plots and fields in view preset",{..Attrs::new().a11y_from(A11yBinding::new(Role::Button).expanded(move ||expanded.get()))},on:click=move |_|{
                    expanded.update(|open|*open = !*open);
                    if expanded.get_untracked() && enabled.get_untracked() && workbench.profiles.instances.with_untracked(|instances|!instances.contains_key(&key.get_untracked())) {
                        workbench.apply_profile(file.with_value(|file|file.path.clone()));
                    }
                }) {vector(class="profile-tree-arrow",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if expanded.get(){"M2 4 L6 8 L10 4"}else{"M4 2 L8 6 L4 10"}),a11y:hidden=true)}
                ProfileViewSource(workbench=workbench,source=Signal::derive_local(move ||Drag{profile:key.get(),item:Item::Workspace(name.get_value())}.source()),name=label,count=0,enabled=enabled)
                text(class="preset-kind") {{format!("{} plots",plots.with_value(Vec::len))}}
                Popover(open=actions) {
                    PopoverTrigger(class="app-action profile-output-more",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label="View preset actions") {vector(class="profile-more-icon",prop:viewBox="0 0 16 16",prop:d="M3 7 H4 V8 H3 Z M7.5 7 H8.5 V8 H7.5 Z M12 7 H13 V8 H12 Z",a11y:hidden=true)}
                    PopoverContent(class="profile-menu-content") {
                        text(class="muted") {{file.with_value(|file|format!("File: {}",file.path.file_name().unwrap_or_default().to_string_lossy()))}}
                        ActionButton(class="preset-edit-profile",disabled=Signal::derive_local(move ||workbench.authoring.busy.get()),on:click=move |_|{actions.set(false);workbench.edit_profile_document(file.with_value(|file|file.path.clone()));}) {"Edit profile…"}
                        ActionButton(disabled=Signal::derive_local(move ||!enabled.get()),on:click=move |_|{actions.set(false);workbench.use_saved_preset(key.get_untracked(),name.get_value(),ApplyAction::NewView);}) {"Open as new view"}
                        ActionButton(disabled=Signal::derive_local(move ||!enabled.get()),on:click=move |_|{actions.set(false);workbench.use_saved_preset(key.get_untracked(),name.get_value(),ApplyAction::AppendView);}) {"Add plots to current view"}
                        if move ||!name.with_value(|name|name.starts_with("@workspace-view/")) {
                            ActionButton(class="preset-delete",disabled=Signal::derive_local(move ||workbench.authoring.busy.get()||workbench.profiles.busy.get()),on:click=move |_|{actions.set(false);workbench.delete_saved_preset(Drag{profile:key.get_untracked(),item:Item::Workspace(name.get_value())});}) {"Delete view preset…"}
                        }
                    }
                }
            }
            if move ||expanded.get() {
                column(class="workspace-preset-plots") {
                    ActionButton(class="profile-output-section workspace-plots-toggle",{..Attrs::new().a11y_from(A11yBinding::new(Role::Button).label("Plots in saved view").expanded(move ||plots_open.get()))},on:click=move |_|plots_open.update(|open|*open = !*open)) {
                        vector(class="profile-tree-arrow",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if plots_open.get(){"M2 4 L6 8 L10 4"}else{"M4 2 L8 6 L4 10"}),a11y:hidden=true)
                        text {{format!("Plots ({})",plots.with_value(Vec::len))}}
                    }
                    if move ||plots_open.get() {
                    for plot in move ||plots.get_value().into_iter().enumerate().collect::<Vec<_>>(),key=|plot|plot.0 {
                        {{let index=plot.0;let count=plot.1.1;let label=plot.1.0;view! {
                            column(class="workspace-preset-plot") {
                                ProfileOutputRow(workbench=workbench,file=file,drag=Signal::derive_local(move ||Drag{profile:key.get(),item:Item::View(crate::preset_lookup::plot_key(&name.get_value(),index))}),name=label,count=count)
                            }
                        }}}
                    }
                    }
                }
            }
        }
    }
}
