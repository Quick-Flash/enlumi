use crate::{
    geometry::GeometryKind,
    state::{Pane, Workbench},
};
use kurbo::{BezPath, Shape};
use plot::PlotGeometry;
use zgui::{
    canvas::{Brush, ShapeBuilder, zgui_color::Color},
    prelude::*,
};
#[derive(Clone, PartialEq)]
struct Label {
    key: (u64, usize),
    name: String,
    left: f64,
    width: f64,
    row: usize,
}
#[derive(Clone, Default, PartialEq)]
struct Backdrop {
    paints: Vec<(u64, BezPath, [u8; 3], f64)>,
    labels: Vec<Label>,
}
#[component]
pub fn BandBackground(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let scene = Memo::new(move |_| {
        let mut out = Backdrop::default();
        if !visible.get() {
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
        let ranges = pane.plot.ranges.get();
        let ids = pane.geometry.get();
        let hidden = pane.hidden_geometry.get();
        let items = workbench.geometry.items.get();
        for (row, item) in items
            .iter()
            .filter(|item| {
                item.kind == GeometryKind::Band
                    && ids.contains(&item.id)
                    && !hidden.contains(&item.id)
            })
            .enumerate()
        {
            let mut append = |index: usize, start: f64, end: f64, name: &str, color: [u8; 3]| {
                let left = g.x(start.max(ranges.x.min), ranges.x);
                let right = g.x(end.min(ranges.x.max), ranges.x);
                if right <= left {
                    return;
                }
                let path = kurbo::Rect::new(left, 0., right, height).to_path(0.1);
                if let Some((_, existing, _, _)) =
                    out.paints.iter_mut().find(|(owner, _, rgb, alpha)| {
                        *owner == item.id && *rgb == color && *alpha == item.opacity
                    })
                {
                    existing.extend(path.elements().iter().copied());
                } else {
                    out.paints.push((item.id, path, color, item.opacity));
                }
                if item.labels && right - left >= 36. && out.labels.len() < 300 && row < 4 {
                    out.labels.push(Label {
                        key: (item.id, index),
                        name: name.into(),
                        left,
                        width: right - left,
                        row,
                    });
                }
            };
            if let Some(states) = &item.states {
                if states.error.is_some() {
                    continue;
                }
                let start = states
                    .intervals
                    .partition_point(|interval| interval.end <= ranges.x.min);
                for (index, interval) in states
                    .intervals
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take_while(|(_, v)| v.start < ranges.x.max)
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
    let canvas = zgui::elements::canvas()
        .class("band-fill-canvas")
        .draw(move |cx| {
            scene.with(|scene| {
                for (_, path, [r, g, b], alpha) in &scene.paints {
                    let color = Color::srgb_u8(*r, *g, *b, 255).with_alpha(*alpha as f32);
                    cx.scene.push(
                        ShapeBuilder::new(path.clone())
                            .fill(Brush::Linear {
                                start: (0., 0.).into(),
                                end: (1., 0.).into(),
                                stops: vec![(0., color), (1., color)],
                                repeating: false,
                            })
                            .build(),
                    );
                }
            })
        });
    view! {column(class="band-background",node_ref=host) {
        {canvas.into_view()}
        for key in move ||scene.with(|scene|scene.labels.iter().map(|label|label.key).collect::<Vec<_>>()),key=|key|*key {
            text(class="band-label",style:left=move ||scene.with(|scene|scene.labels.iter().find(|label|label.key==key).map(|label|format!("{}px",label.left+3.))),style:top=move ||scene.with(|scene|scene.labels.iter().find(|label|label.key==key).map(|label|format!("{}px",8+label.row*20))),style:max-width=move ||scene.with(|scene|scene.labels.iter().find(|label|label.key==key).map(|label|format!("{}px",label.width-6.)))) {{move ||scene.with(|scene|scene.labels.iter().find(|label|label.key==key).map_or_else(String::new,|label|label.name.clone()))}}
        }

    }}
}
