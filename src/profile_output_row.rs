use crate::tree_lines::TreeLinesProps;
use crate::{
    action_button::ActionButtonProps,
    profile::identity::{Drag, Item},
    profile_library::ProfileFile,
    profile_state::Destination,
    profile_view_source::ProfileViewSourceProps,
    state::Workbench,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

fn with_instance<R>(
    workbench: Workbench,
    drag: Signal<Drag, LocalStorage>,
    read: impl FnOnce(Option<&crate::profile_instance::AppliedProfile>) -> R,
) -> R {
    workbench
        .profiles
        .instances
        .with(|all| read(all.get(&drag.get().profile)))
}

#[component]
pub fn ProfileOutputRow(
    workbench: Workbench,
    file: StoredValue<std::sync::Arc<ProfileFile>, LocalStorage>,
    drag: Signal<Drag, LocalStorage>,
    name: String,
    count: usize,
    #[prop(default = false)] compact: bool,
) -> impl IntoView {
    let label = StoredValue::new_local(name.clone());
    let more = RwSignal::new_local(false);
    let members = RwSignal::new_local(false);
    let field_data = use_local_context::<crate::profile_outputs::ProfileFieldData>();
    let available = Memo::new(move |_| {
        with_instance(workbench, drag, |p| {
            p.map_or_else(
                || file.with_value(|f| f.compatible.is_ok()),
                |p| match &drag.get().item {
                    Item::View(name) => p.views.contains_key(name),
                    Item::Workspace(name) => p
                        .definition
                        .view_presets
                        .iter()
                        .any(|view| &view.name == name),
                    Item::WorkspacePreset(name) => p
                        .definition
                        .workspace_presets
                        .iter()
                        .any(|workspace| &workspace.name == name),
                    Item::Field(name) => p.resolve(name).is_some_and(|source| {
                        crate::derived::id(&source).is_none()
                            || field_data.map_or_else(
                                || {
                                    workbench.derived.with(|fields| {
                                        fields.iter().any(|field| {
                                            field.key() == source && field.data.is_some()
                                        })
                                    })
                                },
                                |data| data.counts.with(|counts| counts.contains_key(&source)),
                            )
                    }),
                    Item::Geometry(name) => p
                        .ids
                        .geometry
                        .get(name)
                        .is_some_and(|id| workbench.geometry_item(*id).is_some()),
                },
            )
        })
    });
    let disabled = Memo::new(move |_| {
        !available.get()
            || workbench.profiles.busy.get()
            || workbench.derived_busy.get()
            || workbench.loading.get()
            || workbench.log.get().is_none()
    });
    let disabled = Signal::derive_local(move || disabled.get());
    let reason = Memo::new(move |_| {
        with_instance(workbench, drag, |p| {
            p.and_then(|p| {
                p.diagnostics
                    .iter()
                    .find(|d| {
                        d.resource == label.get_value() || d.dependents.contains(&label.get_value())
                    })
                    .map(|d| d.message.clone())
            })
        })
        .unwrap_or_else(|| {
            file.with_value(|f| f.compatible.as_ref().err().cloned())
                .unwrap_or("Output missing; reload profile to retry.".into())
        })
    });
    view! {
        column(class="profile-output",class:profile-plot-source=move ||matches!(drag.get().item,Item::View(_))) {
            TreeLines(depth=0,last=true,open=Signal::derive_local(move ||members.get()))
            row(class="profile-output-line") {
                if move ||matches!(drag.get().item,Item::View(_)) && count>0 {
                    ActionButton(icon=true,class="profile-members-toggle",a11y:label="Show fields in plot preset",disabled=disabled,on:click=move |_|{
                        members.update(|open|*open = !*open);
                        if members.get_untracked() && !workbench.profiles.instances.with_untracked(|all|all.contains_key(&drag.get_untracked().profile)) { let drag=drag.get_untracked();workbench.profiles.flight.set(drag.profile.flight);workbench.apply_profile(drag.profile.path); }
                    }) {vector(class="profile-tree-arrow",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if members.get(){"M2 4 L6 8 L10 4"}else{"M4 2 L8 6 L4 10"}),a11y:hidden=true)}
                }
                ProfileViewSource(workbench=workbench,source=Signal::derive_local(move ||drag.get().source()),name=name,count=count,enabled=Signal::derive_local(move || !disabled.get()))
                if move ||compact && !available.get() {
                    Tooltip {
                        TooltipTrigger {text(class="profile-field-warning muted") {"Unavailable"}}
                        TooltipContent {text {{move ||reason.get()}}}
                    }
                }
                if move ||matches!(drag.get().item,Item::Field(_)|Item::View(_)) {
                    Popover(open=more) {
                        PopoverTrigger(class="app-action profile-output-more",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label=format!("Actions for {}",label.get_value())) { vector(class="profile-more-icon",prop:viewBox="0 0 12 12",prop:d="M2 6 L2.1 6 M6 6 L6.1 6 M10 6 L10.1 6",a11y:hidden=true) }
                        PopoverContent(class="profile-menu-content") {
                            text(class="muted") {{label.get_value()}}
                            text(class="muted") {{file.with_value(|file|format!("File: {}",file.path.file_name().unwrap_or_default().to_string_lossy()))}}
                            ActionButton(class="preset-edit-profile",disabled=Signal::derive_local(move ||workbench.authoring.busy.get()),on:click=move |_|{more.set(false);workbench.edit_profile_document(file.with_value(|file|file.path.clone()));}) {"Edit profile…"}
                            if move ||matches!(drag.get().item,Item::View(_)) {
                                ActionButton(class="preset-add-plot",disabled=disabled,on:click=move |_|{more.set(false);workbench.use_profile_item(drag.get_untracked(),Destination::Plot(None));}) {"Add as new plot"}
                                if move ||!crate::preset_lookup::is_nested_plot(drag.get().item.name()) {
                                ActionButton(class="preset-add-fields",disabled=disabled,on:click=move |_|{more.set(false);let item=drag.get_untracked();workbench.open_preset_browser(crate::preset_browser::BrowseTarget::Fields(workbench.active().get_untracked()),Some((item.profile.path,item.item.name().into())));}) {"Add fields to selected plot..."}
                                ActionButton(class="preset-delete",disabled=Signal::derive_local(move ||workbench.authoring.busy.get()||workbench.profiles.busy.get()),on:click=move |_|{more.set(false);workbench.delete_saved_preset(drag.get_untracked());}) {"Delete plot preset…"}
                                }
                            }
                            if move ||matches!(drag.get().item,Item::Field(_)) {
                            ActionButton(disabled=disabled,on:click=move |_|{more.set(false);workbench.use_profile_item(drag.get_untracked(),Destination::Filter);}) { "Use in filter" }
                            }
                        }
                    }
                }
            }
            if move ||members.get() {
                column(class="profile-view-members") {
                    for member in move || with_instance(workbench, drag, |instance| {
                        let Item::View(name)=drag.get().item else {return Vec::new();};
                        instance.and_then(|instance|instance.views.get(&name)).map_or_else(Vec::new,|view|view.fields.iter().map(|(source,_)|source.clone()).collect())
                    }),key=|source|source.clone() {
                        {{let name=workbench.field_name(&member);let source=member.clone();let last=Signal::derive_local(move ||with_instance(workbench,drag,|instance|{let Item::View(name)=drag.get().item else{return false;};instance.and_then(|instance|instance.views.get(&name)).and_then(|view|view.fields.last()).is_some_and(|(last,_)|last==&source)}));view! {column(class="profile-nested-entry") {TreeLines(depth=1,last=last)
                            ProfileViewSource(workbench=workbench,source=Signal::derive_local(move ||Drag{profile:drag.get().profile,item:Item::Field(member.clone())}.source()),name=name,count=0,enabled=Signal::derive_local(move ||!disabled.get()))
                        }}}}
                    }
                }
            }
            if move ||!available.get() && !compact {text(class="profile-compatibility muted") {{move ||reason.get()}}}
        }
    }
}
