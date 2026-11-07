use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    state_key::StateKeyProps,
};
use zgui::prelude::*;
#[component]
pub fn StateTintOptions(workbench: Workbench, pane: Pane) -> impl IntoView {
    view! {column(class="state-tint-options") {
        ActionButton(class="state-trace-tint-toggle",class:setting-active=move ||pane.state_tint.get(),on:click=move |_|pane.state_tint.update(|value|*value= !*value)){{move ||if pane.state_tint.get(){"State-band trace tint: on"}else{"State-band trace tint: off"}}}
        text(class="muted"){"Adds visible state-band colors at band opacity. Keeps base trace colors, background bands, and state names."}
        StateKey(workbench=workbench,pane=pane)
    }}
}
