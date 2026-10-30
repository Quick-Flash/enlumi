//! Editable series plots over caller-owned data and application callbacks.
mod model;
pub use model::*;

mod color_picker;
mod editor;
mod legend;
mod legend_drag;
mod legend_entry;
pub use legend_entry::*;
mod line_style;
mod plot_editor;
pub use color_picker::*;
pub use editor::*;
pub use legend_drag::*;
pub use line_style::*;
pub fn stylesheet() -> &'static str {
    include_str!("series.css")
}
fn install() {
    zgui::prelude::install_stylesheet("plot-series-controls", stylesheet());
}

mod plot;
pub use plot::*;
