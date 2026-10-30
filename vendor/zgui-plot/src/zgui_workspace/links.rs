use crate::zgui_link::LinkOptions;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn LinkControls(options: RwSignal<LinkOptions, LocalStorage>) -> impl IntoView {
    install_stylesheet("plot-link-controls", include_str!("links.css"));
    view! {
        column(class = "link-controls") {
            LinkCheck(options = options, axis = 3, label = "All")
            LinkCheck(options = options, axis = 0, label = "Link X bounds")
            LinkCheck(options = options, axis = 1, label = "Link Y bounds")
            LinkCheck(options = options, axis = 2, label = "Link pointer")
        }
    }
}

#[component]
fn LinkCheck(
    options: RwSignal<LinkOptions, LocalStorage>,
    axis: usize,
    label: &'static str,
) -> impl IntoView {
    let checked = Binding::controlled(
        Signal::derive_local(move || {
            let options = options.get();
            match axis {
                0 => options.x,
                1 => options.y,
                2 => options.pointer,
                _ => options.x && options.y && options.pointer,
            }
        }),
        move |value: bool| {
            options.update(|options| {
                if axis == 3 {
                    options.x = value;
                    options.y = value;
                    options.pointer = value;
                    return;
                }
                let slot = match axis {
                    0 => &mut options.x,
                    1 => &mut options.y,
                    _ => &mut options.pointer,
                };
                *slot = value;
            })
        },
    );
    view! {
        MenuCheckboxItem(checked = checked, close_on_select = false, class = "link-option") {{label}}
    }
}
