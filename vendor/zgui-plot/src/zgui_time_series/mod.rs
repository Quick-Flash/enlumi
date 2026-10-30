//! Ready-to-use multiple-series time plot. ULog ingestion stays with consumer.
mod plot;
mod state;
use crate::{LegendTheme, TextStyle};
pub use plot::*;
pub use state::{SeriesHit, TimeSeriesState};

pub fn tick_style() -> TextStyle {
    crate::zgui_plot::theme::tick_text()
}
pub fn legend_theme() -> LegendTheme {
    crate::zgui_plot::theme::LEGEND
}
/// Install alongside `zgui_surface::stylesheet()`.
pub fn stylesheet() -> String {
    crate::zgui_plot::stylesheet()
}
