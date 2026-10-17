use crate::state::Workbench;
use plot::zgui_workspace::{SeriesSourceActions, SeriesSourceProps};
use zgui::prelude::*;

/// A whole saved view uses the same drag gesture as a signal.
#[component]
pub fn ProfileViewSource(
    workbench: Workbench,
    source: String,
    name: String,
    count: usize,
    enabled: bool,
) -> impl IntoView {
    let actions = SeriesSourceActions {
        on_move: UnsyncCallback::new(move |(x, y)| workbench.field_drag_move(x, y)),
        on_drop: UnsyncCallback::new(move |(source, x, y)| workbench.drop_fields(source, x, y)),
        on_cancel: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
        on_finish: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
    };
    view! {
        row(class = "profile-view-source") {
            SeriesSource(id = Signal::stored_local(if enabled { Some(source) } else { None }),
                label = Signal::stored_local(name), count = Signal::stored_local(format!("{count} fields")),
                selected = Signal::stored_local(false), drag = workbench.drag, actions = actions)
        }
    }
}
