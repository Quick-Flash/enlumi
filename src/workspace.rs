use crate::{panel::PanelProps, state::Workbench};
use plot::workspace::layout::Rect;
use plot::zgui_workspace::*;
use zgui::prelude::*;

#[component]
pub fn PlotWorkspace(workbench: Workbench) -> impl IntoView {
    let group = plot::zgui_link::LinkGroup::new();
    let dock = workbench.dock;
    let cursor = RwSignal::new_local(None);
    let geometry = viewport_geometry(workbench);
    let divider_drag = RwSignal::new_local(None);
    view! {
        WorkspaceCanvas(geometry = geometry, drag = divider_drag) {
            WorkspaceDockOverlay(dock = dock, geometry=Some(geometry))
            for pane in move || workbench.panes.get(), key = |pane| pane.id {
                Panel(workbench = workbench, pane = pane, dock = dock, geometry = geometry, group = group, cursor = cursor)
            }
        }
    }
}

fn viewport_geometry(workbench: Workbench) -> WorkspaceGeometry {
    let grid = NodeRef::new();
    let bounds = grid.observe_border_box();
    let size = Signal::derive_local(move || {
        let size = bounds.get().map(|bounds| bounds.size).unwrap_or_default();
        (
            f64::from(size.width.0 / grid.scale()),
            f64::from(size.height.0 / grid.scale()),
        )
    });
    let panes = Signal::derive_local(move || {
        let (width, height) = size.get();
        let mut plots = Vec::new();
        let mut dividers = Vec::new();
        if let Some(id) = workbench.maximized.get() {
            plots.push((
                id,
                Rect {
                    width,
                    height,
                    ..Rect::default()
                },
            ));
        } else {
            let layout = workbench.layout.get();
            let minimum = layout.minimum();
            let canvas = Rect {
                width: width.max(minimum.0).max(1.0),
                height: height.max(minimum.1).max(1.0),
                ..Rect::default()
            };
            layout.geometry(canvas, &mut plots, &mut dividers);
            // Keep shared docking ratios and minimums, then fit geometry to visible viewport.
            let (sx, sy) = (width / canvas.width, height / canvas.height);
            let fit = |rect: &mut Rect| {
                rect.x *= sx;
                rect.y *= sy;
                rect.width *= sx;
                rect.height *= sy;
            };
            for (_, rect) in &mut plots {
                fit(rect);
            }
            for divider in &mut dividers {
                fit(&mut divider.rect);
                divider.span *= if divider.stacked { sy } else { sx };
            }
        }
        (plots, dividers)
    });
    WorkspaceGeometry {
        grid,
        size,
        panes,
        layout: workbench.layout,
    }
}
