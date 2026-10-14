use crate::{field_tree::TreeRow, state::Workbench};
use plot::zgui_workspace::{SeriesSourceActions, SeriesSourceProps};
use zgui::{prelude::*, reactive::UnsyncCallback};

#[component]
pub fn FieldRow(
    workbench: Workbench,
    #[prop(into)] row: Signal<Option<TreeRow>>,
    #[prop(into)] visible_fields: Signal<Vec<String>>,
) -> impl IntoView {
    let plain_click = RwSignal::new_local(true);
    let actions = SeriesSourceActions {
        on_move: UnsyncCallback::new(move |(x, y)| workbench.field_drag_move(x, y)),
        on_drop: UnsyncCallback::new(move |(field, x, y)| workbench.drop_fields(field, x, y)),
        on_cancel: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
        on_finish: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
    };
    let current_id = move || row.get_untracked().map(|row| row.id);
    let semantics = move || {
        Attrs::new().a11y_from(
            A11yBinding::new(Role::Button)
                .expanded(move || row.get().is_some_and(|row| row.open))
                .label(move || {
                    zgui::vocab::SharedString::from(
                        row.get().map_or_else(String::new, |row| row.label),
                    )
                }),
        )
    };
    view! {
        row(class = "tree-row", style:padding-left = move || Some(format!("{}px", row.get().map_or(0, |row| row.depth) * 12))) {
            box(class = "tree-connectors", a11y:hidden = true) {
                for depth in move || row.get().map_or_else(Vec::new, |row| row.guides), key = |depth| *depth {
                    box(class = "tree-guide", style:left = Some(format!("{}px", depth * 12 + 10))) {}
                }
                if move || row.get().is_some_and(|row| row.depth > 0) {
                    box(class = "tree-guide", class:tree-guide-last = move || row.get().is_some_and(|row| row.last), style:left = move || Some(format!("{}px", row.get().map_or(0, |row| row.depth.saturating_sub(1)) * 12 + 10))) {}
                    box(class = "tree-elbow", style:left = move || Some(format!("{}px", row.get().map_or(0, |row| row.depth.saturating_sub(1)) * 12 + 10))) {}
                }
                if move || row.get().is_some_and(|row| row.open) {
                    box(class = "tree-stem", style:left = move || Some(format!("{}px", row.get().map_or(0, |row| row.depth) * 12 + 10))) {}
                }
            }
            if move || row.get().is_some_and(|row| row.field.is_none()) {
                control(class = "tree-branch", class:tree-open = move || row.get().is_some_and(|row| row.open), class:topic-branch = move || row.get().is_some_and(|row| row.depth == 0), class:field-branch = move || row.get().is_some_and(|row| row.depth > 0), tabindex = Focus::Sequential, {..semantics()},
                    on:click = move |_| { if let Some(id) = current_id() { workbench.toggle_branch(id); } },
                    on:key_down = move |event| {
                        let Some(current) = row.get_untracked() else { return; };
                        match event.key {
                            zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowRight) if !current.open => workbench.toggle_branch(current.id),
                            zgui::vocab::Key::Named(zgui::vocab::NamedKey::ArrowLeft) if current.open => workbench.toggle_branch(current.id),
                            zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter | zgui::vocab::NamedKey::Space) => workbench.toggle_branch(current.id),
                            _ => return,
                        }
                        event.prevent_default(); event.stop_propagation();
                    }) {
                    vector(class = "tree-arrow", prop:viewBox = "0 0 12 12", prop:d = move || PropValue::from(if row.get().is_some_and(|row| row.open) { "M2 4 L6 8 L10 4" } else { "M4 2 L8 6 L4 10" }), a11y:hidden = true)
                    text(class = "tree-name") {{move || row.get().map_or_else(String::new, |row| row.label)}}
                    text(class = "tree-count") {{move || row.get().map_or_else(String::new, |row| row.count.to_string())}}
                }
            } else {
                row(class = "field-leaf", on:pointer_down = move |event| {
                        if event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                        let ctrl = event.modifiers.contains(zgui::vocab::Modifiers::CONTROL);
                        let shift = event.modifiers.contains(zgui::vocab::Modifiers::SHIFT);
                        plain_click.set(!ctrl && !shift);
                        if let Some(source) = row.get_untracked().and_then(|row| row.field) {
                            workbench.select_field(source, ctrl, shift, &visible_fields.get_untracked());
                        }
                    },
                    on:pointer_move = move |_| {
                        if workbench.drag.state.get_untracked().is_some_and(|drag| drag.3) { plain_click.set(false); }
                    },
                    on:pointer_cancel = move |_| plain_click.set(true),
                    on:click = move |_| {
                        if plain_click.get_untracked() && let Some(source) = row.get_untracked().and_then(|row| row.field) {
                            workbench.selected_fields.set(std::collections::BTreeSet::from([source.clone()]));
                            workbench.selected_field.set(Some(source.clone()));
                            workbench.selection_anchor.set(Some(source));
                        }
                        plain_click.set(true);
                    },
                    on:key_down = move |event| {
                        if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter) {
                            if let Some(field) = row.get_untracked().and_then(|row| row.field) { workbench.selected_field.set(Some(field.clone())); workbench.add(field); }
                            event.prevent_default(); event.stop_propagation();
                        }
                    }) {
                    SeriesSource(id = Signal::derive_local(move || row.get().and_then(|row| row.field)),
                        label = Signal::derive_local(move || row.get().map_or_else(String::new, |row| row.label)),
                        count = Signal::derive_local(move || row.get().map_or_else(String::new, |row| format!("{} samples", row.count))),
                        selected = Signal::derive_local(move || row.get().and_then(|row| row.field).is_some_and(|field| workbench.selected_fields.with(|fields| fields.contains(&field)))),
                        drag = workbench.drag, actions = actions)
                }
            }
        }
    }
}
