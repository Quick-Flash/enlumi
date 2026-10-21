use crate::{
    action_button::ActionButtonProps,
    profile::identity::{Drag, Item, ProfileKey},
    profile_library::ProfileFile,
    profile_output_row::ProfileOutputRowProps,
    state::Workbench,
    workspace_collection::WorkspaceCollectionProps,
    workspace_preset::WorkspacePresetProps,
};
use zgui::prelude::*;

#[component]
pub fn ProfilePresetTree(
    workbench: Workbench,
    files: Signal<Vec<ProfileFile>>,
    query: Signal<String, LocalStorage>,
) -> impl IntoView {
    view! {
        column(class="profile-preset-tree") {
            for group in move ||["Workspaces", "Views", "Plots", "Fields", "Geometry"],key=|group|*group {
                {{let open=RwSignal::new_local(matches!(group,"Workspaces"|"Views"|"Plots"));
                let entries=Memo::new(move |_| {
                    let query=query.get().trim().to_lowercase();
                    let mut entries=Vec::new();
                    for file in files.get() {
                        let all=group.to_lowercase().contains(&query)||file.name.to_lowercase().contains(&query)||file.path.to_string_lossy().to_lowercase().contains(&query);
                        let items:Vec<_>=match group {
                            "Workspaces"=>file.collections.iter().map(|(name,views)|(Item::WorkspacePreset(name.clone()),name.clone(),views.len())).collect(),
                            "Views"=>file.workspaces.iter().map(|name|(Item::Workspace(name.clone()),name.clone(),0)).collect(),
                            "Plots"=>file.views.iter().map(|(name,count)|(Item::View(name.clone()),name.clone(),*count)).chain(file.workspace_plots.iter().flat_map(|(view,plots)|plots.iter().enumerate().map(move |(index,(name,count))|(Item::View(format!("@workspace/{view}/{index}")),format!("{view} / {name}"),*count)))).collect(),
                            "Fields"=>file.fields.iter().map(|(id,name)|(Item::Field(format!("${id}")),name.clone(),0)).collect(),
                            _=>file.geometry.iter().map(|(id,name)|(Item::Geometry(id.clone()),name.clone(),0)).collect(),
                        };
                        for (item,name,count) in items {
                            let field_match=matches!(&item,Item::Field(token) if workbench.profile_field_matches(&ProfileKey::new(file.path.clone(),workbench.profiles.flight.get()),token,&query));
                            if all||field_match||name.to_lowercase().contains(&query)||item.name().to_lowercase().contains(&query) { entries.push((file.clone(),item,name,count)); }
                        }
                    }
                    entries.sort_by(|a,b|a.2.to_lowercase().cmp(&b.2.to_lowercase()).then(a.0.path.cmp(&b.0.path)));
                    entries
                });view! {
                    if move ||!entries.with(Vec::is_empty) {
                        column(class="profile-section") {
                            ActionButton(class="profile-output-section",{..Attrs::new().a11y_from(A11yBinding::new(Role::Button).label(format!("Toggle saved {group}")).expanded(move ||open.get()))},on:click=move |_|open.update(|open|*open = !*open)) {
                                vector(class="profile-tree-arrow",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if open.get(){"M2 4 L6 8 L10 4"}else{"M4 2 L8 6 L4 10"}),a11y:hidden=true)
                                text {{move ||format!("{group} ({})",entries.with(Vec::len))}}
                            }
                            if move ||open.get() {
                                for entry in move ||entries.get(),key=|entry|(entry.0.clone(),entry.1.clone()) {
                                    {{let file=StoredValue::new_local(entry.0);let item=StoredValue::new_local(entry.1);let name=StoredValue::new_local(entry.2);let count=entry.3;view! {
                                        if move ||matches!(item.get_value(),Item::WorkspacePreset(_)) {
                                            WorkspaceCollection(workbench=workbench,file=file,name=name.get_value())
                                        } else {if move ||matches!(item.get_value(),Item::Workspace(_)) {
                                            WorkspacePreset(workbench=workbench,file=file,name=name.get_value())
                                        } else {
                                            ProfileOutputRow(workbench=workbench,file=file,drag=Signal::derive_local(move ||Drag{profile:ProfileKey::new(file.with_value(|file|file.path.clone()),workbench.profiles.flight.get()),item:item.get_value()}),name=name.get_value(),count=count)
                                        }
                                        }
                                    }}}
                                }
                            }
                        }
                    }
                }}}
            }
        }
    }
}
