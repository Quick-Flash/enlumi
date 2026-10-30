mod export;
mod layers;
mod snapshot;
pub use export::{ScreenshotFormat, capture_plot};
pub use layers::{SnapshotLayer, SnapshotLayers, register_canvas, register_description};
pub use snapshot::{
    PlotSnapshot, PlotSnapshotSource, WorkspaceSnapshot, capture_workspace,
    capture_workspace_with_overlay, node_rect, register_snapshot,
};
use std::fmt::Write as _;

mod workspace;
pub use workspace::{SvgPanel, compose_workspace_svg};

use zgui::canvas::zgui_color::{Color, ColorSpace};
use zgui::canvas::{CanvasItem, CanvasScene, Ink, Marker, Paint, Series};

pub use crate::{PlotDescription, SvgLegendEntry, SvgRect, SvgText};

/// Snapshot retained canvas paths into SVG without triggering repaint or mutating revisions.
pub fn snapshot_canvas_handles(
    handles: &[&zgui::elements::canvas::CanvasHandle],
) -> Result<Vec<CanvasScene>, String> {
    let mut snapshots = Vec::with_capacity(handles.len());
    for handle in handles {
        let scene = zgui::canvas::resolve(handle.scene().token())
            .ok_or_else(|| "plot canvas scene is no longer available".to_owned())?;
        let scene = scene
            .lock()
            .map_err(|_| "plot canvas scene lock is poisoned".to_owned())?;
        snapshots.push(copy_scene(&scene));
    }
    Ok(snapshots)
}

/// Copy shapes, series, order, LOD settings, and view from retained canvas scene.
fn copy_scene(scene: &CanvasScene) -> CanvasScene {
    let mut copy = CanvasScene::default();
    let mut series = scene.series().iter().peekable();
    for (index, shape) in scene.shapes().iter().enumerate() {
        while let Some(at) = series.next_if(|at| at.before <= index) {
            copy.push_series_lod(at.series.clone(), at.lod);
        }
        copy.push(shape.clone());
    }
    for at in series {
        copy.push_series_lod(at.series.clone(), at.lod);
    }
    copy.set_transform(scene.transform());
    copy
}

/// The same plot rendered with and without its overlay scenes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SvgPair {
    pub clean: String,
    pub with_overlay: String,
}

/// Render `base` alone, and `base` with `overlay` drawn on top (for example a
/// cursor or hover marker), sharing one description.
pub fn scenes_to_svg_pair(
    base: &[&CanvasScene],
    overlay: &[&CanvasScene],
    description: &PlotDescription,
) -> Result<SvgPair, String> {
    let layered = base.iter().chain(overlay).copied().collect::<Vec<_>>();
    Ok(SvgPair {
        clean: scenes_to_svg(base, description)?,
        with_overlay: scenes_to_svg(&layered, description)?,
    })
}

