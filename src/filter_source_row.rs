use crate::action_button::ActionButtonProps;
use crate::state::Workbench;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterSourceRow(
    workbench: Workbench,
    source: String,
    #[prop(optional)] slot_index: Option<usize>,
) -> impl IntoView {
    let row = NodeRef::new();
    if slot_index.is_none() {
        workbench.filter_input_ui.rows.update_value(|rows| {
            rows.insert(source.clone(), row);
        });
        let key = source.clone();
        on_cleanup_local(move || {
            workbench.filter_input_ui.rows.update_value(|rows| {
                if rows.get(&key) == Some(&row) {
                    rows.remove(&key);
                }
            });
        });
    }
    let source = StoredValue::new_local(source);
    let reorderable = move || slot_index.is_some() || workbench.filter_kind.get() == "rust";
    let index = move || {
        slot_index.unwrap_or_else(|| {
            source.with_value(|source| {
                workbench.filter_inputs.with(|inputs| {
                    inputs
                        .iter()
                        .position(|input| &input.source == source)
                        .unwrap_or(0)
                })
            })
        })
    };
    let name = Binding::controlled(
        Signal::derive_local(move || {
            source.with_value(|source| {
                workbench.filter_inputs.with(|inputs| {
                    inputs
                        .iter()
                        .find(|input| &input.source == source)
                        .map_or_else(String::new, |input| input.name.clone())
                })
            })
        }),
        move |name: String| {
            source.with_value(|source| {
                workbench.filter_inputs.update(|inputs| {
                    if let Some(input) = inputs.iter_mut().find(|input| &input.source == source) {
                        input.name = name;
                    }
                })
            })
        },
    );
    view! {
        column(class = "filter-source-row", node_ref = row, class:filter-input-handle = reorderable, tabindex = move || if reorderable() { Focus::Sequential } else { Focus::Programmatic },
            class:filter-input-rebase = move || workbench.filter_input_ui.rebase.get(),
            a11y:label = move || zgui::vocab::SharedString::from(source.with_value(|source| if reorderable() { format!("Drag {} to reorder inputs. Up/Down moves input; Escape cancels.", workbench.field_name(source)) } else { format!("Input {}",workbench.field_name(source)) })),
            class:filter-input-dragging = move || workbench.filter_input_ui.drag.get().is_some_and(|drag| drag.from == index() && drag.moving && !drag.settling),
            style:transform = move || Some(workbench.filter_input_offset(index())),
            on:pointer_down = move |event| {
                if !reorderable() || event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                workbench.start_filter_input_drag(index(), event.position.x.0, event.position.y.0);
                if workbench.filter_input_ui.drag.get_untracked().is_some_and(|drag| drag.from == index() && !drag.settling) {
                    event.capture_pointer(); event.prevent_default(); event.stop_propagation();
                }
            },
            on:pointer_move = move |event| { workbench.move_filter_input_drag(event.position.x.0, event.position.y.0); },
            on:pointer_up = move |event| {
                if workbench.filter_input_ui.drag.get_untracked().is_none() { return; }
                workbench.move_filter_input_drag(event.position.x.0, event.position.y.0);
                event.release_pointer(); workbench.finish_filter_input_drag(false); event.stop_propagation();
            },
            on:pointer_cancel = move |event| { event.release_pointer(); workbench.finish_filter_input_drag(true); },
            on:key_down = move |event| {
                if !reorderable() { return; }
                use zgui::vocab::{Key, NamedKey};
                match event.key {
                    Key::Named(NamedKey::Escape) => workbench.finish_filter_input_drag(true),
                    Key::Named(NamedKey::ArrowUp) => source.with_value(|source| workbench.move_filter_input(source,-1)),
                    Key::Named(NamedKey::ArrowDown) => source.with_value(|source| workbench.move_filter_input(source,1)),
                    _ => return,
                }
                event.prevent_default(); event.stop_propagation();
            }) {
            row(class = "filter-source-row-heading") {
                if move || slot_index.is_none() { text(class = "filter-io-label") {{move || if workbench.filter_kind.get() == "rust" { format!("inputs[{}]", index()) } else { "Input".into() }}} }
                text(class = "filter-source-label") {{move || source.with_value(|source| workbench.field_name(source))}}
                if move || !workbench.combined_filter() {
                    text(class = "filter-io-label") { "Output" }
                    Input(value = name, class = "filter-output-name", a11y:label = move || zgui::vocab::SharedString::from(source.with_value(|source| format!("Output name for {source}"))))
                }
                ActionButton(icon = true, class = "remove-filter-source", a11y:label = "Remove filter source", on:pointer_down = move |event| event.stop_propagation(), on:click = move |_| workbench.remove_filter_input(index())) { "\u{00d7}" }
            }
        }
    }
}
