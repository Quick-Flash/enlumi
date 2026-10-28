//! Client-side furniture for the native Wayland surface. Dimensions are logical pixels;
//! the compositor owns movement, resizing, snapping and the resulting window state.
use crate::action_button::ActionButtonProps;
use zgui::platform::{FullscreenMode, ResizeEdge};
use zgui::prelude::*;
use zgui::runtime::try_use_window;
use zgui::vocab::{Key, NamedKey};

#[component]
pub fn WindowFrame(
    #[prop(default = false)] enabled: bool,
    #[prop(attrs)] attrs: Attrs,
    children: Children,
) -> impl IntoView {
    install_stylesheet("enlumi-window-frame", include_str!("window_frame.css"));
    let window = try_use_window().filter(|_| enabled);
    let Some(window) = window else {
        return AnyView::new(view! {
            column(class = "desktop-window", {..attrs}) { {children.into_view_once()} }
        });
    };
    let maximized = window.maximized();
    let fullscreen = window.fullscreen();
    let focused = window.focused();
    let keyboard = window.clone();
    let minimize = window.clone();
    let maximize = window.clone();
    let close = window.clone();
    let enter_fullscreen = window.clone();
    let resize = window.clone();
    let anchor = NodeRef::new();
    let shortcut = zgui::reactive::RenderEffect::new(move |previous| {
        drop(previous);
        anchor.get();
        anchor.window_shortcut()
    });
    on_cleanup_local(move || drop(shortcut));
    AnyView::new(view! {
        column(class = "desktop-window client-frame", node_ref = anchor,
            class:window-maximized = move || maximized.get(),
            class:window-fullscreen = move || fullscreen.get().is_some(),
            class:window-inactive = move || !focused.get(),
            on:key_down = move |event| {
                let next = match event.key {
                    Key::Named(NamedKey::F11) if !event.repeat => Some(if fullscreen.get_untracked().is_some() { None } else { Some(FullscreenMode::Borderless) }),
                    Key::Named(NamedKey::Escape) if fullscreen.get_untracked().is_some() => Some(None),
                    _ => None,
                };
                if let Some(next) = next {
                    keyboard.set_fullscreen(next);
                    event.prevent_default();
                    event.stop_propagation();
                }
            }, {..attrs}) {
            if move || fullscreen.get().is_none() {
                row(class = "window-titlebar", on:pointer_down = window.move_drag_handler()) {
                    text(class = "window-title") { "Enlumi — Log workbench" }
                    row(class = "window-buttons", on:pointer_down = window.no_drag_handler()) {
                        ActionButton(icon = true, class = "window-button window-fullscreen-button", a11y:label = "Enter fullscreen (F11)",
                            on:click = { let window = enter_fullscreen.clone(); move |_| window.set_fullscreen(Some(FullscreenMode::Borderless)) }) {
                            vector(prop:viewBox = "0 0 20 20", prop:d = "M7 3 H3 V7 M13 3 H17 V7 M3 13 V17 H7 M17 13 V17 H13", a11y:hidden = true)
                        }
                        ActionButton(icon = true, class = "window-button window-minimize", a11y:label = "Minimize window",
                            on:click = { let window = minimize.clone(); move |_| window.minimize() }) {
                            vector(prop:viewBox = "0 0 20 20", prop:d = "M4 13 H16", a11y:hidden = true)
                        }
                        ActionButton(icon = true, class = "window-button window-maximize",
                            a11y:label = move || if maximized.get() { "Restore window" } else { "Maximize window" },
                            on:click = { let window = maximize.clone(); move |_| window.toggle_maximized() }) {
                            vector(prop:viewBox = "0 0 20 20", prop:d = move || PropValue::from(if maximized.get() { "M4 6 H14 V16 H4 Z M7 3 H17 V13" } else { "M4 4 H16 V16 H4 Z" }), a11y:hidden = true)
                        }
                        ActionButton(icon = true, class = "window-button window-close", a11y:label = "Close window",
                            on:click = { let window = close.clone(); move |_| window.close() }) {
                            vector(prop:viewBox = "0 0 20 20", prop:d = "M5 5 L15 15 M15 5 L5 15", a11y:hidden = true)
                        }
                    }
                }
            }
            {children.into_view_once()}
            if move || !maximized.get() && fullscreen.get().is_none() {
                {[
                    ("window-resize-n", ResizeEdge::North),
                    ("window-resize-s", ResizeEdge::South),
                    ("window-resize-e", ResizeEdge::East),
                    ("window-resize-w", ResizeEdge::West),
                    ("window-resize-ne", ResizeEdge::NorthEast),
                    ("window-resize-nw", ResizeEdge::NorthWest),
                    ("window-resize-se", ResizeEdge::SouthEast),
                    ("window-resize-sw", ResizeEdge::SouthWest),
                ].into_iter().map(|(class, edge)| view! {
                    column(class = format!("window-resize {class}"), a11y:hidden = true,
                        on:pointer_down = resize.resize_drag_handler(edge))
                }).collect::<Vec<_>>()}
            }
        }
    })
}
