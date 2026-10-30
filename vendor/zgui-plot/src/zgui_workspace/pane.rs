use super::{DockOverlayProps, DockPane, DockState, WorkspaceGeometry};
use crate::workspace::{plot_rect, plot_visible};
use zgui::prelude::*;
use zgui::reactive::{Callable, UnsyncCallback};

/// Positioned pane with selection, maximized visibility, and docking feedback.
#[component]
pub fn WorkspacePane(
    pane: DockPane,
    dock: DockState,
    geometry: WorkspaceGeometry,
    active: RwSignal<u64, LocalStorage>,
    maximized: RwSignal<Option<u64>, LocalStorage>,
    on_select: UnsyncCallback<u64>,
    children: ChildrenFn,
) -> impl IntoView {
    let rect = move || plot_rect(&geometry.panes.get().0, pane.id);
    view! {
        column(class = "workbench-plot", node_ref = pane.node,
            class:hidden = move || !plot_visible(pane.id, maximized.get(), dock.layout.get()),
            class:tab-drop-target = move || dock.target.get().is_some_and(|target| target.0 == pane.id && dock.group_target.get().is_none()),
            style:left = move || Some(format!("{}px", rect().x)),
            style:top = move || Some(format!("{}px", rect().y)),
            style:width = move || Some(format!("{}px", rect().width)),
            style:height = move || Some(format!("{}px", rect().height)),
            class:workbench-plot-active = move || active.get() == pane.id,
            on:pointer_down = move |_| on_select.run(pane.id)) {
            {children.view()}
            DockOverlay(id=pane.id,dock=dock)
        }
    }
}
