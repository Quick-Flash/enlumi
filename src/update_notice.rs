use crate::{action_button::ActionButtonProps, update_state::UpdateState};
use zgui::prelude::*;

#[component]
pub fn UpdateNotice(state: UpdateState) -> impl IntoView {
    view! {
        if move || state.pending.get().is_some() || state.failed.get() || state.busy.get() {
            row(class = "update-notice", a11y:role = Role::Status) {
                text { {move || state.message.get()} }
                if move || state.ready.get().is_some() && state.install_problem.get_value().is_none() {
                    ActionButton(prominent = true, disabled = Signal::derive_local(move || state.busy.get() || state.checking.get()), on:click = move |_| state.install()) { "Update & restart" }
                }
                if move || state.ready.get().is_none() && !state.busy.get() {
                    ActionButton(class="updates-check", disabled=Signal::derive_local(move ||state.checking.get()), on:click=move |_|state.check()) {"Check for updates"}
                }
                if move || state.busy.get() && !state.installing.get() {
                    ActionButton(on:click = move |_| state.cancel_download()) { "Cancel download" }
                }
                if move || state.pending.get().is_some() {
                ActionButton(on:click = move |_| state.open(state.pending.get_untracked())) { "View changes ↗" }
                }
                ActionButton(on:click = move |_| state.open(None)) { "Downloads ↗" }
                ActionButton(disabled = Signal::derive_local(move || state.busy.get()), on:click = move |_| state.dismiss()) { "Dismiss" }
            }
        }
    }
}
