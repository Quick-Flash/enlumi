//! Resize one divider, pushing neighboring dividers only after panes reach minimum size.
use super::layout::{Layout, MIN_PLOT_SIZE, Rect};
use std::collections::BTreeMap;

impl Layout {
    pub fn resize_divider(&mut self, target: u64, value: f64, viewport: (f64, f64)) {
        if !value.is_finite() {
            return;
        }
        let minimum = self.minimum();
        let rect = Rect {
            width: viewport.0.max(minimum.0),
            height: viewport.1.max(minimum.1),
            ..Rect::default()
        };
        let mut plots = Vec::new();
        let mut dividers = Vec::new();
        self.geometry(rect, &mut plots, &mut dividers);
        let Some(selected) = dividers.iter().position(|divider| divider.id == target) else {
            return;
        };
        let divider = &dividers[selected];
        let axis = divider.stacked;
        let indices = dividers
            .iter()
            .enumerate()
            .map(|(index, divider)| (divider.id, index))
            .collect();
        let mut positions: Vec<_> = dividers
            .iter()
            .map(|divider| {
                if divider.stacked {
                    divider.rect.y
                } else {
                    divider.rect.x
                }
            })
            .collect();
        let start = positions.len();
        let end = start + 1;
        positions.extend(if axis {
            [rect.y - 6.0, rect.y + rect.height]
        } else {
            [rect.x - 6.0, rect.x + rect.width]
        });
        let mut constraints = Vec::new();
        edges(self, axis, start, end, &indices, &mut constraints);
        let spacing = if axis {
            MIN_PLOT_SIZE.1
        } else {
            MIN_PLOT_SIZE.0
        } + 6.0;
        let mut lower = vec![f64::NEG_INFINITY; positions.len()];
        let mut upper = vec![f64::INFINITY; positions.len()];
        lower[start] = positions[start];
        upper[end] = positions[end];
        for _ in 0..positions.len() {
            for &(a, b) in &constraints {
                lower[b] = lower[b].max(lower[a] + spacing);
                upper[a] = upper[a].min(upper[b] - spacing);
            }
        }
        let original = positions[selected];
        positions[selected] = (original + (value - divider.ratio) * divider.span)
            .clamp(lower[selected], upper[selected]);
        let forward = positions[selected] >= original;
        for _ in 0..positions.len() {
            let mut moved = false;
            for &(a, b) in &constraints {
                if positions[b] - positions[a] < spacing - 1e-9 {
                    if forward {
                        positions[b] = positions[a] + spacing;
                    } else {
                        positions[a] = positions[b] - spacing;
                    }
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        let pins = dividers
            .iter()
            .enumerate()
            .map(|(index, divider)| (divider.id, positions[index]))
            .collect();
        apply(self, rect, &pins);
    }
}

// Use tree boundaries rather than matching coordinates: separate aligned dividers stay independent.
fn edges(
    layout: &Layout,
    axis: bool,
    start: usize,
    end: usize,
    indices: &BTreeMap<u64, usize>,
    constraints: &mut Vec<(usize, usize)>,
) {
    match layout {
        Layout::Empty => {}
        Layout::Plot(_) | Layout::Tabs { .. } => constraints.push((start, end)),
        Layout::Split {
            id,
            stacked,
            first,
            second,
            ..
        } => {
            if *stacked == axis {
                edges(first, axis, start, indices[id], indices, constraints);
                edges(second, axis, indices[id], end, indices, constraints);
            } else {
                edges(first, axis, start, end, indices, constraints);
                edges(second, axis, start, end, indices, constraints);
            }
        }
    }
}
fn apply(layout: &mut Layout, rect: Rect, pins: &BTreeMap<u64, f64>) {
    if let Layout::Split {
        id,
        stacked,
        ratio,
        first,
        second,
    } = layout
    {
        let span = (if *stacked { rect.height } else { rect.width } - 6.0).max(0.0);
        let size = pins[id] - if *stacked { rect.y } else { rect.x };
        *ratio = size / span.max(1.0);
        let (a, b) = if *stacked {
            (
                Rect {
                    height: size,
                    ..rect
                },
                Rect {
                    y: rect.y + size + 6.0,
                    height: span - size,
                    ..rect
                },
            )
        } else {
            (
                Rect {
                    width: size,
                    ..rect
                },
                Rect {
                    x: rect.x + size + 6.0,
                    width: span - size,
                    ..rect
                },
            )
        };
        apply(first, a, pins);
        apply(second, b, pins);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn geometry(layout: &Layout) -> (Vec<(u64, Rect)>, Vec<super::super::layout::Divider>) {
        let mut plots = Vec::new();
        let mut dividers = Vec::new();
        layout.geometry(
            Rect {
                width: 1600.0,
                height: 1600.0,
                ..Rect::default()
            },
            &mut plots,
            &mut dividers,
        );
        (plots, dividers)
    }
    #[test]
    fn push_starts_at_minimum_and_chains_to_workspace_border_in_both_directions() {
        for axis in [false, true] {
            for forward in [false, true] {
                let original = Layout::flat(&[(1, 1.0), (2, 1.0), (3, 1.0), (4, 1.0)], axis);
                let (_, before) = geometry(&original);
                let index = if forward { 0 } else { 2 };
                let target = before[index];
                let at = |divider: &super::super::layout::Divider| {
                    if axis { divider.rect.y } else { divider.rect.x }
                };
                let minimum = if axis {
                    MIN_PLOT_SIZE.1
                } else {
                    MIN_PLOT_SIZE.0
                };
                let neighbor = 1;
                let slack = (at(&before[neighbor]) - at(&target)).abs() - 6.0 - minimum;
                let direction = if forward { 1.0 } else { -1.0 };
                let mut layout = original.clone();
                layout.resize_divider(
                    target.id,
                    target.ratio + direction * slack / target.span,
                    (1600.0, 1600.0),
                );
                let (_, at_limit) = geometry(&layout);
                assert!((at(&at_limit[neighbor]) - at(&before[neighbor])).abs() < 1e-6);
                layout = original.clone();
                layout.resize_divider(
                    target.id,
                    target.ratio + direction * (slack + 20.0) / target.span,
                    (1600.0, 1600.0),
                );
                let (_, pushed) = geometry(&layout);
                assert!(
                    (at(&pushed[neighbor]) - at(&before[neighbor]) - direction * 20.0).abs() < 1e-6
                );
                let unaffected = if forward { 2 } else { 0 };
                assert!((at(&pushed[unaffected]) - at(&before[unaffected])).abs() < 1e-6);
                layout = original.clone();
                layout.resize_divider(target.id, direction * 100.0, (1600.0, 1600.0));
                let (plots, stopped) = geometry(&layout);
                let size = |rect: &Rect| if axis { rect.height } else { rect.width };
                for (id, rect) in &plots {
                    if *id != if forward { 1 } else { 4 } {
                        assert!((size(rect) - minimum).abs() < 1e-6);
                    }
                }
                let expected = if forward {
                    1600.0 - 3.0 * (minimum + 6.0)
                } else {
                    3.0 * minimum + 12.0
                };
                assert!((at(&stopped[index]) - expected).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn pushing_in_one_row_keeps_aligned_divider_in_other_row_fixed() {
        let mut layout = Layout::Plot(1);
        layout.split(1, 4, true);
        layout.split(1, 2, false);
        layout.split(2, 3, false);
        layout.split(4, 5, false);
        let (_, before) = geometry(&layout);
        let root = before.iter().find(|divider| divider.stacked).unwrap();
        let top: Vec<_> = before
            .iter()
            .filter(|divider| !divider.stacked && divider.rect.y < root.rect.y)
            .collect();
        let bottom = before
            .iter()
            .find(|divider| !divider.stacked && divider.rect.y > root.rect.y)
            .unwrap();
        layout.resize_divider(top[0].id, 10.0, (1600.0, 1600.0));
        let (_, after) = geometry(&layout);
        let find = |id| after.iter().find(|divider| divider.id == id).unwrap();
        assert!(find(top[1].id).rect.x > top[1].rect.x);
        assert!((find(bottom.id).rect.x - bottom.rect.x).abs() < 1e-6);
        assert!((find(root.id).rect.y - root.rect.y).abs() < 1e-6);
    }
    #[test]
    fn nested_resize_pins_other_edges_until_adjacent_pane_reaches_minimum() {
        for stacked in [false, true] {
            let mut layout = Layout::flat(&[(1, 1.0), (2, 1.0), (3, 1.0)], stacked);
            let rect = Rect {
                width: 2400.0,
                height: 2400.0,
                ..Rect::default()
            };
            let mut plots = Vec::new();
            let mut before = Vec::new();
            layout.geometry(rect, &mut plots, &mut before);
            layout.resize_divider(before[0].id, before[0].ratio + 0.1, (2400.0, 2400.0));
            let mut after = Vec::new();
            plots.clear();
            layout.geometry(rect, &mut plots, &mut after);
            let at = |divider: &super::super::layout::Divider| {
                if stacked {
                    divider.rect.y
                } else {
                    divider.rect.x
                }
            };
            assert!(at(&after[0]) > at(&before[0]) + 200.0);
            assert!((at(&after[1]) - at(&before[1])).abs() < 1e-6);
            layout.resize_divider(after[0].id, 0.99, (2400.0, 2400.0));
            let mut limited = Vec::new();
            plots.clear();
            layout.geometry(rect, &mut plots, &mut limited);
            assert!(at(&limited[1]) > at(&before[1]) + 1.0);
            let pane_minimum = if stacked {
                MIN_PLOT_SIZE.1
            } else {
                MIN_PLOT_SIZE.0
            };
            assert!((at(&limited[1]) - at(&limited[0]) - 6.0 - pane_minimum).abs() < 1e-6);
            assert!(plots.iter().all(|(_, pane)| if stacked {
                pane.height >= 160.0 - 1e-6
            } else {
                pane.width >= 220.0 - 1e-6
            }));
        }
    }
    #[test]
    fn mixed_resize_keeps_perpendicular_divider_in_place() {
        let mut layout = Layout::Plot(1);
        layout.split(1, 2, false);
        layout.split(2, 3, true);
        let rect = Rect {
            width: 2000.0,
            height: 1200.0,
            ..Rect::default()
        };
        let mut plots = Vec::new();
        let mut before = Vec::new();
        layout.geometry(rect, &mut plots, &mut before);
        layout.resize_divider(before[0].id, 0.6, (2000.0, 1200.0));
        let mut after = Vec::new();
        plots.clear();
        layout.geometry(rect, &mut plots, &mut after);
        assert!((before[1].rect.y - after[1].rect.y).abs() < 1e-6);
    }
}
