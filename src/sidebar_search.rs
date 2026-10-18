use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn SidebarSearch(
    value: RwSignal<String, LocalStorage>,
    placeholder: &'static str,
    label: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        row(class = "sidebar-search-row") {
            InputGroup(class = "field-search") {
                InputGroupInput(value = value, placeholder = placeholder, a11y:label = label)
                if move || !value.with(String::is_empty) {
                    InputGroupAddon(align = InputGroupAddonAlign::InlineEnd) {
                        InputGroupButton(size = InputGroupButtonSize::IconXs, a11y:label = "Clear search",
                            on:click = move |_| value.set(String::new())) {
                            vector(class = "search-clear-icon", prop:viewBox = "0 0 20 20",
                                prop:d = "M5 5 L15 15 M15 5 L5 15", a11y:hidden = true)
                        }
                    }
                }
            }
            {children.into_view_once()}
        }
    }
}
