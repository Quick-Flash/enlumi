#![recursion_limit = "256"]
mod app;
mod browser;
mod derived;
mod derived_state;
#[cfg(test)]
mod editor;
mod field_row;
mod field_selection;
mod field_tree;
mod filter;
mod filter_editor;
mod filter_source_row;
mod filter_sources;
mod filter_workspace;
mod fps;
mod legend_drag;
mod line_colors;
mod log;
mod panel;
mod plot_actions;
mod plot_bounds;
mod plot_cursor;
mod startup;
mod state;
mod workspace;

use app::WorkbenchAppProps;
use zgui::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    startup::reveal_if_first_frame_stalls();
    app()
        .with_application_id("com.tealdrones.enlumi")
        .with_title("Enlumi - PX4 log workbench")
        .with_size(1500.0, 850.0)
        .with_stylesheet(format!(
            "{}\n{}\n{}\n{}",
            plot::zgui_surface::stylesheet(),
            plot::zgui_time_series::stylesheet(),
            plot::zgui_workspace::stylesheet(),
            include_str!("style.css")
        ))
        .run_on(
            startup::desktop_driver(),
            move || view! { WorkbenchApp(initial_path = path.clone()) },
        )?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod layout_tests;
