use crate::state::Workbench;
use plot::{plot_data::SeriesStyle, workspace::plot_visible};
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
impl Workbench {
    pub fn update_line_drag(self, x: f32, y: f32) {
        let Some(mut drag) = self.line_drag.state.get_untracked() else {
            return;
        };
        drag.moving |= (x - drag.start.0).hypot(y - drag.start.1) >= 5.0;
        self.line_drag.state.set(Some(drag));
        let target = drag.moving.then(|| self.line_drop(x, y)).flatten();
        self.line_drag.target.set(target);
    }
    fn line_drop(self, x: f32, y: f32) -> Option<LineDrop> {
        // Legend rows take priority; use dock visibility so hidden tabs cannot accept drops.
        let layout = self.layout.get_untracked();
        for pane in self.panes.get_untracked() {
            if !plot_visible(pane.id, self.maximized.get_untracked(), layout.clone()) {
                continue;
            }
            let lines = pane.lines.get_untracked();
            for (index, line) in lines.iter().enumerate() {
                let Some(node) = self
                    .line_drag
                    .rows
                    .with_value(|rows| rows.get(&(pane.id, line.id)).copied())
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
                        Some(line.id)
                    } else {
                        lines.get(index + 1).map(|line| line.id)
                    };
                    return Some(LineDrop {
                        pane: pane.id,
                        before,
                    });
                }
            }
        }
        self.hit_plot(x, y)
            .map(|pane| LineDrop { pane, before: None })
    }
    pub fn place_line(self, source: u64, id: u64, target: u64, before: Option<u64>) {
        let (Some(source), Some(target)) = (self.find(source), self.find(target)) else {
            return;
        };
        let Some(mut line) = source
            .lines
            .with_untracked(|lines| lines.iter().find(|line| line.id == id).cloned())
        else {
            return;
        };
        if source.id == target.id {
            if before == Some(id) {
                return;
            }
            source.lines.update(|lines| {
                lines.retain(|line| line.id != id);
                let index = before
                    .and_then(|id| lines.iter().position(|line| line.id == id))
                    .unwrap_or(lines.len());
                lines.insert(index, line);
            });
        } else {
            let key = id.to_string();
            let style = source
                .plot
                .styles
                .with_untracked(|styles| styles.get(&key).copied())
                .unwrap_or_else(|| SeriesStyle::for_id(&key));
            let hidden = source
                .plot
                .hidden
                .with_untracked(|hidden| hidden.contains(&key));
            let drawing = source
                .drawing
                .with_untracked(|drawing| drawing.get(&id).copied().unwrap_or_default());
            line.revision += 1;
            source.remove_line(id);
            target.drawing.update(|settings| {
                settings.insert(id, drawing);
            });
            target.plot.styles.update(|styles| {
                styles.insert(key.clone(), style);
            });
            target.plot.hidden.update(|values| {
                if hidden {
                    values.insert(key);
                }
            });
            target.plot.hover.set(None);
            target.lines.update(|lines| {
                let index = before
                    .and_then(|id| lines.iter().position(|line| line.id == id))
                    .unwrap_or(lines.len());
                lines.insert(index, line);
            });
        }
        self.select(target.id, id);
    }
}
