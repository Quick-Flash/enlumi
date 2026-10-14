use crate::{
    filter_editor::FilterEditorProps, filter_sources::FilterSourcesProps, state::Workbench,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterWorkspace(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-workspace") {
                column(class = "filter-workspace-content") {
                    row(class = "filter-page-heading") {
                        text(class = "filter-page-title") { "Filters & derived fields" }
                        Button(size = ButtonSize::Sm, class = "filters-done", on:click = move |_| { workbench.filters_open.set(false); workbench.finish_field_drag(); }) { "Back to plots" }
                    }
                    text(class = "muted") { "Build named stages. Raw field → filtered field → next filtered field." }
                    row(class = "filter-workspace-body") {
                        column(class = "derived-library") {
                            column(class = "derived-library-list") {
                                for field in move || workbench.derived.get(), key = |field| field.id {
                                    Button(size = ButtonSize::Sm, class = "derived-library-item", class:chosen = move || workbench.filter_outputs.with(|outputs| outputs.contains(&field.id)), on:click = move |_| workbench.edit_filter(Some(field.id), None)) {
                                        column {
                                            text(class = "derived-library-name") {{move || workbench.derived.with(|fields| fields.iter().find(|current| current.id == field.id).map_or_else(String::new, |field| field.name.clone()))}}
                                            text(class = "muted") {{move || workbench.derived.with(|fields| fields.iter().find(|current| current.id == field.id).map_or_else(String::new, |field| field.filter.label()))}}
                                        }
                                    }
                                }
                                if move || workbench.derived.with(Vec::is_empty) { text(class = "muted derived-empty") { "No derived fields yet. Choose source and create first stage." } }
                            }
                        }
                        column(class = "filter-batch") { FilterSources(workbench = workbench) }
                        FilterEditor(workbench = workbench)
                    }
                }
        }
    }
}
