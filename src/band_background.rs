use crate::{
    geometry::GeometryKind,
    state::{Pane, Workbench},
};
use plot::{
    PlotGeometry,
    zgui_time_bands::{RetainedTimeBands, TimeBandBatch},
};
use std::collections::BTreeMap;
use zgui::prelude::*;
#[derive(Clone, PartialEq)]
struct Label {
    name: String,
    left: f64,
    width: f64,
    row: usize,
    color: [u8; 3],
}
type Labels = BTreeMap<(u64, usize), Label>;

fn batches(items: &[crate::geometry::Geometry]) -> Vec<TimeBandBatch> {
    let mut out = Vec::new();
    for item in items {
        if let Some(states) = &item.states {
            if states.error.is_some() {
                continue;
            }
            // Keep owners separate: overlapping band layers retain paint order.
            let mut groups: Vec<([u8; 3], Vec<[f64; 2]>)> = Vec::new();
            for interval in states.intervals.iter() {
                let Some(rule) = states
                    .rules
                    .iter()
                    .find(|rule| rule.value == interval.value)
                else {
                    continue;
                };
                if let Some((_, spans)) = groups.iter_mut().find(|(color, _)| *color == rule.color)
                {
                    spans.push([interval.start, interval.end]);
                } else {
                    groups.push((rule.color, vec![[interval.start, interval.end]]));
                }
            }
            out.extend(groups.into_iter().map(|(color, spans)| TimeBandBatch {
                intervals: spans.into(),
                color,
                opacity: item.opacity as f32,
                boundaries: true,
            }));
        } else {
            let [x1, _, x2, _] = item.coordinates;
            if x1 != x2 {
                out.push(TimeBandBatch {
                    intervals: vec![[x1.min(x2), x1.max(x2)]].into(),
                    color: item.color,
                    opacity: item.opacity as f32,
                    boundaries: false,
                });
            }
        }
    }
    out
}

#[component]
fn BandLabel(labels: Memo<Labels>, label_key: (u64, usize)) -> impl IntoView {
    // Independent memos keep a position change from rewriting text and paint.
    let name = Memo::new(move |_| labels.with(|v| v.get(&label_key).map(|v| v.name.clone())));
    let color = Memo::new(move |_| labels.with(|v| v.get(&label_key).map(|v| v.color)));
    let width = Memo::new(move |_| labels.with(|v| v.get(&label_key).map(|v| v.width - 6.)));
    let position = Memo::new(move |_| {
        labels.with(|v| v.get(&label_key).map(|v| (v.left + 3., 8 + v.row * 20)))
    });
    view! {text(class="band-label",style:border-left-color=move ||color.get().map(|[r,g,b]|format!("rgb({r}, {g}, {b})")),
    style:transform=move ||position.get().map(|(x,y)|format!("translate({x}px, {y}px)")),
    style:max-width=move ||width.get().map(|v|format!("{v}px"))) {{move ||name.get().unwrap_or_default()}}}
}

