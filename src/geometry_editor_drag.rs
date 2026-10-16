use crate::{geometry_state::EditorDrag, state::Workbench};
use kurbo::Point;
use zgui::prelude::*;
impl Workbench {
    pub fn begin_geometry_editor(
        self,
        editor: NodeRef,
        offset: RwSignal<(f64, f64), LocalStorage>,
        position: zgui::geom::Point<zgui::geom::CssPx, zgui::geom::Css>,
    ) {
        self.geometry.editor_drag.set(Some(EditorDrag {
            pane: self.active.get_untracked(),
            editor,
            offset,
            start: Point::new(f64::from(position.x.0), f64::from(position.y.0)),
            original: offset.get_untracked(),
        }));
    }
    pub fn move_geometry_editor(
        self,
        pane: u64,
        position: zgui::geom::Point<zgui::geom::CssPx, zgui::geom::Css>,
    ) {
        let Some(drag) = self
            .geometry
            .editor_drag
            .get_untracked()
            .filter(|drag| drag.pane == pane)
        else {
            return;
        };
        let mut next = (
            drag.original.0 + f64::from(position.x.0) - drag.start.x,
            drag.original.1 + f64::from(position.y.0) - drag.start.y,
        );
        let host = self
            .geometry
            .hosts
            .with_value(|hosts| hosts.get(&pane).copied());
        if let (Some(bounds), Some(host)) = (drag.editor.window_bounds(), host)
            && let Some(plot) = host.window_bounds()
        {
            let scale = f64::from(host.scale());
            let old = drag.offset.get_untracked();
            let origin = (
                f64::from(bounds.origin.x.0) / scale - old.0,
                f64::from(bounds.origin.y.0) / scale - old.1,
            );
            let min = (
                f64::from(plot.origin.x.0) / scale - origin.0,
                f64::from(plot.origin.y.0) / scale - origin.1,
            );
            let max = (
                f64::from(plot.origin.x.0 + plot.size.width.0 - bounds.size.width.0) / scale
                    - origin.0,
                f64::from(plot.origin.y.0 + plot.size.height.0 - bounds.size.height.0) / scale
                    - origin.1,
            );
            next = (
                next.0.clamp(min.0, max.0.max(min.0)),
                next.1.clamp(min.1, max.1.max(min.1)),
            );
        }
        drag.offset.set(next);
    }
    pub fn finish_geometry_editor(self, cancel: bool) {
        if cancel && let Some(drag) = self.geometry.editor_drag.get_untracked() {
            drag.offset.try_set(drag.original);
        }
        self.geometry.editor_drag.set(None);
    }
}
