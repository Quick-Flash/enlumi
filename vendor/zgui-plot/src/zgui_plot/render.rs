use super::{HistogramHit, PlotState, SeriesHit};
use crate::zgui_renderer::{push_fill, push_points_transformed, push_prepared_line, push_stroke};
use crate::{
    PlotGeometry, PlotPoint,
    plot_data::{HistogramCurve, MarkerShape, PlotData, SeriesStyle},
};
use zgui::canvas::{CanvasScene, Marker, zgui_color::Color};
use zgui::prelude::*;

#[derive(Clone, Default, PartialEq)]
pub struct Distribution {
    pub sample_count: usize,
    pub mean: f64,
    pub sigma: f64,
}
impl Distribution {
    pub fn new(moments: (usize, f64, f64)) -> Self {
        Self {
            sample_count: moments.0,
            mean: moments.1,
            sigma: moments.2,
        }
    }
}
pub fn color(style: SeriesStyle, alpha: f64) -> Color {
    let [r, g, b, a] = style.color;
    Color::srgb_u8(r, g, b, ((a as f64 * alpha).clamp(0.0, 255.0)) as u8)
}
pub fn marker(style: SeriesStyle) -> Marker {
    let radius = if style.radius.is_finite() {
        style.radius.clamp(0.5, 50.0)
    } else {
        3.0
    };
    match style.marker {
        MarkerShape::Circle => Marker::Circle { radius },
        MarkerShape::Square => Marker::Square { half: radius },
        MarkerShape::Triangle => Marker::Path(std::sync::Arc::new(crate::triangle_marker(radius))),
        MarkerShape::Diamond => Marker::Path(std::sync::Arc::new(crate::diamond_marker(radius))),
        MarkerShape::Cross => Marker::Path(std::sync::Arc::new(crate::cross_marker(
            radius,
            radius / 3.0,
        ))),
    }
}
fn paint(state: PlotState, id: &str) -> (SeriesStyle, f64) {
    let mut style = state.style(id);
    let hover = state.emphasized.get();
    let alpha = if hover.as_ref().is_some_and(|hover| hover != id) {
        0.3
    } else {
        1.0
    };
    if hover.as_deref() == Some(id) {
        style.width += 1.5;
        style.radius += 1.5;
    }
    (style, alpha)
}
pub fn draw_data(
    scene: &mut CanvasScene,
    data: &PlotData,
    state: PlotState,
    geometry: Option<PlotGeometry>,
    distribution: &Distribution,
) {
    let hidden = state.hidden.get();
    match data {
        PlotData::Time(set) => {
            for series in set
                .series()
                .iter()
                .filter(|series| !hidden.contains(series.id()))
            {
                let (style, alpha) = paint(state, series.id());
                if series.data().samples().len() == 1 {
                    let origin = series.data().origin();
                    push_points_transformed(
                        scene,
                        series.data().vertices(),
                        marker(style),
                        color(style, alpha),
                        kurbo::Affine::translate((origin.x, origin.y)),
                    );
                } else {
                    push_prepared_line(
                        scene,
                        series.data(),
                        color(style, alpha),
                        style.width.max(0.5),
                        state.lod.get(),
                        kurbo::Affine::IDENTITY,
                    );
                }
            }
        }
        PlotData::Scatter(set) => {
            for series in set
                .series()
                .iter()
                .filter(|series| !hidden.contains(series.id()))
            {
                let (style, alpha) = paint(state, series.id());
                let origin = series.origin();
                push_points_transformed(
                    scene,
                    series.vertices(),
                    marker(style),
                    color(style, alpha),
                    kurbo::Affine::translate((origin.x, origin.y)),
                );
            }
        }
        PlotData::Histogram(set) => {
            let Some(geometry) = geometry else {
                return;
            };
            let ranges = state.ranges.get();
            let options = state.histogram.get();
            let bins = set.grouped_bins(&hidden, options.bins, ranges.x);
            let groups = bins
                .iter()
                .map(|(_, bins)| bins.as_slice())
                .collect::<Vec<_>>();
            for (index, path) in
                crate::histogram::stacked_bar_paths_in_range(&groups, geometry, ranges.y)
            {
                let (style, mut alpha) = paint(state, &bins[index].0);
                if state.curve_hover.get().is_some() {
                    alpha *= 0.3;
                }
                push_fill(scene, path, color(style, alpha));
            }
            if options.cdf || options.ccdf {
                push_stroke(
                    scene,
                    crate::overlay::right_axis(geometry, &[0.0, 0.25, 0.5, 0.75, 1.0], 4.0),
                    Color::srgb_u8(160, 170, 180, 255),
                    1.0,
                );
                for (reverse, enabled, color) in [
                    (false, options.cdf, Color::srgb_u8(255, 224, 160, 255)),
                    (true, options.ccdf, Color::srgb_u8(119, 185, 232, 255)),
                ] {
                    if enabled {
                        push_stroke(
                            scene,
                            crate::histogram::cumulative_columns(
                                &set.series()
                                    .iter()
                                    .filter(|series| !hidden.contains(series.id()))
                                    .map(|series| series.values())
                                    .collect::<Vec<_>>(),
                                ranges.x,
                                geometry,
                                reverse,
                            ),
                            color.with_alpha(
                                if state.curve_hover.get().is_some_and(|curve| {
                                    curve
                                        != if reverse {
                                            HistogramCurve::Ccdf
                                        } else {
                                            HistogramCurve::Cdf
                                        }
                                }) {
                                    0.3
                                } else {
                                    1.0
                                },
                            ),
                            if state.curve_hover.get()
                                == Some(if reverse {
                                    HistogramCurve::Ccdf
                                } else {
                                    HistogramCurve::Cdf
                                })
                            {
                                3.0
                            } else {
                                1.5
                            },
                        );
                    }
                }
            }
            if options.normal_fit && distribution.sigma.is_finite() && distribution.sigma > 0.0 {
                let mut path = kurbo::BezPath::new();
                let factor = distribution.sample_count as f64 * ranges.x.span()
                    / options.bins.clamp(1, 4096) as f64
                    / (distribution.sigma * (2.0 * std::f64::consts::PI).sqrt());
                for index in 0..=256 {
                    let x = ranges.x.min + ranges.x.span() * index as f64 / 256.0;
                    let y = factor
                        * (-0.5 * ((x - distribution.mean) / distribution.sigma).powi(2)).exp();
                    let point = (geometry.x(x, ranges.x), geometry.y(y, ranges.y));
                    if index == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                push_stroke(
                    scene,
                    path,
                    Color::srgb_u8(237, 145, 134, 255).with_alpha(
                        if state
                            .curve_hover
                            .get()
                            .is_some_and(|curve| curve != HistogramCurve::NormalFit)
                        {
                            0.3
                        } else {
                            1.0
                        },
                    ),
                    if state.curve_hover.get() == Some(HistogramCurve::NormalFit) {
                        3.5
                    } else {
                        2.0
                    },
                );
            }
        }
    }
}
pub fn hover<'a>(
    data: &'a PlotData,
    state: PlotState,
    target: PlotPoint,
    geometry: PlotGeometry,
) -> Option<SeriesHit> {
    let ranges = state.ranges.get_untracked();
    let hidden = state.hidden.get_untracked();
    let pixels = (geometry.data_width(), geometry.data_height());
    let mut closest: Option<(f64, &'a str, usize, PlotPoint)> = None;
    let mut consider = |series_id: &'a str, index, point: PlotPoint| {
        let distance = ((point.x - target.x) / ranges.x.span() * pixels.0).powi(2)
            + ((point.y - target.y) / ranges.y.span() * pixels.1).powi(2);
        if closest
            .as_ref()
            .is_none_or(|(best, ..)| distance.total_cmp(best).is_lt())
        {
            closest = Some((distance, series_id, index, point));
        }
    };
    match data {
        PlotData::Time(set) => {
            for series in set
                .series()
                .iter()
                .filter(|series| !hidden.contains(series.id()))
            {
                if let Some(index) = series.data().nearest(target, ranges, pixels, 12.0) {
                    consider(series.id(), index, series.data().samples()[index]);
                }
            }
        }
        PlotData::Scatter(set) => {
            for series in set
                .series()
                .iter()
                .filter(|series| !hidden.contains(series.id()))
            {
                if let Some(index) = series.nearest(target, ranges, pixels, 12.0) {
                    consider(series.id(), index, series.samples()[index]);
                }
            }
        }
        PlotData::Histogram(set) => {
            let groups = set.grouped_bins(&hidden, state.histogram.get_untracked().bins, ranges.x);
            let total = set
                .series()
                .iter()
                .filter(|series| !hidden.contains(series.id()))
                .map(|series| series.values().len())
                .sum::<usize>();
            let bins = groups.first().map_or(0, |(_, bins)| bins.len());
            let hit = crate::histogram::bin_at(bins, (target.x - ranges.x.min) / ranges.x.span())
                .map(|index| {
                    let mut bin = groups[0].1[index];
                    bin.count = groups.iter().map(|(_, bins)| bins[index].count).sum();
                    HistogramHit {
                        bin,
                        sample_count: total,
                        percent: if total > 0 {
                            100.0 * bin.count as f64 / total as f64
                        } else {
                            0.0
                        },
                    }
                });
            if state.bin_hover.with_untracked(|current| current != &hit) {
                state.bin_hover.set(hit);
            }
        }
    }
    closest.map(|(_, series_id, sample_index, point)| SeriesHit {
        series_id: series_id.into(),
        sample_index,
        point,
    })
}
