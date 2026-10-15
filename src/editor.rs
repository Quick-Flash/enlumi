use crate::state::{Pane, Workbench};
use plot::zgui_series::SeriesEditorProps;
use zgui::prelude::*;
#[component]
pub fn Editor(
    workbench: Workbench,
    pane: Pane,
    #[prop(optional)] on_filters: Option<UnsyncCallback<()>>,
) -> impl IntoView {
    view! { SeriesEditor(pane = pane.display(), actions = workbench.series_actions(), on_extra = on_filters) }
}
