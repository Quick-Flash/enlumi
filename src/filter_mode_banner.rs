use crate::state::Workbench;
use zgui::prelude::*;

#[component]
pub fn FilterModeBanner(workbench: Workbench) -> impl IntoView {
    view! {
        row(class = "filter-mode-banner", class:filter-mode-edit = move || !workbench.filter_mode.get().is_new()) {
            text(class = "filter-mode-title", a11y:role = Role::Status) {{move || {
                let mode = workbench.filter_mode.get();
                if mode.is_new() { "Create derived fields".to_owned() }
                else if mode.ids().len() == 1 { workbench.derived.with(|fields| fields.iter().find(|field| field.id == mode.ids()[0]).map_or_else(|| "Editing saved field".to_owned(), |field| format!("Editing {}",field.name))) }
                else { format!("Editing {} fields",mode.ids().len()) }
            }}}
        }
    }
}
