//! Mapping foundations; the offline rendering preview is the first integration gate.
mod camera;
mod color_legend;
pub(crate) mod color_paths;
pub(crate) mod color_settings;
pub(crate) mod display_options;
pub(crate) mod export;
mod fit3;
pub(crate) mod flight;
mod flight3;
mod flight3_buffer;
mod gpu;
mod gradient_controls;
mod gradient_legend_row;
mod gradient_range;
mod gradient_stop;
mod ground_mesh;
mod ground_path;
mod hover;
mod layer_option;
pub(crate) mod legend;
mod measure;
mod modes;
pub mod preview;
mod profile;
mod scale;
pub(crate) mod settings;
pub(crate) mod telemetry;
mod terrain;
mod terrain_gpu;
mod terrain_view;
mod throttle;
mod tiles;
mod toolbar;
pub(crate) mod trace_controls;
mod trace_detail;
mod trace_markers;
pub(crate) mod trace_quick;

#[cfg(test)]
mod performance_replay;
#[cfg(test)]
mod test_renderer;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod ui_tests;

pub(crate) use tiles::{Config as TileConfig, Service};
