use crate::{filter_source_row::FilterSourceRowProps, state::Workbench};
use zgui::prelude::*;

#[component]
pub fn FilterQuaternionSlot(workbench: Workbench, index: usize) -> impl IntoView {
    let input = Memo::new(move |_| {
        workbench.filter_inputs.with(|inputs| {
            inputs
                .get(index)
                .filter(|input| !input.source.is_empty())
                .cloned()
        })
    });
    view! {
        row(class = "filter-quaternion-slot", node_ref = workbench.filter_input_ui.slots[index],
            class:filter-slot-hover = move || workbench.filter_input_ui.hover.get() == Some(index)) {
            text(class = "filter-input-role") { {["W", "X", "Y", "Z"][index]} }
            column(class = "filter-slot-content") {
                if move || input.get().is_none() {
                    text(class = "filter-slot-empty") { "Drop field here" }
                }
                for input in move || input.get().into_iter().collect::<Vec<_>>(), key = |input| input.source.clone() {
                    FilterSourceRow(workbench = workbench, source = input.source, slot_index = index)
                }
            }
        }
    }
}
