use crate::{
    legend_entry::LegendEntryProps,
    plot_editor::PlotEditorProps,
    state::{Pane, Workbench},
};
use zgui::prelude::*;

#[component]
pub fn PlotLegend(workbench: Workbench, pane: Pane, node_ref: NodeRef) -> impl IntoView {
    view! {
        if move || !pane.lines.get().is_empty() {
        column(class = "workbench-legend", node_ref = node_ref) {
            row(class = "legend-heading") {
                text { "Legend" }
                PlotEditor(workbench = workbench, pane = pane)
            }
            for line in move || pane.lines.get(), key = |line| (line.id, line.label()) {
                LegendEntry(workbench = workbench, pane = pane, line = line)
            }
        }
        }
    }
}