/// Render `scenes` in order over the description's background, then its
/// rectangles and text, as a standalone SVG document. Clipped shapes are not
/// supported yet and return an error.
pub fn scenes_to_svg(
    scenes: &[&CanvasScene],
    description: &PlotDescription,
) -> Result<String, String> {
    let PlotDescription {
        width,
        height,
        text: labels,
        ..
    } = description;
    if !width.is_finite() || !height.is_finite() || *width <= 0.0 || *height <= 0.0 {
        return Err("SVG dimensions must be finite and positive".to_owned());
    }
    let mut defs = String::new();
    let mut body = String::new();
    let mut paint_id = 0usize;
    let bounds = kurbo::Rect::new(0.0, 0.0, *width, *height);
    write!(
        body,
        "<rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"{}\"/>",
        escape_xml(&description.background),
    )
    .expect("writing to String cannot fail");
    for scene in scenes {
        for item in scene.items() {
            match item {
                CanvasItem::Shape(shape) => {
                    if !shape.clips.is_empty() {
                        return Err(
                            "SVG export does not yet support clipped plot shapes".to_owned()
                        );
                    }
                    let mut attrs = String::new();
                    if let Some(fill) = &shape.fill {
                        let (paint, opacity) = svg_paint(&fill.paint, &mut defs, &mut paint_id);
                        write!(attrs, " fill=\"{paint}\" fill-opacity=\"{opacity}\"")
                            .expect("writing to String cannot fail");
                    } else {
                        attrs.push_str(" fill=\"none\"");
                    }
                    if let Some(stroke) = &shape.stroke {
                        let (paint, opacity) = svg_paint(&stroke.paint, &mut defs, &mut paint_id);
                        let style = &stroke.style;
                        write!(
                    attrs,
                    " stroke=\"{paint}\" stroke-opacity=\"{opacity}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\" stroke-miterlimit=\"{}\"",
                    style.width,
                    cap_name(style.start_cap),
                    join_name(style.join),
                    style.miter_limit,
                )
                .expect("writing to String cannot fail");
                        if !style.dash_pattern.is_empty() {
                            write!(
                                attrs,
                                " stroke-dasharray=\"{}\" stroke-dashoffset=\"{}\"",
                                style
                                    .dash_pattern
                                    .iter()
                                    .map(f64::to_string)
                                    .collect::<Vec<_>>()
                                    .join(","),
                                style.dash_offset,
                            )
                            .expect("writing to String cannot fail");
                        }
                    } else {
                        attrs.push_str(" stroke=\"none\"");
                    }
                    if scene.transform() != kurbo::Affine::IDENTITY {
                        let [a, b, c, d, e, f] = scene.transform().as_coeffs();
                        write!(attrs, " transform=\"matrix({a} {b} {c} {d} {e} {f})\"")
                            .expect("writing to String cannot fail");
                    }
                    write!(body, "<path d=\"{}\"{attrs}/>", shape.path.to_svg())
                        .expect("writing to String cannot fail");
                }
                CanvasItem::Series(series) => {
                    write_series(
                        &mut body,
                        &mut defs,
                        &mut paint_id,
                        series,
                        scene.transform(),
                        bounds,
                    );
                }
            }
        }
    }
    for rect in &description.rects {
        write!(
            body,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\" fill-opacity=\"{}\"",
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            rect.radius,
            escape_xml(&rect.fill),
            rect.opacity,
        )
        .expect("writing to String cannot fail");
        if let Some(stroke) = &rect.stroke {
            write!(
                body,
                " stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"{}\"",
                escape_xml(stroke),
                rect.stroke_opacity,
                rect.stroke_width,
            )
            .expect("writing to String cannot fail");
        } else {
            body.push_str(" stroke=\"none\"");
        }
        body.push_str("/>");
    }
    for label in labels {
        write!(
            body,
            "<text x=\"{}\" y=\"{}\" fill=\"{}\" fill-opacity=\"{}\" font-family=\"sans-serif\" font-size=\"{}\" font-weight=\"{}\" text-anchor=\"{}\"{}>{}</text>",
            label.x,
            label.y,
            escape_xml(&label.color),
            label.opacity,
            label.font_size,
            label.font_weight,
            label.anchor.as_svg(),
            label.stroke.as_ref().map_or_else(String::new, |stroke| format!(" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\" paint-order=\"stroke\"", escape_xml(stroke), label.stroke_width)),
            escape_xml(&label.text),
        )
        .expect("writing to String cannot fail");
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><defs>{defs}</defs>{body}</svg>"
    ))
}

