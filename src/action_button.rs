//! Use for app commands; raw Button is reserved for selectable library rows.
//! Fixed variant prevents primary/ghost drift between toolbars and filter actions.
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn ActionButton(
    #[prop(default = false)] icon: bool,
    #[prop(into, default = Signal::stored_local(false))] disabled: Signal<bool, LocalStorage>,
    #[prop(optional)] node_ref: Option<NodeRef>,
    #[prop(into, optional)] class: Classes,
    #[prop(attrs)] attrs: Attrs,
    children: Children,
) -> impl IntoView {
    install_stylesheet("enlumi-action-button", include_str!("action_button.css"));
    let mut class = class;
    class.push(zgui::view::ClassName::new("app-action"));
    let element = node_ref.unwrap_or_default();
    let size = if icon {
        ButtonSize::IconXs
    } else {
        ButtonSize::Xs
    };
    view! {
        Button(variant = ButtonVariant::Ghost, size = size, disabled = disabled,
            node_ref = element, class = class, {..attrs}) { {children.into_view_once()} }
    }
}
