use crate::{
    geometry::GeometryKind,
    state::{Pane, Workbench},
};
use kurbo::{BezPath, Shape};
use plot::PlotGeometry;
use std::collections::BTreeMap;
use zgui::{
    canvas::{Brush, ShapeBuilder, zgui_color::Color},
    prelude::*,
};
#[derive(Clone, PartialEq)]
struct Label {
    name: String,
    left: f64,
    width: f64,
    row: usize,
    color: [u8; 3],
}
#[derive(Clone, Default, PartialEq)]
struct Backdrop {
    paints: Vec<(u64, BezPath, [u8; 3], f64)>,
    labels: BTreeMap<(u64, usize), Label>,
    edges: Vec<StateEdges>,
}
#[derive(Clone, PartialEq)]
struct StateEdges {
    owner: u64,
    color: [u8; 3],
    edges: BezPath,
}
impl Backdrop {
    fn state_edge(&mut self, owner: u64, left: f64, color: [u8; 3], height: f64) {
        let index = self
            .edges
            .iter()
            .position(|ribbon| ribbon.owner == owner && ribbon.color == color)
            .unwrap_or_else(|| {
                self.edges.push(StateEdges {
                    owner,
                    color,
                    edges: BezPath::new(),
                });
                self.edges.len() - 1
            });
        let edge = &mut self.edges[index];
        edge.edges.move_to((left, 0.));
        edge.edges.line_to((left, height));
    }
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
    let scene = Memo::new(move |_| {
        let mut out = Backdrop::default();
        if !visible.get() {
            return out;
        }
        let hidden = pane.hidden_geometry.get();
        let items = workbench.ordered_geometry(pane);
        let items: Vec<_> = items
            .iter()
            .filter(|item| item.kind == GeometryKind::Band && !hidden.contains(&item.id))
            .collect();
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
                let path = kurbo::Rect::new(left, 0., right, height).to_path(0.1);
                if item.states.is_some() && item.opacity > 0.0 {
                    out.state_edge(item.id, left, color, height);
                }
                if let Some((_, existing, _, _)) =
                    out.paints.iter_mut().find(|(owner, _, rgb, alpha)| {
                        *owner == item.id && *rgb == color && *alpha == item.opacity
                    })
                {
                    existing.extend(path.elements().iter().copied());
                } else {
                    out.paints.push((item.id, path, color, item.opacity));
                }
                if item.labels
                    && out.labels.len() < 300
                    && row < 4
                    && let Some((left, width)) = label_span(left, right)
                {
                    out.labels.insert(
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
    let handle = zgui::elements::canvas::CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Background);
    plot::zgui_svg::register_description(
        plot::zgui_svg::SnapshotLayer::Foreground,
        move |description| {
            for label in scene.get_untracked().labels.into_values() {
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
    let drawing = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        drawing.draw(|canvas_scene| {
            canvas_scene.clear();
            scene.with(|scene| {
                for (_, path, [r, g, b], alpha) in &scene.paints {
                    let color = Color::srgb_u8(*r, *g, *b, 255).with_alpha(*alpha as f32);
                    canvas_scene.push(
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
                for edge in &scene.edges {
                    let [r, g, b] = edge.color;
                    let color = Color::srgb_u8(r, g, b, 255);
                    canvas_scene.push(
                        ShapeBuilder::new(edge.edges.clone())
                            .stroke(Brush::Solid(color.with_alpha(0.35)), 1.)
                            .build(),
                    );
                }
            })
        })
    });
    let canvas = zgui::elements::canvas()
        .class("band-fill-canvas")
        .scene(&handle);
    view! {column(class="band-background",node_ref=host) {
        {canvas.into_view()}
        for key in move ||scene.with(|scene|scene.labels.keys().copied().collect::<Vec<_>>()),key=|key|*key {
            text(class="band-label",style:border-left-color=move ||scene.with(|scene|scene.labels.get(&key).map(|label|format!("rgb({}, {}, {})",label.color[0],label.color[1],label.color[2]))),style:left=move ||scene.with(|scene|scene.labels.get(&key).map(|label|format!("{}px",label.left+3.))),style:top=move ||scene.with(|scene|scene.labels.get(&key).map(|label|format!("{}px",8+label.row*20))),style:max-width=move ||scene.with(|scene|scene.labels.get(&key).map(|label|format!("{}px",label.width-6.)))) {{move ||scene.with(|scene|scene.labels.get(&key).map_or_else(String::new,|label|label.name.clone()))}}
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
    fn repeated_state_intervals_batch_vertical_edges_without_merging_owners_or_colors() {
        let mut scene = Backdrop::default();
        for interval in 0..100 {
            let left = interval as f64 * 10.;
            scene.state_edge(1, left, [10, 20, 30], 200.);
        }
        assert_eq!(
            scene.edges.len(),
            1,
            "One stroke replaces 100 canvas shapes"
        );
        assert_eq!(scene.edges[0].edges.elements().len(), 200);
        for segment in scene.edges[0].edges.elements().chunks_exact(2) {
            let [kurbo::PathEl::MoveTo(start), kurbo::PathEl::LineTo(end)] = segment else {
                panic!("Only vertical interval boundaries expected");
            };
            assert_eq!(start.x, end.x);
        }
        scene.state_edge(2, 0., [10, 20, 30], 200.);
        scene.state_edge(1, 5., [40, 50, 60], 200.);
        assert_eq!(
            scene.edges.len(),
            3,
            "Separate band layers retain their opacity and paint order"
        );
        assert_eq!(scene.edges[0].owner, 1);
        assert_eq!(scene.edges[1].owner, 2);
    }
}
