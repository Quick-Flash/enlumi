//! Use for app commands; raw Button is reserved for selectable library rows.
//! Fixed variant prevents primary/ghost drift between toolbars and filter actions.
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn ActionButton(
    #[prop(default = false)] icon: bool,
    #[prop(default = false)] prominent: bool,
    #[prop(into, default = Signal::stored_local(false))] disabled: Signal<bool, LocalStorage>,
    #[prop(optional)] node_ref: Option<NodeRef>,
    #[prop(into, optional)] class: Classes,
    #[prop(attrs)] attrs: Attrs,
    children: Children,
) -> impl IntoView {
    install_stylesheet("enlumi-action-button", include_str!("action_button.css"));
    let mut class = class;
    class.push(zgui::view::ClassName::new("app-action"));
    if prominent {
        class.push(zgui::view::ClassName::new("action-prominent"));
    }
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

/// Square stroke-icon command with a hover tooltip; the label doubles as its accessible name.
#[component]
pub fn IconAction(
    label: Signal<String, LocalStorage>,
    /// SVG path in a 16×16 view box, drawn as a stroke.
    path: Signal<String, LocalStorage>,
    #[prop(into, default = Signal::stored_local(false))] active: Signal<bool, LocalStorage>,
    #[prop(into, optional)] class: Classes,
    on_click: UnsyncCallback<()>,
) -> impl IntoView {
    install_stylesheet("enlumi-action-button", include_str!("action_button.css"));
    let mut class = class;
    class.push(zgui::view::ClassName::new("icon-action"));
    view! {
        Tooltip {
            TooltipTrigger {
                ActionButton(icon=true,class=class,class:setting-active=move ||active.get(),
                    a11y:label=move ||label.get(),on:click=move |_|on_click.run(())) {
                    vector(class="icon-action-glyph",prop:viewBox="0 0 16 16",
                        prop:d=move ||PropValue::from(path.get()),a11y:hidden=true)
                }
            }
            // Beside the button: below would cover the plot's legend controls.
            TooltipContent(class="icon-action-tip",placement=Signal::stored_local(zgui_ui_primitives::Placement::new(zgui_ui_primitives::Side::Left,zgui_ui_primitives::Align::Center))) { text {{move ||label.get()}} }
        }
    }
}
