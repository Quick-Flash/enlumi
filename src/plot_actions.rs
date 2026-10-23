use crate::action_button::ActionButtonProps;
use crate::plot_appearance::PlotAppearanceProps;
use crate::plot_menu::PlotPresetMenuProps;
use crate::screenshot::ScreenshotProps;
use crate::state::{Pane, Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PlotActions(workbench: Workbench, pane: Pane) -> impl IntoView {
    view! {
        row(class="plot-actions") {
        Screenshot(workbench=workbench,pane=pane)
        PlotPresetMenu(workbench=workbench,pane=pane)
        PlotAppearance(workbench=workbench,pane=pane)
            Popover {
                PopoverTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost,
                    class="plot-units",a11y:label = "Plot units options") { "Units" }
                PopoverContent {
                    text(class = "muted") {{move || {let units=workbench.plot_units(pane);if units.is_empty(){"No unit labels on this plot".into()}else{format!("Unit labels: {}",units.into_iter().collect::<Vec<_>>().join(", "))}}}}
                    ActionButton(disabled=Signal::derive_local(move ||workbench.plot_units(pane).len()<2),on:click = move |_| workbench.separate_plot_units(pane)) { "Separate plots" }
                    if move ||workbench.plot_units(pane).len()<2 {text(class="muted") {"Separate plots becomes available when unit labels differ."}}
                }
            }

        ActionButton(class = "duplicate-plot-button", a11y:label = "Duplicate plot",
            on:click = move |_| workbench.duplicate_plot(pane)) { "Duplicate" }
        ActionButton(class="maximize-button",class:setting-active=move || workbench.maximized().get()==Some(pane.id),a11y:label=move || if workbench.maximized().get()==Some(pane.id){"Restore plot size"}else{"Maximize plot"},on:click=move |_|workbench.maximize(pane.id)) {
            {move ||if workbench.maximized().get()==Some(pane.id){"Restore"}else{"Maximize"}}
        }
        }
    }
}
