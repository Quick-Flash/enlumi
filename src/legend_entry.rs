use crate::{
    legend_drag::DragLine,
    state::{Line, Pane, Workbench},
};
use plot::zgui_legend::{LegendItemProps, LegendMarker, toggle_member};
use zgui::{prelude::*, reactive::UnsyncCallback};
use zgui_ui::prelude::*;

#[component]
pub fn LegendEntry(workbench: Workbench, pane: Pane, line: Line) -> impl IntoView {
    let id = line.id;
    let row = NodeRef::new();
    let drag = workbench.line_drag;
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
        workbench.select(pane.id, id);
        pane.plot
            .hidden
            .update(|hidden| toggle_member(hidden, &id.to_string()));
    };
    view! {
        row(class = "legend-entry", node_ref = row,
            class:legend-dragging = move || drag.state.get().is_some_and(|source| source.pane == pane.id && source.line == id && source.moving),
            class:legend-insert-before = move || drag.target.get().is_some_and(|target| target.pane == pane.id && target.before == Some(id)),
            class:legend-insert-after = move || drag.target.get().is_some_and(|target| target.pane == pane.id && target.before.is_none()) && pane.lines.with(|lines| lines.last().is_some_and(|line| line.id == id))) {
            row(class = "legend-drag-source", a11y:label = format!("Drag {} to move; Up/Down reorders", line.label()),
                on:pointer_down:capture = move |event| {
                    if event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                    workbench.select(pane.id, id);
                    drag.cancel();
                    drag.state.set(Some(DragLine { pane: pane.id, line: id, start: (event.position.x.0, event.position.y.0), moving: false }));
                    event.capture_pointer(); event.stop_propagation();
                },
                on:pointer_move = move |event| { workbench.update_line_drag(event.position.x.0, event.position.y.0); event.stop_propagation(); },
                on:pointer_up = move |event| {
                    workbench.update_line_drag(event.position.x.0, event.position.y.0);
                    let source = drag.state.get_untracked();
                    let target = drag.target.get_untracked();
                    drag.cancel(); event.release_pointer();
                    if let Some(source) = source {
                        if source.moving {
                            if let Some(target) = target { workbench.place_line(source.pane, source.line, target.pane, target.before); }
                            event.prevent_default();
                        } else { toggle(); }
                    }
                },
                on:pointer_cancel = move |event| { drag.cancel(); event.release_pointer(); },
                on:key_down:capture = move |event| {
                    use zgui::vocab::{Key, NamedKey};
                    if event.key == Key::Named(NamedKey::Escape) { drag.cancel(); event.prevent_default(); event.stop_propagation(); return; }
                    let lines = pane.lines.get_untracked();
                    let Some(index) = lines.iter().position(|line| line.id == id) else { return; };
                    let before = match event.key {
                        Key::Named(NamedKey::ArrowUp) if index > 0 => Some(lines[index - 1].id),
                        Key::Named(NamedKey::ArrowDown) if index + 1 < lines.len() => lines.get(index + 2).map(|line| line.id),
                        _ => return,
                    };
                    workbench.place_line(pane.id, id, pane.id, before); event.prevent_default(); event.stop_propagation();
                }) {
                LegendItem(label = line.label(), color = pane.plot.style(&id.to_string()).css_color(),
                    color_signal = Signal::derive_local(move || pane.plot.style(&id.to_string()).css_color()), marker = LegendMarker::Line,
                    visible = Signal::derive_local(move || !pane.plot.hidden.with(|hidden| hidden.contains(&id.to_string()))),
                    on_toggle = UnsyncCallback::new(move |()| toggle()), key = id.to_string(), hovered = pane.plot.emphasized)
            }
            Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "legend-remove", a11y:label = format!("Remove {}", line.label()),
                on:pointer_down = move |event| event.stop_propagation(),
                on:click = move |event| { pane.remove_line(id); event.stop_propagation(); }) { "x" }
        }
    }
}
