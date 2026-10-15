use crate::{
    filter_editor::FilterEditorProps, filter_library::FilterLibraryProps,
    filter_mode_banner::FilterModeBannerProps, filter_sources::FilterSourcesProps,
    state::Workbench,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn FilterWorkspace(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-workspace") {
            column(class = "filter-workspace-content") {
                row(class = "filter-page-heading") {
                    text(class = "filter-page-title") { "Filters" }
                    Button(size = ButtonSize::Xs, class = "filters-done", on:click = move |_| { workbench.filters_open.set(false); workbench.finish_field_drag(); }) { "Back to plots" }
                }
                row(class = "filter-workspace-body") {
                    FilterLibrary(workbench = workbench)
                    column(class = "filter-composer") {
                        FilterModeBanner(workbench = workbench)
                        row(class = "filter-composer-body") {
                            column(class = "filter-batch") { FilterSources(workbench = workbench) }
                            FilterEditor(workbench = workbench)
                        }
                    }
                }
            }
        }
    }
}
