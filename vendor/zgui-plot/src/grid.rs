use kurbo::BezPath;

use crate::axis::tick_levels;
use crate::{PlotGeometry, Ranges, append_line};

/// Grid spacing limits and fading for [`grid_paths`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridOptions {
    pub x_min_step: f64,
    pub y_min_step: f64,
    pub min_pixel_spacing: f64,
    pub max_pixel_spacing: f64,
    pub max_opacity: f64,
}

impl Default for GridOptions {
    fn default() -> Self {
        Self {
            x_min_step: 0.0,
            y_min_step: 0.0,
            min_pixel_spacing: 8.0,
            max_pixel_spacing: 300.0,
            max_opacity: 0.8,
        }
    }
}

/// Opacity of a grid level whose lines are `spacing` pixels apart: zero at
/// `min_pixel_spacing`, rising with a square-root curve to `max_opacity` at
/// `max_pixel_spacing` (egui_plot grid fade).
pub fn grid_opacity(
    spacing: f64,
    min_pixel_spacing: f64,
    max_pixel_spacing: f64,
    max_opacity: f64,
) -> f64 {
    if max_pixel_spacing <= min_pixel_spacing {
        return if spacing >= max_pixel_spacing {
            max_opacity
        } else {
            0.0
        };
    }
    ((spacing - min_pixel_spacing) / (max_pixel_spacing - min_pixel_spacing))
        .clamp(0.0, 1.0)
        .sqrt()
        * max_opacity
}

/// Create ordered vertical and horizontal grid paths for numeric axis ranges.
pub fn grid_paths(
    ranges: impl Into<Ranges>,
    geometry: PlotGeometry,
    options: GridOptions,
) -> Vec<(BezPath, f64)> {
    let Ranges {
        x: x_range,
        y: y_range,
    } = ranges.into();
    let mut layers = Vec::new();
    for (vertical, range, pixels, min_step) in [
        (true, x_range, geometry.data_width(), options.x_min_step),
        (false, y_range, geometry.data_height(), options.y_min_step),
    ] {
        for level in tick_levels(range, pixels, options.min_pixel_spacing, min_step) {
            let opacity = grid_opacity(
                level.spacing,
                options.min_pixel_spacing,
                options.max_pixel_spacing,
                options.max_opacity,
            );
            if opacity <= 0.0 {
                continue;
            }
            let mut path = BezPath::new();
            for value in level.values(range) {
                if vertical {
                    let x = geometry.x(value, x_range);
                    append_line(&mut path, (x, geometry.top), (x, geometry.bottom));
                } else {
                    let y = geometry.y(value, y_range);
                    append_line(&mut path, (geometry.left, y), (geometry.right, y));
                }
            }
            layers.push((path, opacity));
        }
    }
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_is_continuous_and_capped() {
        assert_eq!(grid_opacity(8.0, 8.0, 300.0, 0.8), 0.0);
        assert_eq!(grid_opacity(300.0, 8.0, 300.0, 0.8), 0.8);
        assert!(grid_opacity(8.000001, 8.0, 300.0, 0.8) < 0.001);
        let opacities: Vec<_> = (8..=300)
            .map(|spacing| grid_opacity(f64::from(spacing), 8.0, 300.0, 0.8))
            .collect();
        assert!(opacities.windows(2).all(|pair| pair[1] >= pair[0]));
    }

    #[test]
    fn grid_layers_stay_inside_plot_bounds() {
        let geometry = PlotGeometry::with_bounds(10.0, 5.0, 410.0, 305.0, 420.0, 320.0).unwrap();
        let layers = grid_paths(((0.0, 10.0), (0.0, 5.0)), geometry, GridOptions::default());
        assert!(!layers.is_empty());
        for (path, opacity) in layers {
            assert!(opacity > 0.0 && opacity <= 0.8);
            let bounds = kurbo::Shape::bounding_box(&path);
            if !path.is_empty() {
                assert!(bounds.x0 >= 10.0 && bounds.x1 <= 410.0);
                assert!(bounds.y0 >= 5.0 && bounds.y1 <= 305.0);
            }
        }
    }
}
