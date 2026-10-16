use crate::action_button::ActionButtonProps;
use crate::state::{Pane, Workbench};
use zgui::prelude::*;

#[component]
pub fn PlotActions(workbench: Workbench, pane: Pane) -> impl IntoView {
    view! {
        ActionButton(class = "duplicate-plot-button", a11y:label = "Duplicate plot",
            on:click = move |_| workbench.duplicate_plot(pane)) { "Duplicate" }
        ActionButton(class = "maximize-button", class:setting-active = move || workbench.maximized.get() == Some(pane.id), on:click = move |_| workbench.maximize(pane.id)) {
            {move || if workbench.maximized.get() == Some(pane.id) { "Restore" } else { "Maximize" }}
        }
    }
}
