use crate::{panel::PanelProps, state::Workbench};
use plot::zgui_workspace::*;
use zgui::prelude::*;

#[component]
pub fn PlotWorkspace(workbench: Workbench) -> impl IntoView {
    let group = plot::zgui_link::LinkGroup::new();
    let frequency_group = plot::zgui_link::LinkGroup::new();
    let dock = workbench.dock;
    let cursor = RwSignal::new_local(None);
    let geometry = WorkspaceGeometry::fitted(workbench.layout, workbench.maximized);
    let divider_drag = RwSignal::new_local(None);
    view! {
        WorkspaceCanvas(geometry = geometry, drag = divider_drag) {
            WorkspaceDockOverlay(dock = dock, geometry=Some(geometry))
            for pane in move || workbench.panes.get(), key = |pane| pane.id {
                Panel(workbench = workbench, pane = pane, dock = dock, geometry = geometry, group = group, frequency_group = frequency_group, cursor = cursor)
            }
        }
    }
}
