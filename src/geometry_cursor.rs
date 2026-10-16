use crate::{
    geometry::{Geometry, GeometryKind, LineOrientation},
    geometry_state::GeometryPart,
    state::Workbench,
};
use zgui::prelude::*;
pub fn handle_cursor(item: &Geometry, index: Option<usize>, dragging: bool) -> &'static str {
    if let Some(index) = index {
        if item.kind == GeometryKind::Band {
            return "ew-resize";
        }
        if item.kind == GeometryKind::Line {
            return match item.orientation {
                LineOrientation::Horizontal => "ew-resize",
                LineOrientation::Vertical => "ns-resize",
                LineOrientation::Free => "crosshair",
            };
        }
        let handles = item.handles();
        let a = handles[index];
        let b = handles[(index + 2) % 4];
        return if (a.x - b.x).is_sign_positive() == (a.y - b.y).is_sign_positive() {
            "nesw-resize"
        } else {
            "nwse-resize"
        };
    }
    if item.kind == GeometryKind::Band {
        return "ew-resize";
    }
    if item.kind == GeometryKind::Line && item.extend {
        match item.orientation {
            LineOrientation::Horizontal => return "ns-resize",
            LineOrientation::Vertical => return "ew-resize",
            LineOrientation::Free => {}
        }
    }
    if dragging { "grabbing" } else { "grab" }
}
impl Workbench {
    pub fn geometry_cursor(self, pane: u64) -> Option<String> {
        if self
            .geometry
            .editor_drag
            .get()
            .is_some_and(|drag| drag.pane == pane)
        {
            return Some("grabbing".into());
        }
        self.geometry
            .drag
            .get()
            .filter(|drag| drag.pane == pane)
            .map(|drag| {
                match drag.part {
                    GeometryPart::Create => "crosshair",
                    GeometryPart::Move => handle_cursor(&drag.original, None, true),
                    GeometryPart::Handle(index) => handle_cursor(&drag.original, Some(index), true),
                }
                .into()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursors_follow_actual_axes_active_drag_and_rectangle_corner_order() {
        let mut item = Geometry::new(1, GeometryKind::Rectangle, kurbo::Point::ZERO);
        item.coordinates = [0., 10., 10., 0.];
        assert_eq!(handle_cursor(&item, Some(0), false), "nwse-resize");
        item.coordinates = [10., 10., 0., 0.];
        assert_eq!(handle_cursor(&item, Some(0), false), "nesw-resize");
        assert_eq!(handle_cursor(&item, None, false), "grab");
        assert_eq!(handle_cursor(&item, None, true), "grabbing");
        item.kind = GeometryKind::Line;
        item.orientation = LineOrientation::Horizontal;
        assert_eq!(handle_cursor(&item, None, true), "ns-resize");
        assert_eq!(handle_cursor(&item, Some(1), false), "ew-resize");
        item.orientation = LineOrientation::Vertical;
        assert_eq!(handle_cursor(&item, None, true), "ew-resize");
        assert_eq!(handle_cursor(&item, Some(1), false), "ns-resize");
        item.kind = GeometryKind::Band;
        assert_eq!(handle_cursor(&item, None, true), "ew-resize");
    }
}
