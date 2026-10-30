//! Annotation paths and paint shared by screen rendering and export.
use crate::{
    PlotGeometry, Ranges,
    plot_data::{AnnotationGeometry, PlotAnnotation},
};
use zgui::canvas::{CanvasScene, zgui_color::Color};

pub fn path(
    annotation: AnnotationGeometry,
    ranges: Ranges,
    geometry: PlotGeometry,
) -> kurbo::BezPath {
    use AnnotationGeometry::*;
    let valid = match annotation {
        XThreshold(value) | YThreshold(value) => value.is_finite(),
        XBand(range) | YBand(range) => crate::range::valid(range).is_some(),
        Region(rect) => {
            crate::range::valid(rect.x).is_some() && crate::range::valid(rect.y).is_some()
        }
        FitLine { slope, intercept } => slope.is_finite() && intercept.is_finite(),
    };
    if !valid {
        return kurbo::BezPath::new();
    }
    match annotation {
        XThreshold(value) => crate::overlay::vertical_value_lines([value], ranges.x, geometry),
        YThreshold(value) => crate::overlay::horizontal_value_lines([value], ranges.y, geometry),
        FitLine { slope, intercept } => {
            crate::overlay::fit_line(slope, intercept, ranges, geometry)
        }
        XBand(range) => region(Ranges::new(range, ranges.y), ranges, geometry),
        YBand(range) => region(Ranges::new(ranges.x, range), ranges, geometry),
        Region(rect) => region(rect, ranges, geometry),
    }
}
fn region(rect: Ranges, ranges: Ranges, geometry: PlotGeometry) -> kurbo::BezPath {
    let mut path = kurbo::BezPath::new();
    crate::overlay::append_data_rects(&mut path, [rect], ranges, geometry);
    path
}
pub fn draw(
    scene: &mut CanvasScene,
    annotations: &[PlotAnnotation],
    ranges: Ranges,
    geometry: PlotGeometry,
) {
    for annotation in annotations {
        let path = path(annotation.geometry, ranges, geometry);
        let [r, g, b, a] = annotation.color;
        let color = Color::srgb_u8(r, g, b, a);
        if matches!(
            annotation.geometry,
            AnnotationGeometry::XBand(_)
                | AnnotationGeometry::YBand(_)
                | AnnotationGeometry::Region(_)
        ) {
            crate::zgui_renderer::push_fill(scene, path, color);
        } else if annotation.dashed {
            crate::zgui_renderer::push_dashed_stroke(scene, path, color, annotation.width);
        } else {
            crate::zgui_renderer::push_stroke(scene, path, color, annotation.width);
        }
    }
}
