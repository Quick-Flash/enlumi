#![recursion_limit = "256"]
mod action_button;
mod app;
mod app_icon;
mod band_background;
mod band_mapping;
mod band_mapping_add;
mod band_mapping_edit;
mod band_mapping_file;
mod band_rule_row;
mod band_state;
mod browser;
mod derived;
mod derived_state;
#[cfg(target_os = "linux")]
mod desktop_icon;
#[cfg(test)]
mod editor;
mod field_label;
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
mod geometry;
mod geometry_appearance;
mod geometry_coordinates;
mod geometry_cursor;
mod geometry_editor_drag;
mod geometry_handle;
mod geometry_inspector;
mod geometry_interaction;
mod geometry_legend;
mod geometry_legend_row;
mod geometry_library;
mod geometry_overlay;
mod geometry_state;
mod github_link;
mod legend_drag;
mod line_colors;
mod log;
mod panel;
mod parameter_details;
mod parameter_rows;
mod plot_actions;
mod plot_bounds;
mod plot_cursor;
mod preferences;
mod profile;
mod profile_commit;
mod profile_instance;
mod profile_library;
mod profile_output_row;
mod profile_outputs;
mod profile_placement;
mod profile_preferences;
mod profile_selection;
mod profile_state;
mod profile_view_source;
mod script;
mod script_code_editor;
mod script_compile;
mod script_contract;
mod script_editor;
mod script_output_names;
mod script_state;
#[cfg(test)]
mod script_tests;
mod sidebar_help;
mod sidebar_search;
mod signal_provenance;
mod startup;
mod state;
mod state_bands;
mod theme;
mod theme_picker;
#[cfg(test)]
mod theme_tests;
mod update_controls;
mod update_download;
mod update_install;
mod update_notice;
mod update_release;
mod update_state;
mod updates;
mod workspace;

use app::WorkbenchAppProps;
use zgui::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if update_install::startup()? {
        return Ok(());
    }
    let path = std::env::args_os()
        .nth(1)
        .filter(|arg| arg != "--enlumi-finish-update")
        .map(std::path::PathBuf::from);
    startup::reveal_if_first_frame_stalls();
    app_icon::set_dock_icon();
    #[cfg(target_os = "linux")]
    if let Err(error) = desktop_icon::register() {
        eprintln!("Could not register Enlumi desktop icon: {error}");
    }
    app()
        .with_application_id("com.tealdrones.enlumi")
        .with_icon(app_icon::window())
        .with_title("Enlumi - Log workbench")
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
