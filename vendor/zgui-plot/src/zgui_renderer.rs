//! Zgui CanvasScene adapter for generic vector paths.

use kurbo::BezPath;
use zgui::canvas::zgui_color::Color;
use zgui::canvas::{Brush, CanvasScene, ShapeBuilder};

/// A flat colour as a gradient, which keeps a fill or stroke on the vector route.
///
/// zgui sends small *solid* shapes (thin grid lines, small bars, hover outlines) through a CPU
/// atlas mask, where they are drawn as sprites. A canvas that mixes those with its vector
/// shapes is replayed from a retained chunk whose recorded extent covers only the sprites, so
/// content drawn above the canvas (such as tick labels) is ordered as if the canvas were not
/// there and ends up underneath it. Keeping every shape a vector item avoids the mix; equal
/// stops render as the plain colour.
fn flat(color: Color) -> Brush {
    Brush::Linear {
        start: (0.0, 0.0).into(),
        end: (1.0, 0.0).into(),
        stops: vec![(0.0, color), (1.0, color)],
        repeating: false,
    }
}

/// Fill `path` with a solid colour; empty paths are skipped.
pub fn push_fill(scene: &mut CanvasScene, path: BezPath, color: Color) {
    if !path.is_empty() {
        scene.push(ShapeBuilder::new(path).fill(Brush::Solid(color)).build());
    }
}

/// Stroke `path` with a solid colour; empty paths are skipped.
pub fn push_stroke(scene: &mut CanvasScene, path: BezPath, color: Color, width: f64) {
    if !path.is_empty() {
        scene.push(
            ShapeBuilder::new(path)
                .stroke(Brush::Solid(color), width)
                .build(),
        );
    }
}

/// Stroke `path` with 6px dashes and 4px gaps; empty paths are skipped.
pub fn push_dashed_stroke(scene: &mut CanvasScene, path: BezPath, color: Color, width: f64) {
    if !path.is_empty() {
        scene.push(
            ShapeBuilder::new(path)
                .stroke_styled(
                    Brush::Solid(color),
                    kurbo::Stroke::new(width).with_dashes(0.0, [6.0, 4.0]),
                )
                .build(),
        );
    }
}

/// Fill and stroke `path` in one shape; empty paths are skipped.
pub fn push_fill_stroke(
    scene: &mut CanvasScene,
    path: BezPath,
    fill: Color,
    stroke: Color,
    width: f64,
) {
    if !path.is_empty() {
        scene.push(
            ShapeBuilder::new(path)
                .fill(Brush::Solid(fill))
                .stroke(Brush::Solid(stroke), width)
                .build(),
        );
    }
}

/// Stroke grid layers produced by [`crate::grid_paths`], scaling `color` alpha by layer opacity.
pub fn push_grid(
    scene: &mut CanvasScene,
    layers: impl IntoIterator<Item = (BezPath, f64)>,
    color: Color,
) {
    for (path, opacity) in layers {
        push_stroke(scene, path, color.with_alpha(opacity as f32), 1.0);
    }
}

