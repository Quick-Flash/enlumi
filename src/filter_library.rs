use crate::{filter_library_card::FilterLibraryCardProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterLibrary(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "derived-library") {
            row(class = "derived-library-heading") {
                text(class = "heading") { "Saved fields" }
                text(class = "derived-library-count") {{move || workbench.derived.with(Vec::len).to_string()}}
            }
            text(class = "muted") { "Click to edit; click again to finish." }
            column(class = "derived-library-list") {
                for field in move || workbench.derived.get(), key = |field| field.id {
                    FilterLibraryCard(workbench = workbench, id = field.id)
                }
                if move || workbench.derived.with(Vec::is_empty) {
                    text(class = "muted derived-empty") { "No saved fields yet." }
                }
                Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "new-derived-field", class:new-field-selected = move || workbench.filter_mode.get().is_new(),
                a11y:label = move || zgui::vocab::SharedString::from(if workbench.filter_mode.get().is_new() { "New field, selected. Creating derived outputs." } else { "New field. Click to create derived outputs." }), on:click = move |_| {
                    if !workbench.filter_mode.get_untracked().is_new() { workbench.finish_filter_editing(); }
                }) {
                    row(class = "derived-card-main") {
                        text(class = "derived-library-name new-field-title") { "New field" }
                        text(class = "derived-filter-badge new-field-badge") {{move || if workbench.filter_mode.get().is_new() { "DRAFT" } else { "NEW" }}}
                    }
                    text(class = "derived-library-detail new-field-detail") {{move || if workbench.filter_mode.get().is_new() { "Creating derived outputs" } else { "Create derived outputs" }}}
                }
            }
        }
    }
}
