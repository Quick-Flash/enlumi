use crate::state::{Pane, Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PlotActions(workbench: Workbench, pane: Pane) -> impl IntoView {
    view! {
        Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "duplicate-plot-button", a11y:label = "Duplicate plot",
            on:click = move |_| workbench.duplicate_plot(pane)) { "Duplicate" }
        Button(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "maximize-button", on:click = move |_| workbench.maximize(pane.id)) {
            {move || if workbench.maximized.get() == Some(pane.id) { "Restore" } else { "Maximize" }}
        }
    }
}
