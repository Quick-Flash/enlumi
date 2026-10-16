use crate::{
    geometry::{Geometry, GeometryKind},
    geometry_state::{GeometryDrag, GeometryPart},
    state::{Pane, Workbench},
};
use kurbo::Point;
use plot::PlotGeometry;
use zgui::prelude::*;
impl Workbench {
    pub fn begin_geometry(self, pane: Pane, screen: Point, geometry: PlotGeometry) -> bool {
        self.cancel_geometry_drag();
        let ranges = pane.plot.ranges.get_untracked();
        let point = Point::new(
            geometry.x_value(screen.x, ranges.x),
            geometry.y_value(screen.y, ranges.y),
        );
        let Some(kind) = self.geometry.tool.get_untracked() else {
            return false;
        };
        let item = Geometry::new(self.id(), kind, point);
        self.geometry.items.update(|items| items.push(item.clone()));
        self.attach_geometry(item.id, pane, true);
        self.select_geometry(item.id);
        self.activate(pane.id);
        self.geometry.drag.set(Some(GeometryDrag {
            pane: pane.id,
            original: item,
            start: point,
            part: GeometryPart::Create,
        }));
        true
    }
    pub fn begin_geometry_handle(
        self,
        pane: Pane,
        screen: Point,
        geometry: PlotGeometry,
        part: GeometryPart,
    ) -> bool {
        self.cancel_geometry_drag();
        let Some(item) = self
            .geometry
            .selected
            .get_untracked()
            .and_then(|id| self.geometry_item(id))
        else {
            return false;
        };
        if !pane.geometry.with_untracked(|ids| ids.contains(&item.id))
            || pane
                .hidden_geometry
                .with_untracked(|ids| ids.contains(&item.id))
        {
            return false;
        }
        let ranges = pane.plot.ranges.get_untracked();
        let start = Point::new(
            geometry.x_value(screen.x, ranges.x),
            geometry.y_value(screen.y, ranges.y),
        );
        if item.states.is_some()
            || matches!(part, GeometryPart::Create)
            || matches!(part,GeometryPart::Handle(index) if index>=if item.kind==GeometryKind::Rectangle{4}else{2})
        {
            return false;
        }
        self.activate(pane.id);
        self.geometry.drag.set(Some(GeometryDrag {
            pane: pane.id,
            original: item,
            start,
            part,
        }));
        true
    }
    pub fn move_geometry(self, pane: Pane, screen: Point, geometry: PlotGeometry) {
        let Some(drag) = self
            .geometry
            .drag
            .get_untracked()
            .filter(|drag| drag.pane == pane.id)
        else {
            return;
        };
        let ranges = pane.plot.ranges.get_untracked();
        let point = Point::new(
            geometry.x_value(screen.x, ranges.x),
            geometry.y_value(screen.y, ranges.y),
        );
        let item = match drag.part {
            GeometryPart::Create => drag.original.resize(
                if drag.original.kind == GeometryKind::Rectangle {
                    2
                } else {
                    1
                },
                point,
            ),
            GeometryPart::Handle(index) => drag.original.resize(index, point),
            GeometryPart::Move => drag.original.moved(point - drag.start),
        };
        self.update_geometry(item, true);
    }
    pub fn end_geometry(self) {
        if self.geometry.drag.get_untracked().is_some_and(|drag| {
            !self
                .geometry_item(drag.original.id)
                .is_some_and(|item| item.nonempty())
        }) {
            self.cancel_geometry_drag();
            if self.geometry.selected.get_untracked().is_none() {
                self.finish_geometry_editing();
            }
        }
        self.geometry.drag.set(None);
    }
}
