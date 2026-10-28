use crate::{action_button::ActionButtonProps, update_state::UpdateState, updates};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn UpdateControls(state: UpdateState) -> impl IntoView {
    install_stylesheet("enlumi-updates", include_str!("updates.css"));
    let control = NodeRef::new();
    view! {
        Dialog {
            DialogTrigger(class = "app-action updates-trigger", size = ButtonSize::Xs, variant = ButtonVariant::Ghost) {
                {move || if state.ready.get().is_some() { "Update available" } else if state.failed.get() { "Update issue" } else { "Updates" }}
            }
            DialogContent(class = "updates-menu") {
                column(class="updates-header") {
                    DialogTitle(class="heading") {"App updates"}
                    text(class="muted") {{format!("Installed · {} · {}",env!("CARGO_PKG_VERSION"),updates::BUILD_COMMIT.get(..8).unwrap_or("unknown revision"))}}
                }
                column(class="updates-card",class:update-error=move ||state.failed.get()) {
                    text(class="heading") {{move || if state.installing.get() {"Restarting Enlumi"} else if state.busy.get() {"Downloading update"} else if state.checking.get() {"Checking for updates"} else if state.failed.get() {"Update needs attention"} else if state.ready.get().is_some() {"New nightly available"} else if state.pending.get().is_some() {"New build in progress"} else if state.preferences.with(|p|p.nightly) {"Nightly channel"} else {"Updates are off"}}}
                    if move ||state.ready.get().is_some() {
                        text(class="updates-revision") {{move ||state.ready.with(|release|release.as_ref().map(|release|format!("Build {}",&release.commit[..8])).unwrap_or_default())}}
                    }
                    text(class="updates-status",a11y:role=Role::Status) {{move ||state.message.get()}}
                    if move ||state.ready.get().is_some() && !state.busy.get() {
                        if move ||state.install_problem.get_value().is_none() {
                            ActionButton(prominent=true,disabled=Signal::derive_local(move ||state.checking.get()),on:click=move |_|state.install()) {"Update & restart"}
                            text(class="muted") {"Installs update, then restarts. Current log and unsaved session will close."}
                        } else {
                            text(class="muted") {"Automatic installation unavailable for this build."}
                            text(class="muted") {{state.install_problem.get_value().unwrap_or_default()}}
                            ActionButton(on:click=move |_|state.open(None)) {"Download nightly ↗"}
                        }
                    }
                    if move ||state.busy.get() && !state.installing.get() {
                        ActionButton(on:click=move |_|state.cancel_download()) {"Cancel download"}
                    }
                }
                row(class="updates-option") {
                    column(class="updates-option-copy") {
                        Label(control=control) {"Nightly builds"}
                        text(class="muted") {"Check at startup and retry while a new build is in progress. Changes take effect next launch."}
                    }
                    Checkbox(node_ref=control,disabled=Signal::derive_local(move ||state.busy.get()),checked=Binding::controlled(
                        Signal::derive_local(move ||if state.preferences.with(|p|p.nightly){Checked::Yes}else{Checked::No}),
                        move |value|state.enable(value==Checked::Yes)))
                }
                row(class="updates-actions") {
                    ActionButton(class="updates-check",disabled=Signal::derive_local(move ||state.busy.get() || state.checking.get()),on:click=move |_|state.check()) {"Check for updates"}
                    if move ||state.pending.get().is_some() {
                        ActionButton(on:click=move |_|state.open(state.pending.get_untracked())) {"View changes ↗"}
                    }
                    ActionButton(on:click=move |_|state.open(None)) {"All downloads ↗"}
                    DialogClose(class="app-action updates-done",size=ButtonSize::Xs,variant=ButtonVariant::Ghost) {"Done"}
                }
            }
        }
    }
}
