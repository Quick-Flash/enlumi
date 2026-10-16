use crate::action_button::ActionButtonProps;
use crate::{
    filter_kind_picker::FilterKindPickerProps, filter_output_names::FilterOutputNamesProps,
    state::Workbench,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterEditor(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-editor", class:script-filter-editor = move || workbench.filter_kind.get() == "rust") {
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
            if move || workbench.filter_kind.get() == "affine" {
                row(class = "filter-angle-presets") {
                    text(class = "muted") { "Presets" }
                    ActionButton(class = "preset-rad-to-deg", on:click = move |_| {
                        workbench.filter_gain.set(1.0_f64.to_degrees().to_string()); workbench.filter_offset.set("0".into());
                    }) { "rad → deg" }
                    ActionButton(class = "preset-deg-to-rad", on:click = move |_| {
                        workbench.filter_gain.set(1.0_f64.to_radians().to_string()); workbench.filter_offset.set("0".into());
                    }) { "deg → rad" }
                }
                row(class = "filter-parameters") {
                    column { text(class = "heading") { "Gain" } Input(value = workbench.filter_gain, a11y:label = "Filter gain") }
                    column { text(class = "heading") { "Offset" } Input(value = workbench.filter_offset, a11y:label = "Filter offset") }
                }
                text(class = "muted") { "Output = input × gain + offset" }
            } else {
                text(class = "heading") { "Cutoff (Hz)" }
                Input(value = workbench.filter_cutoff, a11y:label = "Filter cutoff in Hz")
                text(class = "muted") { "Actual timestamps; gaps reset state." }
            }
            }
            if move || workbench.filter_kind.get() != "rust" && !workbench.filter_error.get().is_empty() {
                text(class = "filter-error", a11y:role = Role::Alert) {{move || workbench.filter_error.get()}}
            }
            if move || workbench.filter_mode.get().is_new() && !workbench.filter_outputs.with(Vec::is_empty) && !workbench.derived_busy.get() {
                row(class = "filter-result") {
                    text(class = "filter-result-status") {{move || format!("Created {} fields", workbench.filter_outputs.with(Vec::len))}}
                    ActionButton(class = "edit-created-fields", on:click = move |_| workbench.edit_created_filters()) { "Edit" }
                }
            }
            row(class = "filter-actions") {
                ActionButton(class = "save-derived-field", disabled = Signal::derive_local(move || workbench.loading.get() || workbench.script_busy.get() || workbench.derived_busy.get() || workbench.log.get().is_none() || workbench.filter_input_ui.drag.get().is_some() || workbench.filter_inputs.with(|inputs| inputs.iter().all(|input| input.source.is_empty()) || workbench.filter_kind.get() == "rpy" && (inputs.len() != 4 || inputs.iter().any(|input| input.source.is_empty())))),
                    on:click = move |_| workbench.save_filter()) {{move || if workbench.derived_busy.get() { "Computing\u{2026}".to_owned() } else if !workbench.filter_mode.get().is_new() { "Save changes".to_owned() } else if workbench.filter_kind.get() == "rpy" { "Create 3 fields".to_owned() } else if workbench.filter_kind.get() == "rust" { "Compile & create".to_owned() } else if workbench.filter_kind.get() == "magnitude" { "Create magnitude".to_owned() } else { format!("Create {} fields", workbench.filter_inputs.with(Vec::len)) }}}

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
        }
    }
}
