//! Screen placement of axis tick labels.
//!
//! Y labels sit inside the data area along its left edge, vertically centred
//! on their tick. X labels are centred on their tick in a row at the bottom,
//! either inside the data area or below it. Every label is kept `inset` pixels
//! inside the horizontal bounds, and Y labels never overlap an inside X row.

use crate::{LabelOptions, PlotGeometry, Ranges, label_marks};

/// Which axis a label belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// Vertical position of the X label row relative to the data area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum XLabelRow {
    /// Along the inside of the bottom edge, `inset` pixels above it.
    Inside,
    /// Below the bottom edge, `gap` pixels under it.
    Below { gap: f64 },
}

/// Label box sizes and spacing, in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickLabelLayout {
    /// Width and height reserved for each X label.
    pub x_label: (f64, f64),
    /// Width and height reserved for each Y label.
    pub y_label: (f64, f64),
    /// Minimum distance between a label and the edges it is kept inside.
    pub inset: f64,
    pub x_row: XLabelRow,
}

impl TickLabelLayout {
    /// Top-left corner of the label box for a Y tick at screen `y`.
    pub fn y_label_origin(&self, y: f64, geometry: PlotGeometry) -> (f64, f64) {
        let lowest = match self.x_row {
            XLabelRow::Inside => self.x_row_top(geometry) - self.inset,
            XLabelRow::Below { .. } => geometry.bottom - self.inset,
        } - self.y_label.1;
        let highest = geometry.top + self.inset;
        let top = (y - self.y_label.1 / 2.0).min(lowest).max(highest);
        (geometry.left + self.inset, top)
    }

    /// Top-left corner of the label box for an X tick at screen `x`.
    pub fn x_label_origin(&self, x: f64, geometry: PlotGeometry) -> (f64, f64) {
        let leftmost = geometry.left + self.inset;
        let rightmost = (geometry.right - self.x_label.0 - self.inset).max(leftmost);
        let left = (x - self.x_label.0 / 2.0).max(leftmost).min(rightmost);
        (left, self.x_row_top(geometry))
    }

    fn x_row_top(&self, geometry: PlotGeometry) -> f64 {
        match self.x_row {
            XLabelRow::Inside => geometry.bottom - self.inset - self.x_label.1,
            XLabelRow::Below { gap } => geometry.bottom + gap,
        }
    }
}

/// A positioned tick label.
#[derive(Clone, Debug, PartialEq)]
pub struct TickLabel {
    pub axis: Axis,
    pub value: f64,
    pub text: String,
    pub opacity: f64,
    /// Label box in screen pixels: top-left corner and size.
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// Formatting and fade options for one axis.
pub struct AxisLabels<F: Fn(f64) -> String> {
    pub options: LabelOptions,
    pub format: F,
}

/// Visible tick labels for both axes, Y labels first, each group sorted by value.
pub fn tick_labels(
    ranges: impl Into<Ranges>,
    geometry: PlotGeometry,
    layout: &TickLabelLayout,
    x: AxisLabels<impl Fn(f64) -> String>,
    y: AxisLabels<impl Fn(f64) -> String>,
) -> Vec<TickLabel> {
    let Ranges {
        x: x_range,
        y: y_range,
    } = ranges.into();
    let y_labels = label_marks(y_range, geometry.data_height(), y.options)
        .into_iter()
        .map(|(value, opacity)| {
            let (left, top) = layout.y_label_origin(geometry.y(value, y_range), geometry);
            TickLabel {
                axis: Axis::Y,
                value,
                text: (y.format)(value),
                opacity,
                left,
                top,
                width: layout.y_label.0,
                height: layout.y_label.1,
            }
        });
    let x_labels = label_marks(x_range, geometry.data_width(), x.options)
        .into_iter()
        .map(|(value, opacity)| {
            let (left, top) = layout.x_label_origin(geometry.x(value, x_range), geometry);
            TickLabel {
                axis: Axis::X,
                value,
                text: (x.format)(value),
                opacity,
                left,
                top,
                width: layout.x_label.0,
                height: layout.x_label.1,
            }
        });
    y_labels.chain(x_labels).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format_number;

    fn geometry() -> PlotGeometry {
        PlotGeometry::with_bounds(0.0, 4.0, 500.0, 504.0, 500.0, 520.0).unwrap()
    }

    fn layout(x_row: XLabelRow) -> TickLabelLayout {
        TickLabelLayout {
            x_label: (72.0, 14.0),
            y_label: (64.0, 14.0),
            inset: 4.0,
            x_row,
        }
    }

    #[test]
    fn y_labels_centre_on_ticks_and_clear_inside_x_row() {
        let layout = layout(XLabelRow::Inside);
        assert_eq!(layout.y_label_origin(254.0, geometry()), (4.0, 247.0));
        assert_eq!(layout.y_label_origin(4.0, geometry()).1, 8.0);
        let lowest = layout.y_label_origin(504.0, geometry()).1;
        let x_row = layout.x_label_origin(250.0, geometry()).1;
        assert_eq!(x_row, 486.0);
        assert_eq!(lowest + 14.0 + 4.0, x_row);
    }

    #[test]
    fn x_labels_centre_on_ticks_and_stay_inside_horizontally() {
        let layout = layout(XLabelRow::Below { gap: 2.0 });
        assert_eq!(layout.x_label_origin(250.0, geometry()), (214.0, 506.0));
        assert_eq!(layout.x_label_origin(0.0, geometry()).0, 4.0);
        assert_eq!(layout.x_label_origin(500.0, geometry()).0, 424.0);
        assert_eq!(layout.y_label_origin(504.0, geometry()).1, 486.0);
    }

    #[test]
    fn labels_follow_grid_ticks_with_axis_formatting() {
        let labels = tick_labels(
            ((0.0, 5.0), (0.0, 100.0)),
            geometry(),
            &layout(XLabelRow::Inside),
            AxisLabels {
                options: LabelOptions::horizontal(0.0),
                format: |value| format!("x{value}"),
            },
            AxisLabels {
                options: LabelOptions::vertical(0.0),
                format: format_number,
            },
        );
        let first_x = labels
            .iter()
            .position(|label| label.axis == Axis::X)
            .unwrap();
        assert!(labels[..first_x].iter().all(|label| label.axis == Axis::Y));
        assert!(
            labels[first_x..]
                .iter()
                .all(|label| label.text.starts_with('x'))
        );
        // 500px over 0..5: unit steps are 100px apart and fully visible.
        let two = labels.iter().find(|label| label.text == "x2").unwrap();
        assert_eq!((two.left, two.opacity), (164.0, 1.0));
    }
}
