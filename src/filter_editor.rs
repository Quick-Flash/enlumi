use crate::copyable_text::CopyableTextProps;
use crate::filter_parameters::FilterParametersProps;
use crate::filter_save_button::FilterSaveButtonProps;
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
                if move || workbench.filter_kind.get() != "rust" && !workbench.filter_error.with(String::is_empty) {
                    CopyableText(class="filter-error",label="Filter error",value=workbench.filter_error)
                }
            }
            column(class = "filter-create-action") { FilterSaveButton(workbench = workbench, prominent = true) }

        }
    }
}
