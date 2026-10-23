use crate::{
    action_button::ActionButtonProps,
    geometry::Geometry,
    state::{Pane, Workbench},
};
use plot::zgui_legend::{LegendItemProps, LegendMarker};
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn GeometryLegendRow(workbench: Workbench, pane: Pane, item: Geometry) -> impl IntoView {
    let id = item.id;
    let has_states = item.states.is_some();
    let key = !id;
    let host = workbench.legend_host();
    let drag = host.drag;
    let row = NodeRef::new();
    drag.rows.update_value(|rows| {
        rows.insert((pane.id, key), row);
    });
    on_cleanup_local(move || {
        drag.rows.update_value(|rows| {
            rows.remove(&(pane.id, key));
        });
        if drag
            .state
            .get_untracked()
            .is_some_and(|source| source.pane == pane.id && source.line == key)
        {
            drag.cancel();
        }
    });
    let legend_appearance = plot::zgui_legend::LegendAppearance {
        color: format!(
            "#{:02x}{:02x}{:02x}",
            item.color[0], item.color[1], item.color[2]
        ),
        color_signal: (Signal::derive_local(move || {
            workbench.geometry.items.track();
            workbench
                .geometry_item(id)
                .map_or_else(String::new, |item| {
                    format!(
                        "#{:02x}{:02x}{:02x}",
                        item.color[0], item.color[1], item.color[2]
                    )
                })
        }))
        .into(),
        marker: LegendMarker::Line,
    };
    view! {
            Tooltip {
                TooltipTrigger(class="geometry-legend-trigger") {
                row(class="legend-entry geometry-legend-row", node_ref = row,
                    class:legend-dragging = move || drag.state.get().is_some_and(|source| source.pane == pane.id && source.line == key && source.moving),
                    class:legend-insert-before = move || drag.target.get().is_some_and(|target| target.pane == pane.id && target.before == Some(key)),
                    class:legend-insert-after = move || drag.target.get().is_some_and(|target| target.pane == pane.id && target.before.is_none()) && workbench.legend_keys(pane).last() == Some(&key),class:geometry-legend-hidden=move || pane.hidden_geometry.with(|ids|ids.contains(&id)),class:geometry-legend-editing=move || workbench.geometry.editing.get() && workbench.geometry.selected.get()==Some(id) && workbench.active().get()==pane.id) {
                    row(class="legend-drag-source geometry-legend-toggle", a11y:label = "Drag geometry to move; Up/Down reorders",
                        on:pointer_down:capture = move |event| {
                            if event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                            drag.cancel();
                            drag.state.set(Some(plot::zgui_series::DragLine { pane: pane.id, line: key, start: (event.position.x.0, event.position.y.0), moving: false }));
                            event.capture_pointer(); event.stop_propagation();
                        },
                        on:pointer_move = move |event| { host.update(event.position.x.0, event.position.y.0); event.stop_propagation(); },
                        on:pointer_up = move |event| {
                            host.update(event.position.x.0, event.position.y.0);
                            let source = drag.state.get_untracked(); let target = drag.target.get_untracked();
                            drag.cancel(); event.release_pointer();
                            if let Some(source) = source {
                                if source.moving {
                                    if let Some(target) = target { workbench.place_line(source.pane, source.line, target.pane, target.before); }
                                    event.prevent_default();
                                } else { workbench.toggle_geometry(pane, id); }
                            }
                        },
                        on:pointer_cancel = move |event| { drag.cancel(); event.release_pointer(); },
                        on:key_down:capture = move |event| {
                            use zgui::vocab::{Key, NamedKey};
                            if event.key == Key::Named(NamedKey::Escape) { drag.cancel(); event.prevent_default(); event.stop_propagation(); return; }
                            let keys = workbench.legend_keys(pane);
                            let Some(index) = keys.iter().position(|id| *id == key) else { return; };
                            let before = match event.key {
                                Key::Named(NamedKey::ArrowUp) if index > 0 => Some(keys[index - 1]),
                                Key::Named(NamedKey::ArrowDown) if index + 1 < keys.len() => keys.get(index + 2).copied(),
                                _ => return,
                            };
                            workbench.place_line(pane.id, key, pane.id, before); event.prevent_default(); event.stop_propagation();
                        }) {
                        LegendItem(label=item.name,
    visible=Signal::derive_local(move ||!pane.hidden_geometry.with(|ids|ids.contains(&id))),
    on_toggle=UnsyncCallback::new(move |()|workbench.toggle_geometry(pane,id)),
    key=id,
    appearance = legend_appearance)
                    }
                    ActionButton(icon=true,class="geometry-legend-edit",a11y:label="Edit geometry",on:pointer_down=move |event|event.stop_propagation(),on:click=move |event|{workbench.edit_geometry_on(pane,id);event.stop_propagation();}) {
                        vector(prop:viewBox="0 0 20 20",prop:d="M4 14 L3 18 L7 17 L17 7 L13 3 Z M11 5 L15 9",a11y:hidden=true)
                    }
                    ActionButton(icon=true,class="geometry-legend-remove",a11y:label="Remove geometry from plot",on:pointer_down=move |event|event.stop_propagation(),on:click=move |event|{workbench.remove_geometry_from(pane,id);event.stop_propagation();}) {"×"}
                }
                }
                if move || has_states {
                TooltipContent(class="geometry-state-popup") {
                column(class = "geometry-state-colors") {
                    text(class="heading") {"State modes"}
                    for rule in move || { workbench.geometry.items.track(); workbench.geometry_item(id).and_then(|item| item.states)
                        .map_or_else(Vec::new, |states| states.rules.clone()) },
                        key = |rule| (rule.value, rule.name.clone(), rule.color) {
                        text(style:color = format!("#{:02x}{:02x}{:02x}", rule.color[0], rule.color[1], rule.color[2])) { {rule.name} }
                    }
                }
                }
                }
            }
        }
}
