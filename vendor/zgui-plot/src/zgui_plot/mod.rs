//! Shared plot state, chrome, annotations, and scatter/histogram components.
pub mod annotations;
mod histogram;
#[cfg(test)]
mod hover_tests;
mod plot;
mod render;
mod scatter;
mod state;
pub mod theme;
mod time_cache;
mod time_lines;
pub mod view_state;
pub use histogram::*;
pub use plot::*;
pub use scatter::*;
pub use state::*;

/// Shared screen/export chrome. Install with `zgui_surface::stylesheet()`.
pub fn stylesheet() -> String {
    format!(
        "{}\n{}",
        include_str!("../zgui_time_series/plot.css"),
        theme::stylesheet()
    )
}
