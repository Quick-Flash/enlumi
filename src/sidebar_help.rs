use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn SidebarHelp(label: &'static str, children: ChildrenFn) -> impl IntoView {
    view! {
        Tooltip {
            TooltipTrigger { Button(class = "sidebar-help", size = ButtonSize::IconXs,
                variant = ButtonVariant::Ghost, a11y:label = label) {
                vector(class = "sidebar-info-icon", prop:viewBox = "0 0 20 20",
                    prop:d = "M10 1 A9 9 0 1 1 10 19 A9 9 0 1 1 10 1 M10 9 V15 M10 5 V6",
                    a11y:hidden = true)
            }
            }
            TooltipContent(class = "sidebar-help-content") {
                {children.view()}
            }
        }
    }
}
