use crate::filter_parameters::FilterParametersProps;
use crate::{action_button::ActionButtonProps, filter_save_button::FilterSaveButtonProps};
use crate::{
    filter_kind_picker::FilterKindPickerProps, filter_output_names::FilterOutputNamesProps,
    state::Workbench,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterEditor(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-editor") {
            column(class = "filter-editor-fields") {
                text(class = "filter-section-title") { "Filter" }
                row(class = "filter-actions") {
                    if move || !workbench.filter_mode.get().is_new() { FilterSaveButton(workbench = workbench) }

                    if move || !workbench.filter_mode.get().is_new() {
                        ActionButton(class = "filter-done-editing", on:click = move |_| workbench.finish_filter_editing()) { "Done editing" }
                    }

                    if move || !workbench.filter_outputs.with(Vec::is_empty) {
                        ActionButton(class = "chain-derived-field", disabled = Signal::derive_local(move || workbench.derived_busy.get()), on:click = move |_| {
                            workbench.next_filters();
                        }) { "Add next filter" }
                        ActionButton(class = "plot-derived-field", disabled = Signal::derive_local(move || workbench.derived_busy.get()), on:click = move |_| {
                            for id in workbench.filter_outputs.get_untracked() { workbench.add(crate::derived::key(id)); } workbench.filters_open.set(false);
                        }) { "Plot outputs" }
                    }
                }
                if move || workbench.filter_mode.get().is_new() && !workbench.filter_outputs.with(Vec::is_empty) && !workbench.derived_busy.get() {
                    row(class = "filter-result") {
                        text(class = "filter-result-status") {{move || format!("Created {} fields", workbench.filter_outputs.with(Vec::len))}}
                        ActionButton(class = "edit-created-fields", on:click = move |_| workbench.edit_created_filters()) { "Edit" }
                    }
                }
                FilterKindPicker(workbench = workbench)
                if move || workbench.filter_kind.get() == "rpy" {
                    text(class = "heading") { "Output units" }
                    Select(value = workbench.filter_angle_unit) {
                        SelectTrigger(label = "Quaternion output units", class = "filter-angle-unit") { SelectValue() }
                        SelectContent(class = "filter-type-surface") {
                            SelectItem(value = "rad") { "Radians (rad)" }
                            SelectItem(value = "deg") { "Degrees (deg)" }
                        }
                    }
                }
                if move || workbench.combined_filter() {
                    FilterOutputNames(workbench = workbench)
                } else {
                    FilterParameters(workbench = workbench)
                }
                if move || workbench.filter_kind.get() != "rust" && !workbench.filter_error.get().is_empty() {
                    text(class = "filter-error", a11y:role = Role::Alert) {{move || workbench.filter_error.get()}}
                }
            }
            if move || workbench.filter_mode.get().is_new() {
                column(class = "filter-create-action") { FilterSaveButton(workbench = workbench, prominent = true) }
            }

        }
    }
}
