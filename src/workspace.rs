use crate::{panel::PanelProps, state::Workbench};
use plot::zgui_workspace::*;
use zgui::prelude::*;

#[component]
pub fn PlotWorkspace(workbench: Workbench) -> impl IntoView {
    view! {column(class="workspace-view-stack") {for current in move ||workbench.views.items.with(|views|views.iter().filter(|view|workbench.views.mounted.with(|mounted|mounted.contains(&view.id))).copied().collect::<Vec<_>>()),key=|view|view.id {
        column(class="workspace-view-host",class:hidden=move ||workbench.views.active.get()!=current.id) {
            ViewWorkspace(workbench=workbench.for_view(current.id).expect("View exists"))
        }
    }}}
}

#[component]
fn ViewWorkspace(workbench: Workbench) -> impl IntoView {
    let group = plot::zgui_link::LinkGroup::new();
    let frequency_group = plot::zgui_link::LinkGroup::new();
    let dock = workbench.dock();
    let cursor = RwSignal::new_local(None);
    let geometry = WorkspaceGeometry::fitted(workbench.layout(), workbench.maximized());
    let divider_drag = RwSignal::new_local(None);
    view! {
        WorkspaceCanvas(geometry = geometry, drag = divider_drag) {
            WorkspaceDockOverlay(dock = dock, geometry=Some(geometry))
            for pane in move || workbench.panes().get(), key = |pane| pane.id {
                Panel(workbench = workbench, pane = pane, dock = dock, geometry = geometry, group = group, frequency_group = frequency_group, cursor = cursor)
            }
        }
    }
}