/// Render retained data series with the same view transform used by Zgui canvas.
fn write_series(
    body: &mut String,
    defs: &mut String,
    paint_id: &mut usize,
    series: &Series,
    view: kurbo::Affine,
    bounds: kurbo::Rect,
) {
    match series {
        Series::Points {
            data,
            to_canvas,
            marker,
            fill,
            stroke,
        } => {
            let map = view * *to_canvas;
            let mut attrs = String::new();
            if let Some(brush) = fill {
                let (paint, opacity) = svg_paint(&brush.paint(), defs, paint_id);
                write!(attrs, " fill=\"{paint}\" fill-opacity=\"{opacity}\"")
                    .expect("writing to String cannot fail");
            } else {
                attrs.push_str(" fill=\"none\"");
            }
            if let Some((brush, width)) = stroke {
                let (paint, opacity) = svg_paint(&brush.paint(), defs, paint_id);
                write!(
                    attrs,
                    " stroke=\"{paint}\" stroke-opacity=\"{opacity}\" stroke-width=\"{width}\""
                )
                .expect("writing to String cannot fail");
            }
            let (reach, marker_path) = match marker {
                Marker::Circle { radius } => (*radius, None),
                Marker::Square { half } => (*half, None),
                Marker::Path(path) => {
                    let id = format!("plot-marker-{paint_id}");
                    *paint_id += 1;
                    write!(defs, "<path id=\"{id}\" d=\"{}\"/>", path.to_svg())
                        .expect("writing to String cannot fail");
                    (
                        kurbo::Shape::bounding_box(path.as_ref()).size().max_side(),
                        Some(id),
                    )
                }
                _ => return,
            };
            let visible = bounds.inflate(reach, reach);
            write!(body, "<g{attrs}>").expect("writing to String cannot fail");
            for [x, y] in data.iter() {
                let point = map * kurbo::Point::new(f64::from(*x), f64::from(*y));
                if !point.is_finite() || !visible.contains(point) {
                    continue;
                }
                match (marker, &marker_path) {
                    (Marker::Circle { radius }, _) => write!(
                        body,
                        "<circle cx=\"{}\" cy=\"{}\" r=\"{radius}\"/>",
                        point.x, point.y
                    ),
                    (Marker::Square { half }, _) => write!(
                        body,
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                        point.x - half,
                        point.y - half,
                        2.0 * half,
                        2.0 * half
                    ),
                    (_, Some(id)) => write!(
                        body,
                        "<use href=\"#{id}\" x=\"{}\" y=\"{}\"/>",
                        point.x, point.y
                    ),
                    _ => Ok(()),
                }
                .expect("writing to String cannot fail");
            }
            body.push_str("</g>");
        }
        Series::Line {
            data,
            to_canvas,
            stroke,
            brush,
        } => {
            let map = view * *to_canvas;
            let mut path = kurbo::BezPath::new();
            let mut open = false;
            for [x, y] in data.iter() {
                let point = map * kurbo::Point::new(f64::from(*x), f64::from(*y));
                if !point.is_finite() {
                    open = false;
                    continue;
                }
                if open {
                    path.line_to(point);
                } else {
                    path.move_to(point);
                    open = true;
                }
            }
            let (paint, opacity) = svg_paint(&brush.paint(), defs, paint_id);
            write!(
                body,
                "<path d=\"{}\" fill=\"none\" stroke=\"{paint}\" stroke-opacity=\"{opacity}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"round\"/>",
                path.to_svg(),
                stroke.width,
                cap_name(stroke.start_cap),
            )
            .expect("writing to String cannot fail");
        }
    }
}

fn svg_paint(paint: &Paint, defs: &mut String, next_id: &mut usize) -> (String, f32) {
    match paint {
        Paint::Solid(Ink::Solid(color)) => {
            let (color, opacity) = svg_color(*color);
            (color, opacity)
        }
        Paint::Solid(Ink::Inherited { alpha }) => ("currentColor".to_owned(), *alpha),
        Paint::Gradient(gradient) => {
            let id = format!("plot-gradient-{}", *next_id);
            *next_id += 1;
            let tag = match gradient.kind {
                zgui::canvas::GradientKind::Linear { start, end } => {
                    write!(
                        defs,
                        "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" spreadMethod=\"{}\">",
                        start.x,
                        start.y,
                        end.x,
                        end.y,
                        spread_name(gradient.repeating),
                    )
                    .expect("writing to String cannot fail");
                    "linearGradient"
                }
                zgui::canvas::GradientKind::Radial {
                    center,
                    radius_x,
                    radius_y,
                } => {
                    write!(
                        defs,
                        "<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" r=\"{}\" gradientTransform=\"translate({} {}) scale(1 {}) translate({} {})\" spreadMethod=\"{}\">",
                        center.x,
                        center.y,
                        radius_x,
                        center.x,
                        center.y,
                        radius_y / radius_x,
                        -center.x,
                        -center.y,
                        spread_name(gradient.repeating),
                    )
                    .expect("writing to String cannot fail");
                    "radialGradient"
                }
            };
            for stop in &gradient.stops {
                let (color, opacity) = match stop.color {
                    Ink::Solid(color) => svg_color(color),
                    Ink::Inherited { alpha } => ("currentColor".to_owned(), alpha),
                };
                write!(
                    defs,
                    "<stop offset=\"{}%\" stop-color=\"{color}\" stop-opacity=\"{opacity}\"/>",
                    stop.offset * 100.0,
                )
                .expect("writing to String cannot fail");
            }
            write!(defs, "</{tag}>").expect("writing to String cannot fail");
            (format!("url(#{id})"), 1.0)
        }
    }
}

