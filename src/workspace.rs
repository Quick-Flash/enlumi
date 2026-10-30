use crate::geometry_inspector::GeometryInspectorProps;
use crate::timeline::TimelineStripProps;
use crate::{panel::PanelProps, state::Workbench};
use plot::zgui_workspace::*;
use zgui::prelude::*;

#[component]
pub fn PlotWorkspace(workbench: Workbench) -> impl IntoView {
    view! {column(class="workspace-view-stack") {column(class="workspace-view-pages"){for current in move ||workbench.views.items.with(|views|views.iter().filter(|view|workbench.views.mounted.with(|mounted|mounted.contains(&view.id))).copied().collect::<Vec<_>>()),key=|view|view.id {
        column(class="workspace-view-host",class:hidden=move ||workbench.views.active.get()!=current.id) {
            if move ||workbench.views.active.get()==current.id {
                ViewWorkspace(workbench=workbench.for_view(current.id).expect("View exists"))
            }
        }
    }} TimelineStrip(workbench=workbench)}}
}

#[component]
fn ViewWorkspace(workbench: Workbench) -> impl IntoView {
    let groups = crate::panel::PlotGroups {
        time: plot::zgui_link::LinkGroup::new(),
        frequency: plot::zgui_link::LinkGroup::new(),
        spatial: plot::zgui_link::LinkGroup::new(),
    };
    let dock = workbench.dock();
    let cursor = RwSignal::new_local(None);
    let geometry = WorkspaceGeometry::fitted(workbench.layout(), workbench.maximized());
    let divider_drag = RwSignal::new_local(None);
    let editor_area = NodeRef::new();
    view! {
        WorkspaceCanvas(geometry = geometry, drag = divider_drag) {
            WorkspaceDockOverlay(dock = dock, geometry=Some(geometry))
            for pane in move || workbench.panes().get(), key = |pane| pane.id {
                Panel(workbench = workbench, pane = pane, dock = dock, geometry = geometry, groups=groups, cursor = cursor)
            }
        }
        column(class="geometry-editor-layer",node_ref=editor_area) {
            if move || workbench.views.active.get()==workbench.view().id && workbench.geometry.editing.get() && workbench.geometry.tool.get().is_none() {
                for id in move || workbench.geometry.selected.get().into_iter().collect::<Vec<_>>(),key=|id|*id {
                    GeometryInspector(workbench=workbench,id=id,boundary=editor_area)
                }
            }
        }
    }
}
