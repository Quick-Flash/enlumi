use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn TabName(workbench: Workbench, pane: Pane) -> impl IntoView {
    let open = RwSignal::new_local(false);
    let name = RwSignal::new_local(String::new());
    let save = move || {
        let title = name
            .get_untracked()
            .trim()
            .chars()
            .take(80)
            .collect::<String>();
        workbench.profiles.titles.update(|titles| {
            if title.is_empty() {
                titles.remove(&pane.id);
            } else {
                titles.insert(pane.id, title);
            }
        });
        open.set(false);
    };
    view! {
        Popover(open = open, on_open_change = UnsyncCallback::new(move |is_open| {
            if is_open { name.set(workbench.profiles.titles.with_untracked(|titles| titles.get(&pane.id).cloned().unwrap_or_default())); }
        })) {
            PopoverTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Ghost, class = "app-action rename-tab", a11y:label = "Rename plot tab") { "Rename" }
            PopoverContent(class = "tab-name-editor") {
                text(class = "heading") { "Tab name" }
                Input(value = name, placeholder = format!("Plot {}", pane.id), a11y:label = "Tab name",
                    on:key_down = move |event| {
                        if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter) { save(); event.prevent_default(); event.stop_propagation(); }
                    })
                row { ActionButton(on:click = move |_| save()) { "Save" }
                    ActionButton(on:click = move |_| { name.set(String::new()); save(); }) { "Reset" }
                }
            }
        }
    }
}