fn svg_color(color: Color) -> (String, f32) {
    let color = color.to_space(ColorSpace::Srgb);
    let [red, green, blue] = color.components();
    (
        format!(
            "#{:02x}{:02x}{:02x}",
            (red.clamp(0.0, 1.0) * 255.0).round() as u8,
            (green.clamp(0.0, 1.0) * 255.0).round() as u8,
            (blue.clamp(0.0, 1.0) * 255.0).round() as u8,
        ),
        color.alpha(),
    )
}

fn cap_name(cap: kurbo::Cap) -> &'static str {
    match cap {
        kurbo::Cap::Butt => "butt",
        kurbo::Cap::Square => "square",
        kurbo::Cap::Round => "round",
    }
}

fn join_name(join: kurbo::Join) -> &'static str {
    match join {
        kurbo::Join::Bevel => "bevel",
        kurbo::Join::Miter => "miter",
        kurbo::Join::Round => "round",
    }
}

fn spread_name(repeating: bool) -> &'static str {
    if repeating { "repeat" } else { "pad" }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::BezPath;
    use zgui::canvas::zgui_color::Color;
    use zgui::canvas::{Brush, ShapeBuilder};

    #[test]
    fn exports_vector_paths_and_escaped_vector_text() {
        let mut path = BezPath::new();
        path.move_to((1.0, 2.0));
        path.line_to((30.0, 40.0));
        let mut scene = CanvasScene::default();
        scene.push(
            ShapeBuilder::new(path)
                .stroke(Brush::Solid(Color::WHITE), 2.0)
                .build(),
        );
        let labels = [SvgText {
            x: 4.0,
            y: 18.0,
            text: "A&B <C>".to_owned(),
            color: "#fff".to_owned(),
            opacity: 1.0,
            font_size: 13.0,
            font_weight: 600,
            anchor: crate::TextAnchor::Start,
            stroke: None,
            stroke_width: 0.0,
        }];

        let svg = scenes_to_svg(
            &[&scene],
            &PlotDescription {
                width: 100.0,
                height: 80.0,
                background: "#12181c".to_owned(),
                rects: vec![SvgRect {
                    x: 5.0,
                    y: 5.0,
                    width: 30.0,
                    height: 20.0,
                    fill: "#202b30".to_owned(),
                    opacity: 0.9,
                    stroke: Some("#46565b".to_owned()),
                    stroke_opacity: 0.5,
                    stroke_width: 1.0,
                    radius: 3.0,
                }],
                text: labels.into(),
            },
        )
        .unwrap();

        assert!(svg.contains("<path d=\"M1"));
        assert!(svg.contains("L30"));
        assert!(svg.contains("A&amp;B &lt;C&gt;"));
        assert!(svg.contains("<rect x=\"5\" y=\"5\""));
        assert!(svg.contains(&format!(
            "<rect x=\"0\" y=\"0\" width=\"100\" height=\"80\" fill=\"{}\"",
            "#12181c"
        )));
        assert!(!svg.contains("<image"));
    }

    #[test]
    fn pair_adds_overlay_only_to_second_svg() {
        let line = |x: f64| {
            let mut path = BezPath::new();
            path.move_to((x, 0.0));
            path.line_to((x, 10.0));
            let mut scene = CanvasScene::default();
            scene.push(
                ShapeBuilder::new(path)
                    .stroke(Brush::Solid(Color::WHITE), 1.0)
                    .build(),
            );
            scene
        };
        let (base, cursor) = (line(1.0), line(7.0));
        let pair = scenes_to_svg_pair(
            &[&base],
            &[&cursor],
            &PlotDescription::new(20.0, 20.0, "#000"),
        )
        .unwrap();
        assert!(pair.clean.contains("M1") && !pair.clean.contains("M7"));
        assert!(pair.with_overlay.contains("M1") && pair.with_overlay.contains("M7"));
    }
}
