use zgui::prelude::*;
use zgui_ui::prelude::*;

/// Informational text with native selection, keyboard navigation, and copy support.
#[component]
pub fn CopyableText(
    #[prop(into)] value: Signal<String, LocalStorage>,
    label: &'static str,
    #[prop(into, optional)] class: Classes,
) -> impl IntoView {
    view! {Textarea(class="copyable-text",class=class,read_only=true,label=label,
    value=Binding::controlled(value,|_|{}))}
}
