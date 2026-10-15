use crate::{filter_output_names::FilterOutputNamesProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterEditor(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-editor") {
            text(class = "filter-section-title") { "Filter" }
            Select(value = Binding::controlled(Signal::derive_local(move || workbench.filter_kind.get()), move |kind| workbench.set_filter_kind(kind))) {
                SelectTrigger(label = "Filter type", class = "filter-type") { SelectValue(placeholder = "Choose filter") }
                SelectContent(class = "filter-type-surface") {
                    SelectItem(value = "low") { "Low-pass (PT1)" }
                    SelectItem(value = "high") { "High-pass (PT1)" }
                    SelectItem(value = "affine") { "Gain / offset" }
                    SelectItem(value = "magnitude") { "Magnitude" }
                    SelectItem(value = "rpy") { "Quaternion → RPY" }
                }
            }
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
                    Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "preset-rad-to-deg", on:click = move |_| {
                        workbench.filter_gain.set(1.0_f64.to_degrees().to_string()); workbench.filter_offset.set("0".into());
                    }) { "rad → deg" }
                    Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "preset-deg-to-rad", on:click = move |_| {
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
            if move || !workbench.filter_error.get().is_empty() {
                text(class = "filter-error", a11y:role = Role::Alert) {{move || workbench.filter_error.get()}}
            }
            if move || workbench.filter_mode.get().is_new() && !workbench.filter_outputs.with(Vec::is_empty) && !workbench.derived_busy.get() {
                row(class = "filter-result") {
                    text(class = "filter-result-status") {{move || format!("Created {} fields", workbench.filter_outputs.with(Vec::len))}}
                    Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "edit-created-fields", on:click = move |_| workbench.edit_created_filters()) { "Edit" }
                }
            }
            row(class = "filter-actions") {
                Button(size = ButtonSize::Xs, class = "save-derived-field", disabled = Signal::derive_local(move || workbench.loading.get() || workbench.derived_busy.get() || workbench.log.get().is_none() || workbench.filter_input_ui.drag.get().is_some() || workbench.filter_inputs.with(|inputs| inputs.iter().all(|input| input.source.is_empty()) || workbench.filter_kind.get() == "rpy" && (inputs.len() != 4 || inputs.iter().any(|input| input.source.is_empty())))),
                    on:click = move |_| workbench.save_filter()) {{move || if workbench.derived_busy.get() { "Computing\u{2026}".to_owned() } else if !workbench.filter_mode.get().is_new() { "Save changes".to_owned() } else if workbench.filter_kind.get() == "rpy" { "Create 3 fields".to_owned() } else if workbench.filter_kind.get() == "magnitude" { "Create magnitude".to_owned() } else { format!("Create {} fields", workbench.filter_inputs.with(Vec::len)) }}}

                if move || !workbench.filter_outputs.with(Vec::is_empty) {
                    Button(size = ButtonSize::Xs, class = "chain-derived-field", disabled = Signal::derive_local(move || workbench.derived_busy.get()), on:click = move |_| {
                        workbench.next_filters();
                    }) { "Add next filter" }
                    Button(size = ButtonSize::Xs, class = "plot-derived-field", disabled = Signal::derive_local(move || workbench.derived_busy.get()), on:click = move |_| {
                        for id in workbench.filter_outputs.get_untracked() { workbench.add(crate::derived::key(id)); } workbench.filters_open.set(false);
                    }) { "Plot outputs" }
                }
            }
        }
    }
}
