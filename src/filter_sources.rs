use crate::{filter_source_row::FilterSourceRowProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterSources(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-sources", node_ref = workbench.filter_drop, class:filter-sources-hover = move || workbench.filter_drop_active.get()) {
            row(class = "filter-source-heading") {
                text(class = "filter-section-title") {{move || format!("{} source fields", workbench.filter_inputs.with(|inputs| inputs.len()))}}
                Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "clear-filter-sources", disabled = Signal::derive_local(move || workbench.filter_inputs.with(Vec::is_empty)), on:click = move |_| workbench.clear_filter_sources()) { "Clear fields" }
            }
            text(class = "muted") { "Drag fields here. Ctrl-click or Shift-click several in left tree, then drag any selected field to add whole selection." }
            if move || workbench.filter_inputs.with(Vec::is_empty) {
                text(class = "filter-drop-empty") { "Drop raw or derived fields here" }
            }
            column(class = "filter-source-basket") {
                for input in move || workbench.filter_inputs.get(), key = |input| input.source.clone() {
                    FilterSourceRow(workbench = workbench, source = input.source.clone())
                }
            }
        }
    }
}
