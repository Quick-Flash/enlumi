use crate::state::Workbench;
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
                workbench.select_field(source.get_untracked(), ctrl, shift, &workbench.visible_profile_sources());
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
            SeriesSource(id = id,
                label = Signal::stored_local(name), count = Signal::stored_local(match count {0=>String::new(),_=>count.to_string()}),
                selected = Signal::derive_local(move || workbench.selected_fields.with(|fields| fields.contains(&source.get()))), drag = workbench.drag, actions = actions)
        }
    }
}
