//! Mapping foundations; the offline rendering preview is the first integration gate.
mod camera;
mod color_legend;
pub(crate) mod color_paths;
pub(crate) mod color_settings;
pub(crate) mod export;
mod fit3;
pub(crate) mod flight;
mod flight3;
mod gpu;
mod gradient_controls;
mod gradient_legend_row;
mod gradient_stop;
mod ground_path;
mod hover;
pub mod preview;
mod scale;
pub(crate) mod settings;
pub(crate) mod telemetry;
mod terrain;
mod terrain_gpu;
mod terrain_view;
mod tiles;
mod toolbar;
pub(crate) mod view_options;

#[cfg(test)]
mod test_renderer;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod ui_tests;

pub(crate) use tiles::{Config as TileConfig, Service};
