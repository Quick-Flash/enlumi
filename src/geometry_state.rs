use crate::{
    geometry::{Geometry, GeometryKind, LineOrientation},
    state::{Pane, Workbench},
};
use kurbo::Point;
use std::collections::BTreeMap;
use zgui::prelude::*;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeometryPart {
    Move,
    Handle(usize),
    Create,
}
#[derive(Clone, Debug)]
pub struct GeometryDrag {
    pub pane: u64,
    pub original: Geometry,
    pub start: Point,
    pub part: GeometryPart,
}
#[derive(Clone, Copy)]
pub struct EditorDrag {
    pub pane: u64,
    pub editor: NodeRef,
    pub boundary: NodeRef,
    pub offset: RwSignal<(f64, f64), LocalStorage>,
    pub start: Point,
    pub original: (f64, f64),
}
#[derive(Clone, Copy)]
pub struct GeometryUi {
    pub items: RwSignal<Vec<Geometry>, LocalStorage>,
    pub selected: RwSignal<Option<u64>, LocalStorage>,
    pub editor_drag: RwSignal<Option<EditorDrag>, LocalStorage>,
    pub editing: RwSignal<bool, LocalStorage>,
    pub tool: RwSignal<Option<GeometryKind>, LocalStorage>,
    pub coordinates: RwSignal<[String; 4], LocalStorage>,
    pub error: RwSignal<String, LocalStorage>,
    pub drag: RwSignal<Option<GeometryDrag>, LocalStorage>,
    pub library_drag: RwSignal<Option<(u64, Point, bool)>, LocalStorage>,
    pub target: RwSignal<Option<u64>, LocalStorage>,
    pub hosts: StoredValue<BTreeMap<u64, NodeRef>, LocalStorage>,
}
impl GeometryUi {
    pub fn new() -> Self {
        Self {
            items: RwSignal::new_local(Vec::new()),
            selected: RwSignal::new_local(None),
            editor_drag: RwSignal::new_local(None),
            editing: RwSignal::new_local(false),
            tool: RwSignal::new_local(None),
            coordinates: RwSignal::new_local(std::array::from_fn(|_| String::new())),
            error: RwSignal::new_local(String::new()),
            drag: RwSignal::new_local(None),
            library_drag: RwSignal::new_local(None),
            target: RwSignal::new_local(None),
            hosts: StoredValue::new_local(BTreeMap::new()),
        }
    }
}
impl Workbench {
    pub fn geometry_item(self, id: u64) -> Option<Geometry> {
        self.geometry
            .items
            .with_untracked(|items| items.iter().find(|item| item.id == id).cloned())
    }
    pub fn select_geometry(self, id: u64) {
        self.cancel_band_source();
        self.cancel_geometry_drag();
        if let Some(item) = self.geometry_item(id) {
            self.geometry.selected.set(Some(id));
            self.geometry
                .coordinates
                .set(item.coordinates.map(|v| v.to_string()));
            self.geometry.error.set(String::new());
            self.geometry.editing.set(true);
            self.geometry.tool.set(None);
            self.filters_open.set(false);
        }
    }
    pub fn arm_geometry(self, kind: GeometryKind) {
        self.cancel_band_source();
        self.cancel_geometry_drag();
        self.geometry.tool.set(Some(kind));
        self.geometry.editing.set(true);
        self.filters_open.set(false);
        self.geometry.error.set(String::new());
    }
    pub fn update_geometry(self, item: Geometry, sync: bool) {
        if !item.valid() {
            return;
        }
        let coordinates = item.coordinates;
        self.geometry.items.update(|items| {
            if let Some(current) = items.iter_mut().find(|current| current.id == item.id) {
                *current = item;
            }
        });
        if sync {
            self.geometry
                .coordinates
                .set(coordinates.map(|v| v.to_string()));
        }
    }
    pub fn geometry_coordinates(self, index: usize, value: String) {
        if self
            .geometry
            .selected
            .get_untracked()
            .and_then(|id| self.geometry_item(id))
            .is_some_and(|item| item.states.is_some())
        {
            self.geometry
                .error
                .set("Source controls state band timing".into());
            return;
        }

        self.geometry
            .coordinates
            .update(|fields| fields[index] = value);
        let fields = self.geometry.coordinates.get_untracked();
        let mut coordinates = [0.0; 4];
        for (i, text) in fields.iter().enumerate() {
            match text.trim().parse::<f64>() {
                Ok(value) if value.is_finite() => coordinates[i] = value,
                _ => {
                    self.geometry
                        .error
                        .set("Coordinates must be finite numbers".into());
                    return;
                }
            }
        }
        if let Some(mut item) = self
            .geometry
            .selected
            .get_untracked()
            .and_then(|id| self.geometry_item(id))
        {
            if item.kind == GeometryKind::Line {
                match item.orientation {
                    LineOrientation::Horizontal => {
                        let y = if index == 3 {
                            coordinates[3]
                        } else {
                            coordinates[1]
                        };
                        coordinates[1] = y;
                        coordinates[3] = y;
                        self.geometry.coordinates.update(|draft| {
                            if index == 3 {
                                draft[1] = draft[3].clone();
                            } else {
                                draft[3] = draft[1].clone();
                            }
                        });
                    }
                    LineOrientation::Vertical => {
                        let x = if index == 2 {
                            coordinates[2]
                        } else {
                            coordinates[0]
                        };
                        coordinates[0] = x;
                        coordinates[2] = x;
                        self.geometry.coordinates.update(|draft| {
                            if index == 2 {
                                draft[0] = draft[2].clone();
                            } else {
                                draft[2] = draft[0].clone();
                            }
                        });
                    }
                    LineOrientation::Free => {}
                }
            }
            item.coordinates = coordinates;
            if item.nonempty() {
                self.update_geometry(item, false);
                self.geometry.error.set(String::new());
            } else {
                self.geometry
                    .error
                    .set("Shape needs distinct corners or endpoints".into());
            }
        }
    }
    pub fn orient_geometry(self, id: u64, orientation: LineOrientation) {
        self.cancel_geometry_drag();
        let Some(mut item) = self
            .geometry_item(id)
            .filter(|item| item.kind == GeometryKind::Line)
        else {
            return;
        };
        let c = &mut item.coordinates;
        let ranges = self.pane().plot.ranges.get_untracked();
        match orientation {
            LineOrientation::Horizontal => {
                let y = c[1] * 0.5 + c[3] * 0.5;
                c[1] = y;
                c[3] = y;
                if c[0] == c[2] {
                    let center = c[0];
                    c[0] = center - ranges.x.span() * 0.15;
                    c[2] = center + ranges.x.span() * 0.15;
                }
            }
            LineOrientation::Vertical => {
                let x = c[0] * 0.5 + c[2] * 0.5;
                c[0] = x;
                c[2] = x;
                if c[1] == c[3] {
                    let center = c[1];
                    c[1] = center - ranges.y.span() * 0.15;
                    c[3] = center + ranges.y.span() * 0.15;
                }
            }
            LineOrientation::Free => {}
        }
        item.orientation = orientation;
        self.update_geometry(item, true);
        self.geometry.error.set(String::new());
    }
    pub fn attach_geometry(self, id: u64, pane: Pane, shown: bool) {
        if self.geometry_item(id).is_some() {
            pane.geometry.update(|items| {
                if shown {
                    items.insert(id);
                } else {
                    items.remove(&id);
                }
            });
            pane.hidden_geometry.update(|items| {
                items.remove(&id);
            });
        }
    }
    pub fn toggle_geometry(self, pane: Pane, id: u64) {
        if !pane.geometry.with_untracked(|items| items.contains(&id)) {
            return;
        }
        pane.hidden_geometry.update(|items| {
            if !items.remove(&id) {
                items.insert(id);
            }
        });
        if pane
            .hidden_geometry
            .with_untracked(|items| items.contains(&id))
            && self.geometry.selected.get_untracked() == Some(id)
            && self.active().get_untracked() == pane.id
        {
            self.finish_geometry_editing();
        }
    }
    pub fn edit_geometry_library(self, id: u64) {
        let active = self.pane();
        let pane = if active.geometry.with_untracked(|ids| ids.contains(&id)) {
            active
        } else {
            self.panes()
                .get_untracked()
                .into_iter()
                .find(|pane| {
                    pane.geometry.with_untracked(|ids| ids.contains(&id))
                        && plot::workspace::plot_visible(
                            pane.id,
                            self.maximized().get_untracked(),
                            self.layout().get_untracked(),
                        )
                })
                .unwrap_or(active)
        };
        self.edit_geometry_on(pane, id);
    }
    pub fn edit_geometry_on(self, pane: Pane, id: u64) {
        self.activate(pane.id);
        self.select_geometry(id);
    }
    pub fn remove_geometry_from(self, pane: Pane, id: u64) {
        if self
            .geometry
            .drag
            .get_untracked()
            .is_some_and(|drag| drag.pane == pane.id && drag.original.id == id)
        {
            self.cancel_geometry_drag();
        }
        self.attach_geometry(id, pane, false);
        if self.active().get_untracked() == pane.id
            && self.geometry.selected.get_untracked() == Some(id)
        {
            self.finish_geometry_editing();
        }
    }
    pub fn delete_geometry(self, id: u64) {
        self.cancel_geometry_drag();
        self.geometry
            .items
            .update(|items| items.retain(|item| item.id != id));
        for pane in self.all_panes() {
            pane.geometry.update(|items| {
                items.remove(&id);
            });
            pane.hidden_geometry.update(|items| {
                items.remove(&id);
            });
        }
        if self.geometry.selected.get_untracked() == Some(id) {
            self.geometry.selected.set(None);
            self.geometry.editing.set(false);
        }
    }
    pub fn duplicate_geometry(self, id: u64) {
        if let Some(mut item) = self.geometry_item(id) {
            item.id = self.id();
            item.name = format!("{} copy {}", item.name, item.id);
            let copy = item.id;
            self.geometry.items.update(|items| items.push(item));
            self.attach_geometry(copy, self.pane(), true);
            self.select_geometry(copy);
        }
    }
    pub fn cancel_geometry_drag(self) {
        if let Some(drag) = self.geometry.drag.get_untracked() {
            if drag.part == GeometryPart::Create {
                self.geometry
                    .items
                    .update(|items| items.retain(|item| item.id != drag.original.id));
                for pane in self.all_panes() {
                    pane.geometry.update(|items| {
                        items.remove(&drag.original.id);
                    });
                    pane.hidden_geometry.update(|items| {
                        items.remove(&drag.original.id);
                    });
                }
                self.geometry.selected.set(None);
            } else {
                self.update_geometry(drag.original, true);
            }
        }
        self.geometry.drag.set(None);
        self.geometry.library_drag.set(None);
        self.geometry.target.set(None);
    }
    pub fn finish_geometry_editing(self) {
        self.finish_geometry_editor(true);
        self.cancel_geometry_drag();
        self.geometry.tool.set(None);
        self.geometry.editing.set(false);
        self.geometry.error.set(String::new());
    }
    pub fn geometry_library_move(
        self,
        position: zgui::geom::Point<zgui::geom::CssPx, zgui::geom::Css>,
    ) {
        let Some((id, start, moving)) = self.geometry.library_drag.get_untracked() else {
            return;
        };
        let point = Point::new(f64::from(position.x.0), f64::from(position.y.0));
        let moving = moving || point.distance(start) > 5.0;
        self.geometry.library_drag.set(Some((id, start, moving)));
        let target = if moving {
            self.panes()
                .get_untracked()
                .into_iter()
                .filter(|pane| {
                    plot::workspace::plot_visible(
                        pane.id,
                        self.maximized().get_untracked(),
                        self.layout().get_untracked(),
                    )
                })
                .find_map(|pane| {
                    self.geometry.hosts.with_value(|hosts| {
                        hosts.get(&pane.id).and_then(|host| {
                            let (x, y) = plot::zgui_renderer::local_position(host, position)?;
                            let bounds = host.window_bounds()?;
                            ((0.0..=f64::from(bounds.size.width.0 / host.scale())).contains(&x)
                                && (0.0..=f64::from(bounds.size.height.0 / host.scale()))
                                    .contains(&y))
                            .then_some(pane.id)
                        })
                    })
                })
        } else {
            None
        };
        self.geometry.target.set(target);
    }
}
