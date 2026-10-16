use crate::action_button::ActionButtonProps;
use crate::{
    filter_quaternion_slot::FilterQuaternionSlotProps, filter_source_row::FilterSourceRowProps,
    state::Workbench,
};
use zgui::prelude::*;

#[component]
pub fn FilterSources(workbench: Workbench) -> impl IntoView {
    workbench.filter_input_ui.clock.set_value(Timers::current());
    on_cleanup_local(move || {
        workbench.cancel_filter_input_drag();
        workbench.filter_input_ui.clock.set_value(None);
    });
    view! {
        column(class = "filter-sources", node_ref = workbench.filter_drop, class:filter-sources-hover = move || workbench.filter_drop_active.get()) {
            row(class = "filter-source-heading") {
                text(class = "filter-section-title") {{move || if workbench.combined_filter() { "Inputs" } else { "Inputs / outputs" }}}
                ActionButton(class = "clear-filter-sources", disabled = Signal::derive_local(move || workbench.filter_inputs.with(|inputs| inputs.iter().all(|input| input.source.is_empty()))), on:click = move |_| workbench.clear_filter_sources()) { "Clear fields" }
            }
            if move || workbench.filter_kind.get() != "rpy" && workbench.filter_inputs.with(Vec::is_empty) {
                text(class = "filter-drop-empty") { "Drop fields here" }
            }
            if move || workbench.filter_kind.get() == "rpy" {
                text(class = "muted") { "Unit quaternion: W, X, Y, Z. Drag to swap slots." }
            }
            column(class = "filter-source-basket") {
                if move || workbench.filter_kind.get() == "rpy" {
                    for index in move || 0..4, key = |index| *index {
                        FilterQuaternionSlot(workbench = workbench, index = index)
                    }
                } else {
                for input in move || workbench.filter_inputs.get(), key = |input| input.source.clone() {
                    FilterSourceRow(workbench = workbench, source = input.source.clone())
                }
                }
            }
        }
    }
}
