use crate::action_button::IconActionProps;
use crate::plot_menu::PlotPresetMenuProps;
use crate::screenshot::ScreenshotProps;
use crate::state::{Pane, Workbench};
use zgui::prelude::*;

const EDIT_ICON: &str = "M10.5 2.5 L13.5 5.5 L6 13 H3 V10 Z";
const MAXIMIZE_ICON: &str = "M9.5 2.5 H13.5 V6.5 M6.5 13.5 H2.5 V9.5 M13.5 2.5 L9 7 M2.5 13.5 L7 9";
const RESTORE_ICON: &str = "M13.5 2.5 L9.5 6.5 M9.5 3 V6.5 H13 M2.5 13.5 L6.5 9.5 M3 9.5 H6.5 V13";

/// Pane header: frequent actions as icons; rare ones live in the plot actions menu.
#[component]
pub fn PlotActions(workbench: Workbench, pane: Pane) -> impl IntoView {
    let maximized = Signal::derive_local(move || workbench.maximized().get() == Some(pane.id));
    let is_map = move || {
        pane.trajectory
            .with(|s| s.as_ref().is_some_and(|s| s.map.is_some()))
    };
    view! {
        row(class="plot-actions") {
            if move ||pane.trajectory.with(Option::is_some) {
                IconAction(class="trajectory-open-setup",
                    label=Signal::derive_local(move ||if is_map(){"Edit coordinates".to_owned()}else{"Edit X/Y".to_owned()}),
                    path=Signal::stored_local(EDIT_ICON.to_owned()),
                    on_click=UnsyncCallback::new(move |()|workbench.edit_trajectory(pane)))
            }
            Screenshot(workbench=workbench,pane=pane)
            PlotPresetMenu(workbench=workbench,pane=pane)
            IconAction(class="maximize-button",active=maximized,
                label=Signal::derive_local(move ||if maximized.get(){"Restore plot size".to_owned()}else{"Maximize plot".to_owned()}),
                path=Signal::derive_local(move ||if maximized.get(){RESTORE_ICON.to_owned()}else{MAXIMIZE_ICON.to_owned()}),
                on_click=UnsyncCallback::new(move |()|workbench.maximize(pane.id)))
        }
    }
}
