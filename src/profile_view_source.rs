use crate::{field_actions::FieldActionsProps, state::Workbench};
use plot::zgui_workspace::{SeriesSourceActions, SeriesSourceProps};
use zgui::prelude::*;

/// A whole saved view uses the same drag gesture as a signal.
#[component]
pub fn ProfileViewSource(
    workbench: Workbench,
    source: Signal<String, LocalStorage>,
    name: String,
    count: usize,
    enabled: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let node = NodeRef::new();
    let row_id = workbench.id();
    let id = Signal::derive_local(move || {
        if enabled.get() {
            Some(source.get())
        } else {
            None
        }
    });
    workbench.profiles.rows.update_value(|rows| {
        rows.insert(
            row_id,
            crate::profile_selection::ProfileSourceRow { source: id, node },
        );
    });
    on_cleanup_local(move || {
        workbench.profiles.rows.update_value(|rows| {
            rows.remove(&row_id);
        })
    });
    let plain_click = RwSignal::new_local(true);
    let samples = Signal::derive_local(move || {
        let Some(drag) = crate::profile::identity::Drag::parse(&source.get()) else {
            return String::new();
        };
        let crate::profile::identity::Item::Field(name) = drag.item else {
            return if count == 0 {
                String::new()
            } else {
                format!("{count} fields")
            };
        };
        let resolved = workbench.profiles.instances.with(|instances| {
            instances
                .get(&drag.profile)
                .and_then(|instance| instance.resolve(&name))
        });
        let Some(source) = resolved else {
            return String::new();
        };
        let count = if source.starts_with("@derived/") {
            workbench.derived.with(|fields| {
                fields
                    .iter()
                    .find(|field| field.key() == source)
                    .and_then(|field| field.data.as_ref().map(|data| data.samples().len()))
            })
        } else {
            workbench.log.with(|log| {
                log.as_ref().and_then(|log| {
                    log.catalogue
                        .iter()
                        .find(|field| field.id == source)
                        .map(|field| field.count)
                })
            })
        };
        count.map_or_else(String::new, |count| count.to_string())
    });
    let actions = SeriesSourceActions {
        on_move: UnsyncCallback::new(move |(x, y)| workbench.field_drag_move(x, y)),
        on_drop: UnsyncCallback::new(move |(source, x, y)| workbench.drop_fields(source, x, y)),
        on_cancel: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
        on_finish: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
    };
    view! {
        row(class = "profile-view-source", node_ref = node,
            on:pointer_down:capture = move |event| {
                if event.button != Some(zgui::vocab::PointerButton::Primary) || !enabled.get_untracked() { return; }
                let ctrl = event.modifiers.contains(zgui::vocab::Modifiers::CONTROL);
                let shift = event.modifiers.contains(zgui::vocab::Modifiers::SHIFT);
                plain_click.set(!ctrl && !shift);
                workbench.select_field(source.get_untracked(), ctrl, shift, &workbench.sidebar_sources());
            },
            on:pointer_move = move |_| {
                if workbench.drag.state.get_untracked().is_some_and(|drag| drag.3) { plain_click.set(false); }
            },
            on:pointer_cancel = move |_| plain_click.set(true),
            on:click = move |_| {
                if plain_click.get_untracked() && enabled.get_untracked() {
                    let source = source.get_untracked();
                    workbench.selected_fields.set(std::collections::BTreeSet::from([source.clone()]));
                    workbench.selected_field.set(Some(source.clone()));
                    workbench.selection_anchor.set(Some(source));
                }
                plain_click.set(true);
            }) {
            FieldActions(workbench=workbench,source=Signal::derive_local(move ||id.get()),on_double_click=UnsyncCallback::new(move |()|{if let Some(item)=crate::profile::identity::Drag::parse(&source.get_untracked()) && matches!(item.item,crate::profile::identity::Item::View(_)|crate::profile::identity::Item::Workspace(_)|crate::profile::identity::Item::WorkspacePreset(_)){workbench.use_profile_item(item,crate::profile_state::Destination::Plot(None));}})) {
            SeriesSource(id = id,
                label = Signal::derive_local({let name=name.clone();move || {let Some(item)=crate::profile::identity::Drag::parse(&source.get())else{return name.clone();};if let crate::profile::identity::Item::Field(token)=item.item&&let Some(source)=workbench.profiles.instances.with(|instances|instances.get(&item.profile).and_then(|instance|instance.resolve(&token))){workbench.field_name(&source)}else{name.clone()}}}), count = samples,
                selected = Signal::derive_local(move || workbench.selected_fields.with(|fields| fields.contains(&source.get()))), drag = workbench.drag, actions = actions)
            }
        }
    }
}
