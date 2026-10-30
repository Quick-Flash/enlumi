use super::{DockPreviewPaneProps, DockState};
use crate::workspace::layout::Rect;
use zgui::prelude::*;

#[component]
pub fn DockLayoutPreview(dock: DockState, size: Signal<(f64, f64), LocalStorage>) -> impl IntoView {
    let panes = Memo::new(move |_| {
        let Some((_, tree)) = dock.preview.get() else {
            return vec![];
        };
        let (width, height) = size.get();
        let minimum = tree.minimum();
        let canvas = Rect {
            width: width.max(minimum.0).max(1.0),
            height: height.max(minimum.1).max(1.0),
            ..Rect::default()
        };
        let mut plots = vec![];
        tree.geometry(canvas, &mut plots, &mut vec![]);
        plots.retain(|(id, _)| tree.is_visible(*id));
        for (_, rect) in &mut plots {
            rect.x *= width / canvas.width;
            rect.width *= width / canvas.width;
            rect.y *= height / canvas.height;
            rect.height *= height / canvas.height;
        }
        plots
    });
    view! {
        if move || !panes.get().is_empty() {
            column(class="dock-layout-preview") {
                for pane in move || panes.get(),key=|pane|pane.0 {
                    DockPreviewPane(id=pane.0,panes=panes,dock=dock)
                }
            }
        }
    }
}
