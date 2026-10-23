#![recursion_limit = "256"]
mod action_button;
mod analysis_heatmap;
mod analysis_plot;
mod analysis_range;
mod analysis_source;
mod analysis_state;
mod analysis_workspace;
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
mod browser_fields;
mod derived;
mod derived_state;
#[cfg(target_os = "linux")]
mod desktop_icon;
#[cfg(test)]
mod editor;
mod field_actions;
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
mod measurement_background;
mod measurement_overlay;
mod measurement_state;
mod panel;
mod parameter_details;
mod parameter_rows;
mod plot_actions;
mod plot_appearance;
mod plot_bounds;
mod plot_cursor;
mod plot_menu;
mod preferences;
mod preset;
mod preset_apply;
mod preset_browser;
mod preset_capture;
mod preset_delete;
mod preset_dialog;
mod preset_lookup;
mod preset_reuse;
mod preset_store;
mod preset_ui;
mod profile;
mod profile_commit;
mod profile_delete;
mod profile_geometry;
mod profile_instance;
mod profile_library;
mod profile_output_row;
mod profile_outputs;
mod profile_placement;
mod profile_preferences;
mod profile_preset_tree;
mod profile_selection;
mod profile_state;
mod profile_view_source;
mod screenshot;
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
mod spectral;
mod startup;
mod state;
mod state_bands;
mod theme;
mod theme_picker;
#[cfg(test)]
mod theme_tests;
mod tree_lines;
mod update_controls;
mod update_download;
mod update_install;
mod update_notice;
mod update_release;
mod update_state;
mod updates;
mod view_picker;
mod view_state;
#[cfg(test)]
mod view_tests;
mod workspace;
mod workspace_catalogue;
mod workspace_collection;
mod workspace_loading;
mod workspace_picker;
mod workspace_preset;
mod workspace_state;
#[cfg(test)]
mod workspace_tests;

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
    if let Some(file) = path
        .as_ref()
        .filter(|path| path.is_file())
        .and_then(|path| path.canonicalize().ok())
    {
        let _ = preferences::LastLogDirectory::user().save(&file);
    }
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

#[cfg(test)]
mod spectral_tests;

#[cfg(test)]
mod preset_tests;
