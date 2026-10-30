use crate::Range;

/// Plot drawing bounds inside a canvas, in canvas coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotGeometry {
    pub width: f64,
    pub height: f64,
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl PlotGeometry {
    /// Unit square at the origin. Paths built on it depend only on data, so
    /// they can be cached and placed on any plot with [`Self::unit_transform`].
    pub const UNIT: Self = Self {
        width: 1.0,
        height: 1.0,
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    };

    /// Maps paths built on [`Self::UNIT`] onto this geometry's data area.
    pub fn unit_transform(self) -> kurbo::Affine {
        kurbo::Affine::new([
            self.data_width(),
            0.0,
            0.0,
            self.data_height(),
            self.left,
            self.top,
        ])
    }

    /// Whether `bounds` (screen pixels) touches the data area.
    pub fn overlaps(self, bounds: kurbo::Rect) -> bool {
        bounds.x1 >= self.left
            && bounds.x0 <= self.right
            && bounds.y1 >= self.top
            && bounds.y0 <= self.bottom
    }

    /// Map data-space points to canvas space for `ranges`.
    ///
    /// Series can stay in data space while pan and zoom update only canvas view.
    pub fn data_transform(self, ranges: impl Into<crate::Ranges>) -> kurbo::Affine {
        let crate::Ranges { x, y } = ranges.into();
        let sx = self.data_width() / x.span();
        let sy = self.data_height() / y.span();
        kurbo::Affine::new([
            sx,
            0.0,
            0.0,
            -sy,
            self.left - x.min * sx,
            self.bottom + y.min * sy,
        ])
    }
    /// Build plot geometry with explicit plot bounds and canvas size.
    pub fn with_bounds(
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        width: f64,
        height: f64,
    ) -> Option<Self> {
        [left, top, right, bottom, width, height]
            .iter()
            .all(|value| value.is_finite())
            .then_some(Self {
                width,
                height,
                left,
                top,
                right,
                bottom,
            })
            .filter(|_| {
                left >= 0.0
                    && top >= 0.0
                    && right >= left
                    && bottom >= top
                    && width >= right
                    && height >= bottom
            })
    }

    /// Screen x of a data value.
    pub fn x(self, value: f64, range: impl Into<Range>) -> f64 {
        let range = range.into();
        self.left + (value - range.min) / range.span() * self.data_width()
    }

    /// Screen y of a data value; larger values are higher on screen.
    pub fn y(self, value: f64, range: impl Into<Range>) -> f64 {
        let range = range.into();
        self.bottom - (value - range.min) / range.span() * self.data_height()
    }

    /// Inverse of [`Self::x`].
    pub fn x_value(self, x: f64, range: impl Into<Range>) -> f64 {
        let range = range.into();
        range.min + (x - self.left) / self.data_width() * range.span()
    }

    /// Inverse of [`Self::y`].
    pub fn y_value(self, y: f64, range: impl Into<Range>) -> f64 {
        let range = range.into();
        range.max - (y - self.top) / self.data_height() * range.span()
    }

    /// Width of the data area in pixels.
    pub fn data_width(self) -> f64 {
        self.right - self.left
    }

    /// Height of the data area in pixels.
    pub fn data_height(self) -> f64 {
        self.bottom - self.top
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_edges_and_round_trips() {
        let geometry = PlotGeometry::with_bounds(10.0, 4.0, 630.0, 444.0, 640.0, 480.0).unwrap();
        assert_eq!(geometry.x(0.0, (0.0, 10.0)), 10.0);
        assert_eq!(geometry.x(10.0, (0.0, 10.0)), 630.0);
        assert_eq!(geometry.y(0.0, (0.0, 10.0)), 444.0);
        assert_eq!(geometry.y(10.0, (0.0, 10.0)), 4.0);
        assert!((geometry.x_value(geometry.x(3.3, (0.0, 10.0)), (0.0, 10.0)) - 3.3).abs() < 1e-12);
        assert!((geometry.y_value(geometry.y(3.3, (0.0, 10.0)), (0.0, 10.0)) - 3.3).abs() < 1e-12);
    }

    #[test]
    fn unit_paths_land_where_direct_paths_do() {
        let geometry = PlotGeometry::with_bounds(10.0, 4.0, 630.0, 444.0, 640.0, 480.0).unwrap();
        let range = (2.0, 7.0);
        for value in [2.0, 3.3, 7.0] {
            let unit = kurbo::Point::new(
                PlotGeometry::UNIT.x(value, range),
                PlotGeometry::UNIT.y(value, range),
            );
            let placed = geometry.unit_transform() * unit;
            assert!((placed.x - geometry.x(value, range)).abs() < 1e-9);
            assert!((placed.y - geometry.y(value, range)).abs() < 1e-9);
        }
    }

    #[test]
    fn overlap_includes_touching_edges() {
        let geometry = PlotGeometry::with_bounds(10.0, 4.0, 630.0, 444.0, 640.0, 480.0).unwrap();
        assert!(geometry.overlaps(kurbo::Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(!geometry.overlaps(kurbo::Rect::new(0.0, 0.0, 9.0, 10.0)));
        assert!(!geometry.overlaps(kurbo::Rect::new(100.0, 445.0, 120.0, 460.0)));
    }

    #[test]
    fn rejects_bounds_outside_canvas() {
        assert!(PlotGeometry::with_bounds(0.0, 0.0, 20.0, 10.0, 10.0, 10.0).is_none());
        assert!(PlotGeometry::with_bounds(5.0, 0.0, 4.0, 10.0, 10.0, 10.0).is_none());
        assert!(PlotGeometry::with_bounds(f64::NAN, 0.0, 4.0, 10.0, 10.0, 10.0).is_none());
    }
}
