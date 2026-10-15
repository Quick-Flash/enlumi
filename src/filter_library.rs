use crate::{filter_library_card::FilterLibraryCardProps, state::Workbench};
use zgui::prelude::*;

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
            }
        }
    }
}
