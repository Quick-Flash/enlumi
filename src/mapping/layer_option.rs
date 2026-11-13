use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn LayerOption(
    class: &'static str,
    label: &'static str,
    checked: Signal<bool, LocalStorage>,
    on_change: UnsyncCallback<bool>,
) -> impl IntoView {
    let control = NodeRef::new();
    let name = NodeRef::new();
    view! {row(class=class) {
        Checkbox(node_ref=control,labelled_by=name,checked=Binding::controlled(Signal::derive_local(move ||if checked.get(){Checked::Yes}else{Checked::No}),move |value|on_change.run(value==Checked::Yes)))
        Label(node_ref=name,control=control,on:click=move |_|on_change.run(!checked.get_untracked())) {{label}}
    }}
}
