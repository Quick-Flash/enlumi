use crate::{derived::FilterInput, state::Workbench};
use std::{collections::BTreeMap, time::Duration};
use zgui::prelude::*;
use zgui::view::FrameHandle;
use zgui::vocab::Timestamp;

#[derive(Clone, Copy)]
pub struct FilterInputUi {
    pub slots: [NodeRef; 4],
    pub rows: StoredValue<BTreeMap<String, NodeRef>, LocalStorage>,
    pub hover: RwSignal<Option<usize>, LocalStorage>,
    pub drag: RwSignal<Option<InputDrag>, LocalStorage>,
    pub rebase: RwSignal<bool, LocalStorage>,
    pub clock: StoredValue<Option<Timers>, LocalStorage>,
    frame: StoredValue<Option<FrameHandle>, LocalStorage>,
}
impl FilterInputUi {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| NodeRef::new()),
            rows: StoredValue::new_local(BTreeMap::new()),
            hover: RwSignal::new_local(None),
            drag: RwSignal::new_local(None),
            rebase: RwSignal::new_local(false),
            clock: StoredValue::new_local(None),
            frame: StoredValue::new_local(None),
        }
    }
}
#[derive(Clone)]
pub struct InputDrag {
    pub from: usize,
    pub moving: bool,
    pub settling: bool,
    start: (f32, f32),
    pointer: (f32, f32),
    target: Option<usize>,
    rects: Vec<(f32, f32, f32, f32)>,
    original: Vec<FilterInput>,
    swap: bool,
    clip: Option<(f32, f32, f32, f32)>,
}
pub fn empty_input() -> FilterInput {
    FilterInput {
        source: String::new(),
        name: String::new(),
    }
}
fn bounds(node: NodeRef) -> Option<(f32, f32, f32, f32)> {
    let rect = node.window_bounds()?;
    let scale = node.scale();
    Some((
        rect.origin.x.0 / scale,
        rect.origin.y.0 / scale,
        rect.size.width.0 / scale,
        rect.size.height.0 / scale,
    ))
}
impl InputDrag {
    fn hit(&self, x: f32, y: f32) -> Option<usize> {
        if self
            .clip
            .is_some_and(|r| x < r.0 || x > r.0 + r.2 || y < r.1 || y > r.1 + r.3)
        {
            return None;
        }
        let first = self.rects.first()?;
        let last = self.rects.last()?;
        if x < first.0 || x > first.0 + first.2 || y < first.1 || y > last.1 + last.3 {
            return None;
        }
        self.rects
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (y - a.1 - a.3 / 2.0)
                    .abs()
                    .total_cmp(&(y - b.1 - b.3 / 2.0).abs())
            })
            .map(|(index, _)| index)
    }
    fn order(&self) -> Vec<usize> {
        let mut order: Vec<_> = (0..self.original.len()).collect();
        if let Some(target) = self.target {
            if self.swap {
                order.swap(self.from, target);
            } else {
                let moved = order.remove(self.from);
                order.insert(target, moved);
            }
        }
        order
    }
    fn offset(&self, index: usize) -> f32 {
        if !self.moving {
            return 0.0;
        }
        if index == self.from && !self.settling {
            return self.pointer.1 - self.start.1;
        }
        let target = self
            .order()
            .iter()
            .position(|&source| source == index)
            .unwrap_or(index);
        self.rects[target].1 - self.rects[index].1
    }
}
impl Workbench {
    pub fn quaternion_slot_at(self, x: f32, y: f32) -> Option<usize> {
        if self.filter_kind.get_untracked() != "rpy" {
            return None;
        }
        self.filter_input_ui.slots.iter().position(|&node| {
            bounds(node).is_some_and(|r| x >= r.0 && x <= r.0 + r.2 && y >= r.1 && y <= r.1 + r.3)
        })
    }
    pub fn start_filter_input_drag(self, index: usize, x: f32, y: f32) {
        if self.filter_input_ui.drag.get_untracked().is_some() {
            return;
        }
        let kind = self.filter_kind.get_untracked();
        if !matches!(kind.as_str(), "rpy" | "rust") || self.script_busy.get_untracked() {
            return;
        }
        let mut original = self.filter_inputs.get_untracked();
        if original
            .get(index)
            .is_none_or(|input| input.source.is_empty())
        {
            return;
        }
        let rects: Option<Vec<_>> = if kind == "rpy" {
            original.resize_with(4, empty_input);
            self.filter_input_ui
                .slots
                .iter()
                .map(|&node| bounds(node))
                .collect()
        } else {
            self.filter_input_ui.rows.with_value(|rows| {
                original
                    .iter()
                    .map(|input| rows.get(&input.source).copied().and_then(bounds))
                    .collect()
            })
        };
        let Some(rects) = rects else {
            return;
        };
        self.filter_input_ui.drag.set(Some(InputDrag {
            from: index,
            moving: false,
            settling: false,
            start: (x, y),
            pointer: (x, y),
            target: Some(index),
            rects,
            original,
            swap: kind == "rpy",
            clip: bounds(self.filter_drop),
        }));
    }
    pub fn move_filter_input_drag(self, x: f32, y: f32) {
        self.filter_input_ui.drag.update(|drag| {
            if let Some(drag) = drag
                && !drag.settling
            {
                drag.moving |= (x - drag.start.0).hypot(y - drag.start.1) >= 5.0;
                drag.pointer = (x, y);
                drag.target = drag.hit(x, y);
            }
        });
        self.filter_input_ui
            .hover
            .set(self.filter_input_ui.drag.with_untracked(|drag| {
                drag.as_ref()
                    .filter(|drag| drag.moving)
                    .and_then(|drag| drag.target)
            }));
    }
    pub fn filter_input_offset(self, index: usize) -> String {
        format!(
            "translateY({}px)",
            self.filter_input_ui.drag.with(|drag| drag
                .as_ref()
                .filter(|drag| index < drag.original.len())
                .map_or(0.0, |drag| drag.offset(index)))
        )
    }
    pub fn finish_filter_input_drag(self, cancel: bool) {
        let Some(mut drag) = self.filter_input_ui.drag.get_untracked() else {
            return;
        };
        if drag.settling {
            return;
        }
        if !drag.moving {
            self.cancel_filter_input_drag();
            return;
        }
        if cancel {
            drag.target = None;
        }
        drag.settling = true;
        self.filter_input_ui.drag.set(Some(drag));
        self.settle_filter_input_drag(None);
    }
    fn settle_filter_input_drag(self, start: Option<Timestamp>) {
        let Some(clock) = self.filter_input_ui.clock.get_value() else {
            self.cancel_filter_input_drag();
            return;
        };
        self.filter_input_ui
            .frame
            .set_value(Some(clock.request_frame(move |at| {
                let start = start.unwrap_or(at);
                if at.saturating_since(start) < Duration::from_millis(170) {
                    self.settle_filter_input_drag(Some(start));
                    return;
                }
                if let Some(drag) = self.filter_input_ui.drag.get_untracked()
                    && drag.target.is_some()
                {
                    self.filter_input_ui.rebase.set(true);
                    let current = self.filter_inputs.get_untracked();
                    self.filter_inputs.set(
                        drag.order()
                            .into_iter()
                            .map(|index| {
                                current
                                    .iter()
                                    .find(|input| input.source == drag.original[index].source)
                                    .unwrap_or(&drag.original[index])
                                    .clone()
                            })
                            .collect(),
                    );
                }
                self.filter_input_ui.drag.set(None);
                self.filter_input_ui.hover.set(None);
                let next = self.filter_input_ui.clock.get_value().unwrap();
                self.filter_input_ui
                    .frame
                    .set_value(Some(next.request_frame(move |_| {
                        self.filter_input_ui.rebase.set(false);
                        self.filter_input_ui.frame.set_value(None);
                    })));
            })));
    }
    pub fn cancel_filter_input_drag(self) {
        self.filter_input_ui.frame.set_value(None);
        self.filter_input_ui.drag.set(None);
        self.filter_input_ui.hover.set(None);
        self.filter_input_ui.rebase.set(false);
    }
    pub fn remove_filter_input(self, index: usize) {
        self.cancel_filter_input_drag();
        self.filter_inputs.update(|inputs| {
            if index < inputs.len() {
                if self.filter_kind.get_untracked() == "rpy" {
                    inputs[index] = empty_input();
                } else {
                    inputs.remove(index);
                }
            }
        });
        if !self.combined_filter_untracked() {
            let sources: Vec<_> = self
                .filter_inputs
                .get_untracked()
                .into_iter()
                .map(|input| input.source)
                .collect();
            let fields = self.derived.get_untracked();
            self.filter_outputs.update(|outputs| {
                outputs.retain(|id| {
                    fields.iter().any(|field| {
                        field.id == *id
                            && field.sources.iter().all(|source| sources.contains(source))
                    })
                })
            });
        }
    }
    pub fn set_filter_kind(self, kind: String) {
        self.cancel_filter_input_drag();
        self.filter_inputs
            .update(|inputs| inputs.retain(|input| !input.source.is_empty()));
        if kind == "rpy" && self.filter_inputs.with_untracked(Vec::len) > 4 {
            self.filter_inputs.update(|inputs| inputs.truncate(4));
            self.filter_error
                .set("Quaternion has four slots; extra source fields were not assigned".into());
        }
        self.filter_kind.set(kind);
    }
}
