//! Shared axis fit context menu for data plots.
use crate::interaction::Axes;
use zgui::{
    prelude::*,
    reactive::{Callable, UnsyncCallback},
};
use zgui_ui::prelude::*;

/// Right-click fit actions. DataPlot installs this with `workspace` feature;
/// custom plot consumers may compose it around their own surface.
#[component]
pub fn PlotFitMenu(
    on_fit: UnsyncCallback<Axes>,
    #[prop(optional)] on_line_settings: Option<UnsyncCallback<()>>,
    #[prop(default = Signal::derive_local(|| true))] line_settings_enabled: Signal<
        bool,
        LocalStorage,
    >,
    children: Children,
) -> impl IntoView {
    install_stylesheet("plot-fit-menu", include_str!("fit_menu.css"));
    view! {
        ContextMenu {
            ContextMenuTrigger(class = "plot-fit-area") { {children.into_view_once()} }
            ContextMenuContent(class = "plot-fit-menu") {
                MenuItem(class = "plot-fit-x", on_select = UnsyncCallback::new(move |()| on_fit.run(Axes::X_ONLY))) { "Fit X axis" }
                MenuItem(class = "plot-fit-y", on_select = UnsyncCallback::new(move |()| on_fit.run(Axes { x: false, y: true }))) { "Fit Y axis" }
                MenuItem(class = "plot-fit-both", on_select = UnsyncCallback::new(move |()| on_fit.run(Axes::BOTH))) { "Fit both axes" }
                if move || on_line_settings.is_some() {
                    MenuSeparator {}
                    MenuItem(class = "plot-line-settings", disabled = Signal::derive_local(move || !line_settings_enabled.get()), on_select = UnsyncCallback::new(move |()| { if let Some(action) = on_line_settings { action.run(()); } })) { "Line settings" }
                }
            }
        }
    }
}