fn label_span(left: f64, right: f64) -> Option<(f64, f64)> {
    let ticks = plot::zgui_plot::theme::TREND_LABELS;
    let left = left.max(ticks.inset + ticks.y_label.0 + 4.);
    (right - left >= 36.).then_some((left, right - left))
}
#[component]
pub fn BandBackground(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let x_range = Memo::new(move |_| pane.plot.ranges.get().x);
    let items = Memo::new(move |_| {
        if !visible.get() {
            return Vec::new();
        }
        let hidden = pane.hidden_geometry.get();
        workbench
            .ordered_geometry(pane)
            .into_iter()
            .filter(|item| item.kind == GeometryKind::Band && !hidden.contains(&item.id))
            .collect::<Vec<_>>()
    });
    let paints = Memo::new(move |_| items.with(|items| batches(items)));
    let scene = Memo::new(move |_| {
        let mut out = Labels::new();
        let items = items.get();
        if items.is_empty() {
            return out;
        }
        let Some(bounds) = bounds.get() else {
            return out;
        };
        let width = f64::from(bounds.size.width.0 / host.scale());
        let height = f64::from(bounds.size.height.0 / host.scale());
        let Some(g) = PlotGeometry::with_bounds(0., 0., width, height, width, height) else {
            return out;
        };
        let x_range = x_range.get();
        for (row, item) in items.into_iter().enumerate() {
            let mut append = |index: usize, start: f64, end: f64, name: &str, color: [u8; 3]| {
                let left = g.x(start.max(x_range.min), x_range);
                let right = g.x(end.min(x_range.max), x_range);
                if right <= left {
                    return;
                }
                if item.labels
                    && out.len() < 300
                    && row < 4
                    && let Some((left, width)) = label_span(left, right)
                {
                    out.insert(
                        (item.id, index),
                        Label {
                            name: name.into(),
                            left,
                            width,
                            row,
                            color,
                        },
                    );
                }
            };
            if let Some(states) = &item.states {
                if states.error.is_some() {
                    continue;
                }
                let start = states
                    .intervals
                    .partition_point(|interval| interval.end <= x_range.min);
                for (index, interval) in states
                    .intervals
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take_while(|(_, v)| v.start < x_range.max)
                {
                    if let Some(rule) = states
                        .rules
                        .iter()
                        .find(|rule| rule.value == interval.value)
                    {
                        append(index, interval.start, interval.end, &rule.name, rule.color);
                    }
                }
            } else {
                let [x1, _, x2, _] = item.coordinates;
                append(0, x1.min(x2), x1.max(x2), &item.name, item.color);
            }
        }
        out
    });
    let retained = std::rc::Rc::new(std::cell::RefCell::new(RetainedTimeBands::default()));
    let handle = retained.borrow().handle().clone();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Background);
    plot::zgui_svg::register_description(
        plot::zgui_svg::SnapshotLayer::Foreground,
        move |description| {
            for label in scene.get_untracked().into_values() {
                description.text.push(plot::SvgText {
                    x: label.left + 3.,
                    y: 20. + label.row as f64 * 20.,
                    text: label.name,
                    color: "#ffffff".into(),
                    opacity: 1.,
                    font_size: 11.,
                    font_weight: 600,
                    anchor: plot::TextAnchor::Start,
                    stroke: Some("#000000".into()),
                    stroke_width: 1.5,
                });
            }
        },
    );
    plot::zgui_renderer::redraw_on_change(move || {
        let Some(bounds) = bounds.get() else {
            return;
        };
        let mut retained = retained.borrow_mut();
        retained.replace(paints.get());
        retained.show(
            x_range.get(),
            f64::from(bounds.size.width.0 / host.scale()),
            f64::from(bounds.size.height.0 / host.scale()),
        );
    });
    let canvas = zgui::elements::canvas()
        .class("band-fill-canvas")
        .scene(&handle);
    view! {column(class="band-background",node_ref=host) {
        {canvas.into_view()}
        for key in move ||scene.with(|scene|scene.keys().copied().collect::<Vec<_>>()),key=|key|*key {
            BandLabel(labels=scene,label_key=key)
        }

    }}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_labels_clear_y_ticks_and_stay_inside_their_interval() {
        assert_eq!(label_span(0., 200.), Some((64., 136.)));
        assert_eq!(label_span(100., 200.), Some((100., 100.)));
        assert!(label_span(0., 80.).is_none());
    }
    #[test]
    fn batches_preserve_owner_order_and_merge_same_colour_intervals() {
        use crate::geometry::Geometry;
        let mut first = Geometry::new(1, GeometryKind::Band, kurbo::Point::ZERO);
        first.coordinates = [1., 0., 3., 0.];
        let mut second = first.clone();
        second.id = 2;
        second.opacity = 0.6;
        let result = batches(&[first, second]);
        assert_eq!(result.len(), 2);
        assert_eq!(&*result[0].intervals, &[[1., 3.]]);
        assert_eq!(result[1].opacity, 0.6);
        assert!(!result[0].boundaries);
    }
}
