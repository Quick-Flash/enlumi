#![recursion_limit = "256"]
mod action_button;
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
mod filter_commit;
mod filter_editor;
mod filter_extra;
#[cfg(test)]
mod filter_extra_tests;
mod filter_input_drag;
mod filter_kind_picker;
mod filter_library;
mod filter_library_card;
mod filter_mode;
mod filter_mode_banner;
mod filter_multi;
mod filter_output_names;
mod filter_parameters;
mod filter_quaternion_slot;
mod filter_save_button;
mod filter_source_row;
mod filter_sources;
mod filter_workspace;
mod filter_zero_phase;
mod fps;
mod legend_drag;
mod line_colors;
mod log;
mod panel;
mod plot_actions;
mod plot_bounds;
mod plot_cursor;
mod preferences;
mod script;
mod script_code_editor;
mod script_compile;
mod script_contract;
mod script_editor;
mod script_output_names;
mod script_state;
#[cfg(test)]
mod script_tests;
mod startup;
mod state;
mod theme;
mod theme_picker;
#[cfg(test)]
mod theme_tests;
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
            crate::theme::stylesheet()
        ))
        .run_on(
            startup::desktop_driver(),
            move || view! { WorkbenchApp(initial_path = path.clone(), preferences = crate::preferences::ThemePreference::user()) },
        )?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod layout_tests;