/// Filled, outlined rectangle spanning two corners given in either order.
pub fn push_selection(
    scene: &mut CanvasScene,
    ((x0, x1), (y0, y1)): ((f64, f64), (f64, f64)),
    fill: Color,
    stroke: Color,
) {
    let mut path = BezPath::new();
    crate::append_rect(&mut path, x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
    // Drag geometry changes every frame; solid paints churn CPU atlas masks.
    scene.push(
        ShapeBuilder::new(path)
            .fill(flat(fill))
            .stroke(flat(stroke), 1.0)
            .build(),
    );
}

/// Fill `path` with a flat colour that stays on the GPU vector route.
///
/// The pinned zgui renderer sends small solid fills through a CPU atlas mask.
/// Point batches change bounds/membership while panning, causing costly mask
/// misses once they fit that route. A canvas that mixes those mask sprites with
/// vector shapes is also replayed with a recorded extent that covers only the
/// sprites, so content drawn above it (tick labels) is ordered as if the canvas
/// were absent and ends up underneath. Equal gradient stops preserve a flat
/// colour while keeping these shapes on the general GPU vector route.
pub fn push_gpu_fill(scene: &mut CanvasScene, path: BezPath, color: Color) {
    if path.is_empty() {
        return;
    }
    scene.push(ShapeBuilder::new(path).fill(flat(color)).build());
}

/// [`push_gpu_fill`] `path` only if its bounds touch the data area; returns
/// whether it was pushed. Culls whole off-screen batches without clipping.
pub fn push_visible_gpu_fill(
    scene: &mut CanvasScene,
    path: BezPath,
    color: Color,
    geometry: crate::PlotGeometry,
) -> bool {
    let visible = geometry.overlaps(kurbo::Shape::bounding_box(&path));
    if visible {
        push_fill(scene, path, color);
    }
    visible
}

/// Convert plot points to compact data-space series for Zgui's retained canvas renderer.
pub fn series_data(points: &[crate::PlotPoint]) -> std::sync::Arc<[[f32; 2]]> {
    points
        .iter()
        .map(|point| [point.x as f32, point.y as f32])
        .collect()
}

/// Draw points in data space; marker size remains constant across zoom levels.
pub fn push_points(
    scene: &mut CanvasScene,
    data: std::sync::Arc<[[f32; 2]]>,
    marker: zgui::canvas::Marker,
    color: Color,
) {
    push_points_transformed(scene, data, marker, color, kurbo::Affine::IDENTITY);
}

/// Draw points with a data-to-canvas transform while preserving pixel marker size.
pub fn push_points_transformed(
    scene: &mut CanvasScene,
    data: std::sync::Arc<[[f32; 2]]>,
    marker: zgui::canvas::Marker,
    color: Color,
    to_canvas: kurbo::Affine,
) {
    scene.push_series(zgui::canvas::Series::Points {
        data,
        to_canvas,
        marker,
        fill: Some(Brush::Solid(color)),
        stroke: None,
    });
}

/// Legacy adapter for point batches: viewport culling and retained submission.
/// Prepared `ScatterSeries` is preferable when caller can retain complete sources.
pub fn push_visible_points(
    scene: &mut CanvasScene,
    points: &[crate::PlotPoint],
    color: Color,
    radius: f64,
    ranges: crate::Ranges,
    geometry: crate::PlotGeometry,
) -> bool {
    let x_margin = 8.0 * ranges.x.span() / geometry.data_width();
    let y_margin = 8.0 * ranges.y.span() / geometry.data_height();
    let points = points
        .iter()
        .copied()
        .filter(|point| {
            point.x.is_finite()
                && point.y.is_finite()
                && point.x >= ranges.x.min - x_margin
                && point.x <= ranges.x.max + x_margin
                && point.y >= ranges.y.min - y_margin
                && point.y <= ranges.y.max + y_margin
        })
        .collect::<Vec<_>>();
    if points.is_empty() {
        return false;
    }
    push_points_transformed(
        scene,
        series_data(&points),
        zgui::canvas::Marker::Circle { radius },
        color,
        data_view(geometry, ranges),
    );
    true
}

/// Draw data-space line once; `Lod::Columns` reduces dense series per device column.
pub fn push_line(
    scene: &mut CanvasScene,
    data: std::sync::Arc<[[f32; 2]]>,
    color: Color,
    width: f64,
    lod: zgui::canvas::Lod,
) {
    push_line_transformed(scene, data, color, width, lod, kurbo::Affine::IDENTITY);
}

/// Draw a transformed data-space line with optional level-of-detail reduction.
pub fn push_line_transformed(
    scene: &mut CanvasScene,
    data: std::sync::Arc<[[f32; 2]]>,
    color: Color,
    width: f64,
    lod: zgui::canvas::Lod,
    to_canvas: kurbo::Affine,
) {
    let series = zgui::canvas::Series::Line {
        data,
        to_canvas,
        stroke: kurbo::Stroke::new(width)
            .with_caps(kurbo::Cap::Round)
            .with_join(kurbo::Join::Round),
        brush: Brush::Solid(color),
    };
    scene.push_series_lod(series, lod);
}

/// Prepared origin-relative line; shared vertices survive viewport changes.
pub fn push_prepared_line(
    scene: &mut CanvasScene,
    data: &crate::time_series::SeriesData,
    color: Color,
    width: f64,
    lod: zgui::canvas::Lod,
    to_canvas: kurbo::Affine,
) {
    let origin = data.origin();
    push_line_transformed(
        scene,
        data.vertices(),
        color,
        width,
        lod,
        to_canvas * kurbo::Affine::translate((origin.x, origin.y)),
    );
}

/// Transform mapping data coordinates into canvas coordinates.
pub fn data_view(geometry: crate::PlotGeometry, ranges: impl Into<crate::Ranges>) -> kurbo::Affine {
    kurbo::Affine::translate((-geometry.left, -geometry.top)) * geometry.data_transform(ranges)
}

/// Update canvas view without rebuilding or uploading retained series.
pub fn show_ranges(
    handle: &zgui::elements::canvas::CanvasHandle,
    geometry: crate::PlotGeometry,
    ranges: impl Into<crate::Ranges>,
) {
    handle.set_transform(data_view(geometry, ranges));
}

/// Run `draw` now and again whenever a signal it read changes, for as long as
/// the calling component lives.
pub fn redraw_on_change(draw: impl Fn() + 'static) {
    let effect = zgui::reactive::RenderEffect::new(move |_| draw());
    zgui::prelude::on_cleanup_local(move || drop(effect));
}

/// Fraction (0..=1) of the way across `node` from `left` to its right edge
/// at pointer `x`, or `None` outside that span.
pub fn pointer_x_fraction(node: &zgui::prelude::NodeRef, x: f32, left: f64) -> Option<f64> {
    let bounds = node.window_bounds()?;
    let local = f64::from(x - bounds.origin.x.0 / node.scale());
    let right = f64::from(bounds.size.width.0 / node.scale());
    (left..=right)
        .contains(&local)
        .then_some((local - left) / (right - left))
}

/// Pointer `position` relative to `node`'s top-left corner, in CSS pixels.
pub fn local_position(
    node: &zgui::prelude::NodeRef,
    position: zgui::geom::Point<zgui::geom::CssPx, zgui::geom::Css>,
) -> Option<(f64, f64)> {
    let bounds = node.window_bounds()?;
    let scale = node.scale();
    Some((
        f64::from(position.x.0 - bounds.origin.x.0 / scale),
        f64::from(position.y.0 - bounds.origin.y.0 / scale),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_gradient(paint: Option<&zgui::canvas::Paint>) -> bool {
        matches!(paint, Some(zgui::canvas::Paint::Gradient(_)))
    }

    fn is_flat_gradient(shape: &zgui::canvas::Shape) -> bool {
        is_gradient(shape.fill.as_ref().map(|fill| &fill.paint))
    }

    #[test]
    fn standard_paints_use_native_brushes_and_legacy_gpu_fill_keeps_gradient() {
        let color = Color::srgb_u8(92, 205, 108, 255);
        let square = || {
            let mut path = BezPath::new();
            crate::append_rect(&mut path, 0.0, 0.0, 3.0, 3.0);
            path
        };
        let mut scene = CanvasScene::default();
        push_fill(&mut scene, square(), color);
        push_fill_stroke(&mut scene, square(), color, color, 1.0);
        push_gpu_fill(&mut scene, square(), color);
        push_selection(&mut scene, ((0.0, 3.0), (0.0, 3.0)), color, color);
        assert_eq!(scene.shapes().len(), 4);
        for index in [0, 1] {
            assert!(matches!(
                scene.shapes()[index].fill.as_ref().unwrap().paint,
                zgui::canvas::Paint::Solid(_)
            ));
        }
        assert!(is_flat_gradient(&scene.shapes()[2]));
        assert!(is_flat_gradient(&scene.shapes()[3]));
        let mut strokes = CanvasScene::default();
        push_stroke(&mut strokes, square(), color, 1.0);
        push_dashed_stroke(&mut strokes, square(), color, 1.0);
        push_fill_stroke(&mut strokes, square(), color, color, 1.0);
        for shape in strokes.shapes() {
            let stroke = shape.stroke.as_ref().expect("stroked");
            assert!(matches!(stroke.paint, zgui::canvas::Paint::Solid(_)));
        }
    }

    #[test]
    fn zoom_box_avoids_cpu_masks_at_every_drag_size() {
        let fill = Color::srgb_u8(119, 185, 232, 40);
        let stroke = Color::srgb_u8(119, 185, 232, 255);
        for extent in [3.0, 128.0, 256.0, 512.0, 1200.0] {
            let mut scene = CanvasScene::default();
            push_selection(&mut scene, ((extent, 0.0), (extent, 0.0)), fill, stroke);
            assert_eq!(scene.shapes().len(), 1);
            let shape = &scene.shapes()[0];
            let mut expected = BezPath::new();
            crate::append_rect(&mut expected, 0.0, 0.0, extent, extent);
            assert_eq!(shape.path.as_ref(), &expected);
            for (paint, color) in [
                (&shape.fill.as_ref().unwrap().paint, fill),
                (&shape.stroke.as_ref().unwrap().paint, stroke),
            ] {
                let zgui::canvas::Paint::Gradient(gradient) = paint else {
                    panic!("Changing zoom boxes must not create CPU atlas masks");
                };
                assert_eq!(gradient.stops.len(), 2);
                assert!(
                    gradient
                        .stops
                        .iter()
                        .all(|stop| stop.color == zgui::canvas::Ink::Solid(color))
                );
            }
            assert_eq!(shape.stroke.as_ref().unwrap().style.width, 1.0);
        }
    }

    #[test]
    fn visible_fill_culls_batches_outside_the_data_area() {
        let geometry =
            crate::PlotGeometry::with_bounds(0.0, 4.0, 640.0, 444.0, 640.0, 480.0).unwrap();
        let color = Color::srgb_u8(92, 205, 108, 255);
        let mut scene = CanvasScene::default();
        for (center, expected) in [((100.0, 100.0), true), ((-100.0, 100.0), false)] {
            let mut path = BezPath::new();
            crate::append_circle(&mut path, center, 3.0);
            assert_eq!(
                push_visible_gpu_fill(&mut scene, path, color, geometry),
                expected
            );
        }
        assert_eq!(scene.shapes().len(), 1);
    }

    #[test]
    fn gpu_fill_keeps_exact_geometry_and_color_above_and_below_mask_thresholds() {
        let color = Color::srgb_u8(92, 205, 108, 255);
        for extent in [5.0, 128.0, 512.0, 1200.0] {
            let mut path = BezPath::new();
            crate::append_circle(&mut path, (0.0, 0.0), 2.5);
            crate::append_circle(&mut path, (extent, extent), 2.5);
            let mut scene = CanvasScene::default();
            push_gpu_fill(&mut scene, path.clone(), color);
            assert_eq!(scene.shapes().len(), 1);
            let shape = &scene.shapes()[0];
            assert_eq!(shape.path.as_ref(), &path);
            assert!(shape.clips.is_empty());
            let zgui::canvas::Paint::Gradient(gradient) = &shape.fill.as_ref().unwrap().paint
            else {
                panic!("solid paint re-enables CPU atlas masks for small point batches");
            };
            assert!(!gradient.repeating);
            assert_eq!(gradient.stops.len(), 2);
            assert!(
                gradient
                    .stops
                    .iter()
                    .all(|stop| stop.color == zgui::canvas::Ink::Solid(color))
            );
        }
    }
}
