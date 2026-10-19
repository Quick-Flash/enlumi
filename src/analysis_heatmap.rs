use crate::spectral::Heatmap;
use kurbo::Shape;
use plot::{PlotGeometry, zgui_plot::PlotState};
use zgui::{
    canvas::{Brush, ShapeBuilder, zgui_color::Color},
    prelude::*,
};

fn color(value: usize) -> Color {
    let t = value as f32 / 255.;
    Color::srgb(
        (1.5 * t - 0.5).clamp(0., 1.),
        (1.5 - (2. * t - 1.).abs() * 1.5).clamp(0., 1.),
        (1. - 1.5 * t).clamp(0., 1.),
        1.,
    )
}

#[component]
pub fn AnalysisHeatmap(data: Heatmap, state: PlotState) -> impl IntoView {
    let host = NodeRef::new();
    let size = host.observe_border_box();
    let handle = zgui::elements::canvas::CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Background);
    let drawing = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        drawing.draw(|canvas_scene| {
            canvas_scene.clear();
            let Some(size) = size.get() else {
                return;
            };
            let w = f64::from(size.size.width.0 / host.scale());
            let h = f64::from(size.size.height.0 / host.scale());
            let Some(g) = PlotGeometry::with_bounds(0., 0., w, h, w, h) else {
                return;
            };
            let ranges = state.ranges.get();
            let dx = (data.x_high - data.x_low) / data.columns as f64;
            let dy = (data.y_high - data.y_low) / data.rows as f64;
            let mut paths = vec![kurbo::BezPath::new(); 256];
            for row in 0..data.rows {
                let low = data.y_low + row as f64 * dy;
                let high = low + dy;
                if high < ranges.y.min || low > ranges.y.max {
                    continue;
                }
                for column in 0..data.columns {
                    let Some(db) = data.values[row * data.columns + column] else {
                        continue;
                    };
                    let left = data.x_low + column as f64 * dx;
                    let right = left + dx;
                    if right < ranges.x.min || left > ranges.x.max {
                        continue;
                    }
                    let level = (((db - data.min_db) / (data.max_db - data.min_db).max(1e-12)
                        * 255.) as usize)
                        .min(255);
                    let rect = kurbo::Rect::new(
                        g.x(left.max(ranges.x.min), ranges.x),
                        g.y(high.min(ranges.y.max), ranges.y),
                        g.x(right.min(ranges.x.max), ranges.x),
                        g.y(low.max(ranges.y.min), ranges.y),
                    );
                    paths[level].extend(rect.to_path(0.1).elements().iter().copied());
                }
            }
            for (level, path) in paths
                .into_iter()
                .enumerate()
                .filter(|(_, path)| !path.is_empty())
            {
                let color = color(level);
                canvas_scene.push(
                    ShapeBuilder::new(path)
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
    let canvas = zgui::elements::canvas()
        .class("analysis-heatmap-canvas")
        .scene(&handle);
    view! { column(class = "analysis-heatmap", node_ref = host) { {canvas.into_view()} } }
}
