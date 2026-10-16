use crate::{action_button::ActionButtonProps, state::Workbench};
use zgui::prelude::*;

#[component]
pub fn FilterSaveButton(
    workbench: Workbench,
    #[prop(default = false)] prominent: bool,
) -> impl IntoView {
    view! {
        ActionButton(prominent = prominent, class = "save-derived-field", disabled = Signal::derive_local(move || workbench.loading.get() || workbench.script_busy.get() || workbench.derived_busy.get() || workbench.log.get().is_none() || workbench.filter_input_ui.drag.get().is_some() || workbench.filter_inputs.with(|inputs| inputs.iter().all(|input| input.source.is_empty()) || workbench.filter_kind.get() == "rpy" && (inputs.len() != 4 || inputs.iter().any(|input| input.source.is_empty())))),
                    on:click = move |_| workbench.save_filter()) {{move || if workbench.derived_busy.get() { "Computing\u{2026}".to_owned() } else if !workbench.filter_mode.get().is_new() { "Save changes".to_owned() } else if workbench.filter_kind.get() == "rpy" { "Create 3 fields".to_owned() } else if workbench.filter_kind.get() == "rust" { "Compile & create".to_owned() } else if workbench.filter_kind.get() == "magnitude" { "Create magnitude".to_owned() } else { format!("Create {} fields", workbench.filter_inputs.with(Vec::len)) }}}
    }
}
