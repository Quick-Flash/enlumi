use crate::action_button::ActionButtonProps;
use crate::{
    filter_editor::FilterEditorProps, filter_library::FilterLibraryProps,
    filter_mode_banner::FilterModeBannerProps, filter_sources::FilterSourcesProps,
    script_editor::ScriptEditorProps, state::Workbench,
};
use zgui::prelude::*;

#[component]
pub fn FilterWorkspace(workbench: Workbench) -> impl IntoView {
    view! {
        column(class = "filter-workspace") {
            column(class = "filter-workspace-content") {
                row(class = "filter-page-heading") {
                    text(class = "filter-page-title") { "Filters" }
                    ActionButton(class = "filters-done", on:click = move |_| { workbench.filters_open.set(false); workbench.finish_field_drag(); }) { "Back to plots" }
                }
                row(class = "filter-workspace-body") {
                    FilterLibrary(workbench = workbench)
                    column(class = "filter-composer") {
                        FilterModeBanner(workbench = workbench)
                        row(class = "filter-composer-body") {
                            column(class = "filter-batch", class:script-batch = move || workbench.filter_kind.get() == "rust") {
                                FilterSources(workbench = workbench)
                                if move || workbench.filter_kind.get() == "rust" { ScriptEditor(workbench = workbench) }
                            }
                            FilterEditor(workbench = workbench)
                        }
                    }
                }
            }
        }
    }
}
