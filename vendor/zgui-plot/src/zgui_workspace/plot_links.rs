use super::links::LinkControlsProps;
use crate::zgui_link::LinkOptions;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PlotLinksMenu(options: RwSignal<LinkOptions, LocalStorage>) -> impl IntoView {
    install_stylesheet("plot-link-controls", include_str!("links.css"));
    let open = RwSignal::new_local(false);
    view! {
        DropdownMenu(open = open) {
            DropdownMenuTrigger(size = ButtonSize::Xs, variant = ButtonVariant::Outline, class = "plot-links-trigger", class:links-open = move || open.get(),
                class:links-active = move || { let links = options.get(); links.x || links.y || links.pointer }, a11y:label = "Linked plots") {
                text(class = "plot-links-label") {{move || {
                    let links = options.get();
                    if links.x && links.y && links.pointer { return "Links: all".into(); }
                    let selected = [(links.x, "X"), (links.y, "Y"), (links.pointer, "pointer")].into_iter().filter_map(|(enabled, label)| enabled.then_some(label)).collect::<Vec<_>>();
                    format!("Links: {}", if selected.is_empty() { "off".into() } else { selected.join(", ") })
                }}}
                vector(class = "plot-links-chevron", prop:viewBox = "0 0 12 12", prop:d = "M3 4 L6 7 L9 4", a11y:hidden = true)
            }
            DropdownMenuContent(class = "plot-links-surface") {
                LinkControls(options = options)
            }
        }
    }
}
