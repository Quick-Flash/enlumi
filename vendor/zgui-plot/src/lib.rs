//! Backend-neutral 2D plotting primitives built on [`kurbo`] paths.
//!
//! The crate turns data and axis ranges into screen-space [`kurbo::BezPath`]s;
//! drawing them, choosing colours and handling input stay with the caller.
//!
//! - [`PlotGeometry`]: canvas size and data rectangle; maps values to pixels and back.
//! - [`grid_paths`], [`label_marks`], [`axis`]: decimal grid lines and fading axis labels.
//! - [`labels`]: screen placement of tick labels around the data area.
//! - [`line_segments`], [`scatter_path`], [`nearest`]: series geometry and pointer snapping.
//! - [`histogram`]: binning, bars, stacked bars and cumulative curves.
//! - [`overlay`]: hover crosshairs, fit lines, dashed limits, shaded bands, margin masks.
//! - [`Range`] / [`Ranges`] and [`range`]: axis ranges; pan, zoom, edge drag, box selection.
//! - [`interaction`]: hit zones, drag, box zoom and wheel zoom in screen pixels.
//! - [`legend`] and [`PlotDescription`]: text and boxes for vector export.
//!
//! The `zgui` feature adds the zgui integration:
//!
//! - `zgui_surface`: `PlotSurface`, the interactive plot shell (layers, tick
//!   labels, pointer/wheel/keyboard input), and its layout stylesheet.
//! - `zgui_pointer`: `PlotPointer`, the reactive wrapper around
//!   [`interaction::Controller`] that the surface drives.
//! - `zgui_legend`: interactive legend entries and their stylesheet.
//! - `zgui_renderer`: push paths into a canvas scene; pointer coordinates.
//! - `zgui_svg`: export canvas scenes as SVG.
//!
//! [`workspace`] supplies backend-neutral docking and assignment models.
//! The optional `workspace` feature adds `zgui_workspace` controls: positioned
//! pane host, animated tabs, split resizing, settings, and signal drag/drop.
//!
//! ```
//! use zgui_plot::{GridOptions, PlotGeometry, PlotPoint, grid_paths, line_segments, nearest};
//!
//! // 400x300 canvas with a 30px axis strip on the left and bottom.
//! let geometry = PlotGeometry::with_bounds(30.0, 0.0, 400.0, 270.0, 400.0, 300.0).unwrap();
//! let ranges = ((0.0, 10.0), (0.0, 1.0));
//! let points = [PlotPoint { x: 0.0, y: 0.2 }, PlotPoint { x: 5.0, y: 0.9 }];
//!
//! let grid = grid_paths(ranges, geometry, GridOptions::default());
//! let lines = line_segments(&points, ranges, geometry);
//! assert!(!grid.is_empty());
//! assert_eq!(lines.elements().len(), 2);
//!
//! // Snap a pointer at pixel (215, 30) to the nearest point within 10px.
//! let target = PlotPoint {
//!     x: geometry.x_value(215.0, ranges.0),
//!     y: geometry.y_value(30.0, ranges.1),
//! };
//! let pixels = (geometry.data_width(), geometry.data_height());
//! assert_eq!(nearest(points, target, ranges, pixels, 10.0), Some(1));
//! ```

pub mod axis;
#[cfg(feature = "zgui")]
pub mod preparation;
mod description;
mod geometry;
mod grid;
pub mod histogram;
pub mod histogram_layers;
mod hit;
pub mod interaction;
pub mod labels;
pub mod line_draw;
pub mod line_pattern;
pub mod overlay;
pub mod palette;
mod path;
pub mod plot_data;
pub mod range;
pub mod series_samples;
pub mod time_series;
#[cfg(feature = "workspace")]
pub mod zgui_series;

#[cfg(feature = "zgui")]
pub mod zgui_plot;

/// PNG rendering from same SVG document used for vector exports.
#[cfg(feature = "raster")]
pub mod raster;

/// Native image clipboard, using the same SVG renderer as PNG export.
#[cfg(feature = "clipboard")]
pub mod clipboard;
mod series;

#[cfg(feature = "zgui")]
pub mod zgui_legend;

/// Optional linked bounds and data-space pointer across plot surfaces.
#[cfg(feature = "zgui")]
pub mod zgui_link;

#[cfg(feature = "zgui")]
pub mod zgui_pointer;

#[cfg(feature = "zgui")]
pub mod zgui_renderer;

#[cfg(feature = "zgui")]
pub mod zgui_surface;

/// Generic multiple-series time plots, retained data, hover, and snapshot registration.
#[cfg(feature = "zgui")]
pub mod zgui_time_series;

/// SVG export of zgui canvas scenes.
#[cfg(feature = "zgui")]
pub mod zgui_svg;

pub use axis::{LabelOptions, format_number, label_marks};
pub use description::{
    LegendEntryStyle, LegendMarker, LegendStyle, LegendTheme, PlotDescription, SvgLegendEntry,
    SvgRect, SvgText, TITLE_LEGEND_TOP, TITLE_TOP, TextAnchor, TextStyle, css_color, legend,
    tick_label_text, title_text_style,
};
pub use geometry::PlotGeometry;
pub use grid::{GridOptions, grid_opacity, grid_paths};
pub use histogram::Bin;
pub use hit::{nearest, nearest_sorted};
pub use path::{
    append_circle, append_line, append_rect, append_top_rounded_rect, cross_marker, diamond_marker,
    triangle_marker,
};
pub use range::{Range, Ranges};
pub use series::{PlotPoint, line_segments, scatter_path};

/// Backend-neutral docking tree and plot assignment model.
pub mod workspace;

/// Optional native workspace controls, enabled by the `workspace` feature.
#[cfg(feature = "workspace")]
pub mod zgui_workspace;
