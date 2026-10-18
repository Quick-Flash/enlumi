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

#[component]
pub fn ProfileOutputRow(
    workbench: Workbench,
    file: StoredValue<ProfileFile, LocalStorage>,
    drag: Signal<Drag, LocalStorage>,
    name: String,
    count: usize,
) -> impl IntoView {
    let label = StoredValue::new_local(name.clone());
    let more = RwSignal::new_local(false);
    let instance = Signal::derive_local(move || {
        workbench
            .profiles
            .instances
            .with(|all| all.get(&drag.get().profile).cloned())
    });
    let available = Signal::derive_local(move || {
        instance.with(|p| {
            p.as_ref().map_or_else(
                || file.with_value(|f| f.compatible.is_ok()),
                |p| match &drag.get().item {
                    Item::View(name) => p.views.contains_key(name),
                    Item::Field(name) => p.resolve(name).is_some_and(|source| {
                        crate::derived::id(&source).is_none()
                            || workbench.derived.with(|fields| {
                                fields
                                    .iter()
                                    .any(|field| field.key() == source && field.data.is_some())
                            })
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
    let disabled = Signal::derive_local(move || {
        !available.get()
            || workbench.profiles.busy.get()
            || workbench.derived_busy.get()
            || workbench.loading.get()
            || workbench.log.get().is_none()
    });
    let reason = Signal::derive_local(move || {
        instance
            .with(|p| {
                p.as_ref().and_then(|p| {
                    p.diagnostics
                        .iter()
                        .find(|d| {
                            d.resource == label.get_value()
                                || d.dependents.contains(&label.get_value())
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
        column(class="profile-output") {
            row(class="profile-output-line") {
                ProfileViewSource(workbench=workbench,source=Signal::derive_local(move ||drag.get().source()),name=name,count=count,enabled=available)
                ActionButton(class="profile-view-add",disabled=disabled,a11y:label=format!("Add {} to active plot",label.get_value()),on:click=move |_|workbench.place_profile_item(drag.get_untracked(),Some(workbench.active.get_untracked()))) { "Add" }
                ActionButton(class="profile-view-new",disabled=disabled,a11y:label=format!("Open {} in new plot",label.get_value()),on:click=move |_|workbench.place_profile_item(drag.get_untracked(),None)) { "New" }
                if move ||matches!(drag.get().item,Item::Field(_)) {
                    Popover(open=more) {
                        PopoverTrigger(class="app-action profile-output-more",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label=format!("Actions for {}",label.get_value())) { "⋯" }
                        PopoverContent(class="profile-menu-content") {
                            text(class="muted") {{label.get_value()}}
                            ActionButton(disabled=disabled,on:click=move |_|{more.set(false);workbench.use_profile_item(drag.get_untracked(),Destination::Filter);}) { "Use in filter" }
                        }
                    }
                }
            }
            if move ||!available.get() {text(class="profile-compatibility muted") {{move ||reason.get()}}}
        }
    }
}
