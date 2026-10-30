use super::{DividerDrag, SplitDividerProps};
use crate::workspace::layout::{Divider, Layout, Rect};
use zgui::prelude::*;

pub type PaneGeometry = (Vec<(u64, Rect)>, Vec<Divider>);

/// Viewport measurement independent of content minimum, avoiding resize feedback.
#[derive(Clone, Copy)]
pub struct WorkspaceGeometry {
    pub grid: NodeRef,
    pub size: Signal<(f64, f64), LocalStorage>,
    pub panes: Signal<PaneGeometry, LocalStorage>,
    pub layout: RwSignal<Layout, LocalStorage>,
}
impl WorkspaceGeometry {
    /// Fit all dock geometry into viewport, including windows smaller than pane minimums.
    pub fn fitted(
        layout: RwSignal<Layout, LocalStorage>,
        maximized: RwSignal<Option<u64>, LocalStorage>,
    ) -> Self {
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
            if let Some(id) = maximized.get() {
                plots.push((
                    id,
                    Rect {
                        width,
                        height,
                        ..Rect::default()
                    },
                ));
            } else {
                let layout = layout.get();
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
            layout,
        }
    }

    pub fn new(
        layout: RwSignal<Layout, LocalStorage>,
        maximized: RwSignal<Option<u64>, LocalStorage>,
    ) -> Self {
        let grid = NodeRef::new();
        // Scroll content follows canvas size; observing it feeds the minimum back into itself.
        // Border box measures available viewport independently of its children.
        let grid_bounds = grid.observe_border_box();
        let canvas_size = Signal::derive_local(move || {
            let minimum = if maximized.get().is_some() {
                crate::workspace::layout::MIN_PLOT_SIZE
            } else {
                layout.get().minimum()
            };
            let size = grid_bounds
                .get()
                .map(|bounds| bounds.size)
                .unwrap_or_default();
            (
                f64::from(size.width.0 / grid.scale()).max(minimum.0),
                f64::from(size.height.0 / grid.scale()).max(minimum.1),
            )
        });
        let geometry = Signal::derive_local(move || {
            let (width, height) = canvas_size.get();
            let rect = Rect {
                width,
                height,
                ..Rect::default()
            };
            let mut plots = Vec::new();
            let mut dividers = Vec::new();
            if let Some(id) = maximized.get() {
                plots.push((id, rect));
            } else {
                layout.get().geometry(rect, &mut plots, &mut dividers);
            }
            (plots, dividers)
        });
        Self {
            grid,
            size: canvas_size,
            panes: geometry,
            layout,
        }
    }
}

/// Scrollable positioned pane host with keyboard/pointer split resizing.
#[component]
pub fn WorkspaceCanvas(
    geometry: WorkspaceGeometry,
    drag: DividerDrag,
    children: ChildrenFn,
) -> impl IntoView {
    let canvas_size = geometry.size;
    let panes = geometry.panes;
    view! {
        row(class = "plot-grid", node_ref = geometry.grid) {
            column(class = "split-canvas",
                style:width = move || Some(format!("{}px", canvas_size.get().0)),
                style:height = move || Some(format!("{}px", canvas_size.get().1))) {
                {children.view()}
                for divider in move || panes.get().1, key = |divider| divider.id {
                    SplitDivider(id = divider.id, layout = geometry.layout, drag = drag, canvas_size = canvas_size,
                        divider = Signal::derive_local(move || panes.get().1.into_iter().find(|item| item.id == divider.id)))
                }
            }
        }
    }
}
