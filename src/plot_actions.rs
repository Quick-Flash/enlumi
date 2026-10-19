use crate::action_button::ActionButtonProps;
use crate::state::{Pane, Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PlotActions(workbench: Workbench, pane: Pane) -> impl IntoView {
    view! {
        if move || workbench.plot_units(pane).len() > 1 {
            Popover {
                PopoverTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost,
                    a11y:label = "Mixed units options") { "Units" }
                PopoverContent {
                    text(class = "muted") {{move || format!("Mixed unit labels: {}", workbench.plot_units(pane).into_iter().collect::<Vec<_>>().join(", "))}}
                    ActionButton(on:click = move |_| workbench.separate_plot_units(pane)) { "Separate plots" }
                }
            }
        }

        if move || pane.analysis.get().is_none() { ActionButton(on:click=move |_|workbench.analyze_plot(pane)) {"Analyze"} }
        if move || pane.analysis.get().is_some() { ActionButton(on:click=move |_|workbench.edit_analysis(pane)) {"Analysis settings"} }
        ActionButton(class = "duplicate-plot-button", a11y:label = "Duplicate plot",
            on:click = move |_| workbench.duplicate_plot(pane)) { "Duplicate" }
        ActionButton(class = "maximize-button", class:setting-active = move || workbench.maximized.get() == Some(pane.id), on:click = move |_| workbench.maximize(pane.id)) {
            {move || if workbench.maximized.get() == Some(pane.id) { "Restore" } else { "Maximize" }}
        }
    }
}
