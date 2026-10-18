use crate::{action_button::ActionButtonProps, update_state::UpdateState, updates};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn UpdateControls(state: UpdateState) -> impl IntoView {
    install_stylesheet("enlumi-updates", include_str!("updates.css"));
    let control = NodeRef::new();
    view! {
        Popover {
            PopoverTrigger(class = "app-action updates-trigger", size = ButtonSize::Xs, variant = ButtonVariant::Ghost) {
                {move || if state.pending.get().is_some() || state.failed.get() { "Updates •" } else { "Updates" }}
            }
            PopoverContent(class = "updates-menu") {
                text(class = "heading") { "Updates" }
                text(class = "muted") { {format!("Enlumi {} · {}", env!("CARGO_PKG_VERSION"), if updates::BUILD_COMMIT.is_empty() { "unknown revision" } else { &updates::BUILD_COMMIT[..8] })} }
                row(class = "updates-option") {
                    Checkbox(node_ref = control, disabled = Signal::derive_local(move || state.busy.get()), checked = Binding::controlled(
                        Signal::derive_local(move || if state.preferences.with(|p| p.nightly) { Checked::Yes } else { Checked::No }),
                        move |value| state.enable(value == Checked::Yes)))
                    Label(control = control) { "Enable nightly updates" }
                }
                text(class = "muted") { "Checks master once at startup when enabled. Enable to check on next startup." }
                text(class = "muted") { "Choose Update & restart to install completed nightly. Restart closes current log and unsaved work." }
                text(class = "updates-status", a11y:role = Role::Status) {{move || state.message.get()}}
                if move || state.ready.get().is_some() {
                    if move || state.install_problem.get_value().is_none() {
                        ActionButton(prominent = true, disabled = Signal::derive_local(move || state.busy.get()), on:click = move |_| state.install()) { "Update & restart" }
                    } else {
                        text(class = "muted") {{move || state.install_problem.get_value().unwrap_or_default()}}
                    }
                }
                if move || state.busy.get() && !state.installing.get() {
                    ActionButton(on:click = move |_| state.cancel_download()) { "Cancel download" }
                }
                row(class = "updates-actions") {
                    ActionButton(on:click = move |_| state.open(None)) { "Downloads ↗" }
                }
            }
        }
    }
}
