use super::{SeriesActions, SeriesPane};
use std::collections::BTreeMap;
use zgui::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub struct DragLine {
    pub pane: u64,
    pub line: u64,
    pub start: (f32, f32),
    pub moving: bool,
}
#[derive(Clone, Copy, PartialEq)]
pub struct LineDrop {
    pub pane: u64,
    pub before: Option<u64>,
}
#[derive(Clone, Copy)]
pub struct LegendDrag {
    pub state: RwSignal<Option<DragLine>, LocalStorage>,
    pub target: RwSignal<Option<LineDrop>, LocalStorage>,
    pub rows: StoredValue<BTreeMap<(u64, u64), NodeRef>, LocalStorage>,
}
impl Default for LegendDrag {
    fn default() -> Self {
        Self::new()
    }
}
impl LegendDrag {
    pub fn new() -> Self {
        Self {
            state: RwSignal::new_local(None),
            target: RwSignal::new_local(None),
            rows: StoredValue::new_local(BTreeMap::new()),
        }
    }
    pub fn cancel(self) {
        self.state.set(None);
        self.target.set(None);
    }
}
/// Measured drop feedback. Acceptance and membership remain consumer-owned.
#[derive(Clone, Copy)]
pub struct LegendHost {
    pub drag: LegendDrag,
    pub panes: Signal<Vec<SeriesPane>, LocalStorage>,
    pub visible: UnsyncCallback<u64, bool>,
    pub hit_plot: UnsyncCallback<(f32, f32), Option<u64>>,
    pub accepts: UnsyncCallback<(u64, u64, u64), bool>,
    pub actions: SeriesActions,
    /// Optional consumer order including custom legend entries.
    pub order: Option<UnsyncCallback<u64, Vec<u64>>>,
}
impl LegendHost {
    pub fn keys(self, pane: SeriesPane) -> Vec<u64> {
        self.order.map_or_else(
            || {
                pane.lines
                    .with(|lines| lines.iter().map(|line| line.id).collect())
            },
            |order| order.run(pane.id),
        )
    }
    pub fn update(self, x: f32, y: f32) {
        let Some(mut drag) = self.drag.state.get_untracked() else {
            return;
        };
        drag.moving |= (x - drag.start.0).hypot(y - drag.start.1) >= 5.0;
        self.drag.state.set(Some(drag));
        let target = drag
            .moving
            .then(|| self.line_drop(x, y))
            .flatten()
            .filter(|target| self.accepts.run((drag.pane, drag.line, target.pane)));
        self.drag.target.set(target);
    }
    fn line_drop(self, x: f32, y: f32) -> Option<LineDrop> {
        // Legend rows take priority; use dock visibility so hidden tabs cannot accept drops.
        for pane in self.panes.get_untracked() {
            if !self.visible.run(pane.id) {
                continue;
            }
            let lines = self.keys(pane);
            for (index, line) in lines.iter().enumerate() {
                let Some(node) = self
                    .drag
                    .rows
                    .with_value(|rows| rows.get(&(pane.id, *line)).copied())
                else {
                    continue;
                };
                let Some(bounds) = node.window_bounds() else {
                    continue;
                };
                let (px, py) = (x * node.scale(), y * node.scale());
                if px >= bounds.origin.x.0
                    && px < bounds.origin.x.0 + bounds.size.width.0
                    && py >= bounds.origin.y.0
                    && py < bounds.origin.y.0 + bounds.size.height.0
                {
                    let before = if py < bounds.origin.y.0 + bounds.size.height.0 / 2.0 {
                        Some(*line)
                    } else {
                        lines.get(index + 1).copied()
                    };
                    return Some(LineDrop {
                        pane: pane.id,
                        before,
                    });
                }
            }
        }
        self.hit_plot
            .run((x, y))
            .map(|pane| LineDrop { pane, before: None })
    }
}
