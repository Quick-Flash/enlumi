use crate::state::Workbench;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterEditor(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-editor") {
            text(class = "filter-section-title") { "Shared filter settings" }
            text(class = "muted") { "Same exact settings apply to every source field in batch." }
            Select(value = workbench.filter_kind) {
                SelectTrigger(label = "Filter type", class = "filter-type") { SelectValue(placeholder = "Choose filter") }
                SelectContent(class = "filter-type-surface") {
                    SelectItem(value = "low") { "Low-pass (PT1)" }
                    SelectItem(value = "high") { "High-pass (PT1)" }
                    SelectItem(value = "affine") { "Gain / offset" }
                }
            }
            if move || workbench.filter_kind.get() == "affine" {
                row(class = "filter-parameters") {
                    column { text(class = "heading") { "Gain" } Input(value = workbench.filter_gain, a11y:label = "Filter gain") }
                    column { text(class = "heading") { "Offset" } Input(value = workbench.filter_offset, a11y:label = "Filter offset") }
                }
                text(class = "muted") { "Output = input × gain + offset" }
            } else {
                text(class = "heading") { "Cutoff (Hz)" }
                Input(value = workbench.filter_cutoff, a11y:label = "Filter cutoff in Hz")
                text(class = "muted") { "Causal PT1 using actual timestamps; gaps reset filter state." }
            }
            if move || !workbench.filter_error.get().is_empty() {
                text(class = "filter-error", a11y:role = Role::Alert) {{move || workbench.filter_error.get()}}
            }
            row(class = "filter-actions") {
                Button(size = ButtonSize::Sm, class = "save-derived-field", disabled = Signal::derive_local(move || workbench.loading.get() || workbench.derived_busy.get() || workbench.log.get().is_none() || workbench.filter_inputs.with(Vec::is_empty)),
                    on:click = move |_| workbench.save_filter()) {{move || if workbench.derived_busy.get() { "Computing…".to_owned() } else if workbench.filter_edit.get().is_some() { "Save changes".to_owned() } else { format!("{} {} fields", if workbench.filter_outputs.with(Vec::is_empty) { "Create" } else { "Apply to" }, workbench.filter_inputs.with(|inputs| inputs.len())) }}}
                if move || !workbench.filter_outputs.with(Vec::is_empty) {
                    Button(size = ButtonSize::Sm, class = "chain-derived-field", disabled = Signal::derive_local(move || workbench.derived_busy.get()), on:click = move |_| {
                        workbench.next_filters();
                    }) { "Add next filter" }
                    Button(size = ButtonSize::Sm, class = "plot-derived-field", disabled = Signal::derive_local(move || workbench.derived_busy.get()), on:click = move |_| {
                        for id in workbench.filter_outputs.get_untracked() { workbench.add(crate::derived::key(id)); } workbench.filters_open.set(false);
                    }) { "Plot outputs" }
                }
            }
            if move || workbench.filter_edit.get().is_some() {
                Button(size = ButtonSize::Sm, variant = ButtonVariant::Ghost, class = "delete-derived-field", disabled = Signal::derive_local(move || workbench.derived_busy.get()), on:click = move |_| {
                    if let Some(id) = workbench.filter_edit.get_untracked() {
                        match workbench.delete_filter(id) { Ok(()) => workbench.edit_filter(None, None), Err(error) => workbench.filter_error.set(error) }
                    }
                }) { "Delete field" }
            }
        }
    }
}
