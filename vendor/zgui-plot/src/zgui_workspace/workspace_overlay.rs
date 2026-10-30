use super::{DockLayoutPreviewProps, DockState, GroupDockOverlayProps, WorkspaceGeometry};
use zgui::prelude::*;

/// Four-direction guides for panes and contiguous bands; no outer-edge strips.
#[component]
pub fn WorkspaceDockOverlay(
    dock: DockState,
    #[prop(optional)] geometry: Option<WorkspaceGeometry>,
) -> impl IntoView {
    let size = geometry.map_or_else(
        || Signal::derive_local(move || dock.viewport_size()),
        |g| g.size,
    );
    view! {
        if move || dock.drag.get().is_some_and(|drag|drag.3) {
            column(class="workspace-dock-overlay") { DockLayoutPreview(dock=dock,size=size) GroupDockOverlay(dock=dock) }
        }
    }
}
