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
    let canvas = zgui::elements::canvas()
        .class("analysis-heatmap-canvas")
        .draw(move |cx| {
            let Some(size) = size.get() else {
                return;
            };
            let w = f64::from(size.size.width.0 / host.scale());
            let h = f64::from(size.size.height.0 / host.scale());
            let Some(g) = PlotGeometry::with_bounds(0., 0., w, h, w, h) else {
                return;
            };
            let ranges = state.ranges.get();
            let dy = (data.high - data.low) / data.rows as f64;
            let mut paths = vec![kurbo::BezPath::new(); 256];
            for row in 0..data.rows {
                let low = data.low + row as f64 * dy;
                let high = low + dy;
                if high < ranges.y.min || low > ranges.y.max {
                    continue;
                }
                for column in 0..data.columns {
                    let Some(db) = data.values[row * data.columns + column] else {
                        continue;
                    };
                    let left = ((column as f64 - 0.5) * data.frequency_step).max(0.);
                    let right = ((column as f64 + 0.5) * data.frequency_step)
                        .min((data.columns - 1) as f64 * data.frequency_step);
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
                cx.scene.push(
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
        });
    view! { column(class = "analysis-heatmap", node_ref = host) { {canvas.into_view()} } }
}
