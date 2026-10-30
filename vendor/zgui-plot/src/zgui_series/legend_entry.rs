use super::{LegendHost, SeriesLine as Line, SeriesPane as Pane, legend_drag::DragLine};
use crate::zgui_legend::{LegendItemProps, LegendMarker, toggle_member};
use zgui::{prelude::*, reactive::UnsyncCallback};
use zgui_ui::prelude::*;

#[component]
pub fn LegendEntry(host: LegendHost, pane: Pane, line: Line) -> impl IntoView {
    let id = line.id;
    let row = NodeRef::new();
    let drag = host.drag;
    drag.rows.update_value(|rows| {
        rows.insert((pane.id, id), row);
    });
    on_cleanup_local(move || {
        if pane.plot.emphasized.get_untracked().as_deref() == Some(&id.to_string()) {
            pane.plot.emphasized.set(None);
        }
        drag.rows.update_value(|rows| {
            rows.remove(&(pane.id, id));
        });
        if drag
            .state
            .get_untracked()
            .is_some_and(|source| source.pane == pane.id && source.line == id)
        {
            drag.cancel();
        }
    });
    let toggle = move || {
        pane.plot.auto_fit.set(false);
        pane.selected.set(Some(id));
        host.actions.select.run((pane.id, id));
        pane.plot
            .hidden
            .update(|hidden| toggle_member(hidden, &id.to_string()));
    };
    let legend_appearance = crate::zgui_legend::LegendAppearance {
        color: pane.plot.style(&id.to_string()).css_color(),
        color_signal: (Signal::derive_local(move || pane.plot.style(&id.to_string()).css_color()))
            .into(),
        marker: LegendMarker::Line,
    };
    view! {
            row(class = "legend-entry", node_ref = row,
                class:legend-dragging = move || drag.state.get().is_some_and(|source| source.pane == pane.id && source.line == id && source.moving),
                class:legend-insert-before = move || drag.target.get().is_some_and(|target| target.pane == pane.id && target.before == Some(id)),
                class:legend-insert-after = move || drag.target.get().is_some_and(|target| target.pane == pane.id && target.before.is_none()) && host.keys(pane).last() == Some(&id)) {
                row(class = "legend-drag-source", a11y:label = format!("Drag {} to move; Up/Down reorders", line.label()),
                    on:pointer_enter = move |_| pane.plot.emphasized.set(Some(id.to_string())),
                    on:pointer_leave = move |_| {
                        if pane.plot.emphasized.get_untracked().as_deref() == Some(&id.to_string()) {
                            pane.plot.emphasized.set(None);
                        }
                    },
                    on:pointer_down:capture = move |event| {
                        if event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                        pane.selected.set(Some(id)); host.actions.select.run((pane.id, id));
                        drag.cancel();
                        drag.state.set(Some(DragLine { pane: pane.id, line: id, start: (event.position.x.0, event.position.y.0), moving: false }));
                        event.capture_pointer(); event.stop_propagation();
                    },
                    on:pointer_move = move |event| {
                        host.update(event.position.x.0, event.position.y.0);
                        if drag.state.get_untracked().is_some_and(|source| source.moving)
                            && pane.plot.emphasized.get_untracked().as_deref() == Some(&id.to_string()) {
                            pane.plot.emphasized.set(None);
                        }
                        event.stop_propagation();
                    },
                    on:pointer_up = move |event| {
                        host.update(event.position.x.0, event.position.y.0);
                        let source = drag.state.get_untracked();
                        let target = drag.target.get_untracked();
                        drag.cancel(); event.release_pointer();
                        if let Some(source) = source {
                            if source.moving {
                                if let Some(target) = target { host.actions.move_line.run((source.pane, source.line, target.pane, target.before)); }
                                event.prevent_default();
                            } else { toggle(); }
                        }
                    },
                    on:pointer_cancel = move |event| { drag.cancel(); event.release_pointer(); },
                    on:key_down:capture = move |event| {
                        use zgui::vocab::{Key, NamedKey};
                        if event.key == Key::Named(NamedKey::Escape) { drag.cancel(); event.prevent_default(); event.stop_propagation(); return; }
                        let lines = host.keys(pane);
                        let Some(index) = lines.iter().position(|line| *line == id) else { return; };
                        let before = match event.key {
                            Key::Named(NamedKey::ArrowUp) if index > 0 => Some(lines[index - 1]),
                            Key::Named(NamedKey::ArrowDown) if index + 1 < lines.len() => lines.get(index + 2).copied(),
                            _ => return,
                        };
                        host.actions.move_line.run((pane.id, id, pane.id, before)); event.prevent_default(); event.stop_propagation();
                    }) {
                    LegendItem(label = line.label(),
    visible = Signal::derive_local(move || !pane.plot.hidden.with(|hidden| hidden.contains(&id.to_string()))),
    on_toggle = UnsyncCallback::new(move |()| toggle()),
    key = id.to_string(),
    appearance = legend_appearance)
                }
                Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "legend-remove", a11y:label = format!("Remove {}", line.label()),
                    on:pointer_down = move |event| event.stop_propagation(),
                    on:click = move |event| { host.actions.remove.run((pane.id, id)); event.stop_propagation(); }) { "x" }
            }
        }
}
