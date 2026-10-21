use crate::{
    action_button::ActionButtonProps,
    profile::identity::{Drag, Item, ProfileKey},
    profile_library::ProfileFile,
    profile_view_source::ProfileViewSourceProps,
    state::Workbench,
    workspace_preset::WorkspacePresetProps,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn WorkspaceCollection(
    workbench: Workbench,
    file: StoredValue<ProfileFile, LocalStorage>,
    name: String,
) -> impl IntoView {
    let name = StoredValue::new_local(name);
    let expanded = RwSignal::new_local(false);
    let menu = RwSignal::new_local(false);
    let views = StoredValue::new_local(file.with_value(|file| {
        file.collections
            .iter()
            .find(|(workspace, _)| workspace == &name.get_value())
            .map(|(_, views)| views.clone())
            .unwrap_or_default()
    }));
    let key = Memo::new(move |_| {
        ProfileKey::new(
            file.with_value(|file| file.path.clone()),
            workbench.profiles.flight.get(),
        )
    });
    view! {column(class="workspace-collection") {
        row(class="workspace-preset-heading") {
            ActionButton(icon=true,class="profile-members-toggle",a11y:label="Show views in workspace",{..Attrs::new().a11y_from(A11yBinding::new(Role::Button).expanded(move ||expanded.get()))},on:click=move |_|expanded.update(|open|*open = !*open)) {vector(class="profile-tree-arrow",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if expanded.get(){"M2 4 L6 8 L10 4"}else{"M4 2 L8 6 L4 10"}),a11y:hidden=true)}
            ProfileViewSource(workbench=workbench,source=Signal::derive_local(move ||Drag{profile:key.get(),item:Item::WorkspacePreset(name.get_value())}.source()),name=name.get_value(),count=0,enabled=Signal::derive_local(move ||workbench.log.with(Option::is_some)&&!workbench.loading.get()&&!workbench.profiles.busy.get()))
            text(class="preset-kind") {{format!("{} views",views.with_value(Vec::len))}}
            Popover(open=menu) {
                PopoverTrigger(class="app-action profile-output-more",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label="Saved workspace actions") {vector(class="profile-more-icon",prop:viewBox="0 0 16 16",prop:d="M3 7 H4 V8 H3 Z M7.5 7 H8.5 V8 H7.5 Z M12 7 H13 V8 H12 Z",a11y:hidden=true)}
                PopoverContent(class="profile-menu-content") {
                    ActionButton(class="workspace-preset-open",disabled=Signal::derive_local(move ||workbench.loading.get()||workbench.profiles.busy.get()),on:click=move |_|{
                        menu.set(false);
                        if let Some(saved)=workbench.workspaces.saved.with_untracked(|saved|saved.iter().find(|saved|saved.path==key.get_untracked().path&&saved.name==name.get_value()).cloned()) {workbench.open_saved_workspace(saved);}
                        else {workbench.use_saved_preset(key.get_untracked(),name.get_value(),crate::preset_ui::ApplyAction::Workspace);}
                    }) {"Open workspace"}
                    ActionButton(class="preset-edit-profile",on:click=move |_|{menu.set(false);workbench.edit_profile_document(file.with_value(|file|file.path.clone()));}) {"Edit profile…"}
                    ActionButton(class="preset-delete",disabled=Signal::derive_local(move ||workbench.authoring.busy.get()||workbench.profiles.busy.get()),on:click=move |_|{menu.set(false);workbench.delete_saved_preset(Drag{profile:key.get_untracked(),item:Item::WorkspacePreset(name.get_value())});}) {"Delete workspace preset…"}
                }
            }
        }
        if move ||expanded.get() {
            column(class="workspace-collection-views") {
                text(class="profile-output-section") {{format!("Views ({})",views.with_value(Vec::len))}}
                for view in move ||views.get_value().into_iter().enumerate().collect::<Vec<_>>(),key=|view|view.0 {
                    {{let source=crate::preset_lookup::view_key(&name.get_value(),view.0);let label=view.1.0;let mut nested=file.get_value();nested.workspace_plots.push((source.clone(),view.1.1));let nested=StoredValue::new_local(nested);view! {
                        WorkspacePreset(workbench=workbench,file=nested,name=label,source_name=source)
                    }}}
                }
            }
        }
    }}
}
