use crate::{action_button::ActionButtonProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn BandMappingAdd(workbench: Workbench, id: u64) -> impl IntoView {
    let value = RwSignal::new_local(String::new());
    let name = RwSignal::new_local(String::new());
    let add = move || {
        if workbench.edit_mapping_rule(id, None, &value.get_untracked(), &name.get_untracked()) {
            value.set(String::new());
            name.set(String::new());
        }
    };
    view! {column(class="band-mapping-add") {
        text(class="muted") {"Add custom state, including values absent from log"}
        row(class="band-add-row",on:key_down=move |event| {if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter) {add();event.prevent_default();event.stop_propagation();}}) {
            Input(class="band-new-value",value=value,a11y:label="New state value",placeholder="Value")
            Input(class="band-rule-name band-new-name",value=name,a11y:label="New state name",placeholder="State name")
            ActionButton(class="band-add-state",on:click=move |_|add()) {"Add"}
        }
    }}
}
