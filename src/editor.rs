use crate::{
    line_style::LineStyleProps,
    state::{Pane, Workbench},
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn Editor(workbench: Workbench, pane: Pane) -> impl IntoView {
    let selected = Signal::derive_local(move || {
        let id = pane.selected.get();
        pane.lines
            .with(|lines| lines.iter().find(|line| Some(line.id) == id).cloned())
    });
    view! {
        column(class = "editor") {
            column(class = "line-list") {
                for line in move || pane.lines.get(), key = |line| line.id {
                    Button(size = ButtonSize::Sm, class = "line-button", style:background-color = move || { let [r,g,b,_] = pane.plot.style(&line.id.to_string()).color; Some(format!("rgba({r}, {g}, {b}, 0.25)")) }, style:border-color = move || Some(pane.plot.style(&line.id.to_string()).css_color()), class:selected = move || selected.get().is_some_and(|selected| selected.id == line.id),
                        on:click = move |_| workbench.select(pane.id, line.id)) {
                        box(class = "line-color-swatch", style:background-color = move || Some(pane.plot.style(&line.id.to_string()).css_color())) {}
                        text {{move || pane.lines.with(|lines| lines.iter().find(|current| current.id == line.id).map_or_else(String::new, |current| current.label()))}}
                    }
                }
            }
            if move || selected.get().is_some() {
            for line in move || selected.get().into_iter().collect::<Vec<_>>(), key = |line| line.id {
                LineStyle(id = line.id, pane = pane)
            }
            row(class = "plot-toolbar") {
                Button(size = ButtonSize::Sm, disabled = Signal::derive_local(move || selected.get().is_none()),
                    on:click = move |_| { workbench.activate(pane.id); workbench.remove(); }) { "Remove line" }
                Button(size = ButtonSize::Sm, disabled = Signal::derive_local(move || selected.get().is_none() || workbench.panes.with(|panes| panes.len()) < 2),
                    on:click = move |_| { workbench.activate(pane.id); workbench.move_line(); }) { "Move to next plot" }
            }
            text(class = "muted") { "Drag fields into Filters workspace to build filter chains." }
            } else {
                text(class = "editor-empty muted") {{move || if pane.lines.with(|lines| lines.is_empty()) { "Drop a field onto this plot to begin." } else { "Select a line to edit its appearance." }}}
            }
        }
    }
}
