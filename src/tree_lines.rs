use zgui::prelude::*;

/// Shared connector geometry for raw fields and saved profile resources.
#[component]
pub fn TreeLines(
    #[prop(into)] depth: Signal<usize, LocalStorage>,
    #[prop(into)] last: Signal<bool, LocalStorage>,
    #[prop(into, default = Signal::stored_local(false))] open: Signal<bool, LocalStorage>,
    #[prop(into, default = Signal::stored_local(Vec::new()))] guides: Signal<
        Vec<usize>,
        LocalStorage,
    >,
) -> impl IntoView {
    view! {box(class="tree-connectors",a11y:hidden=true) {
        for guide in move ||guides.get(),key=|guide|*guide {
            box(class="tree-guide",style:left=Some(format!("{}px",guide*12+10)))
        }
        if move ||depth.get()>0 {
            box(class="tree-guide",class:tree-guide-last=move ||last.get(),style:left=move ||Some(format!("{}px",depth.get().saturating_sub(1)*12+10)))
            box(class="tree-elbow",style:left=move ||Some(format!("{}px",depth.get().saturating_sub(1)*12+10)))
        }
        if move ||open.get() {
            box(class="tree-stem",style:left=move ||Some(format!("{}px",depth.get()*12+10)))
        }
    }}
}

#[component]
pub fn TreeBranch(
    open: RwSignal<bool, LocalStorage>,
    name: String,
    #[prop(into)] count: Signal<usize, LocalStorage>,
    #[prop(default = 1)] depth: usize,
    #[prop(default = false)] last: bool,
    #[prop(default = true)] connectors: bool,
    #[prop(optional)] class: &'static str,
) -> impl IntoView {
    let label = format!("Toggle saved {name}");
    view! {row(class="tree-row profile-group-row",style:padding-left=Some(format!("{}px",depth*12))) {
        if move ||connectors {TreeLines(depth=depth,last=last,open=Signal::derive_local(move ||open.get()))}
        control(class="tree-branch",class=class,class:profile-output-section=depth>0,class:tree-open=move ||open.get(),tabindex=Focus::Sequential,
            {..Attrs::new().a11y_from(A11yBinding::new(Role::Button).label(label).expanded(move ||open.get()))},
            on:click=move |_|open.update(|open|*open = !*open),
            on:key_down=move |event| {
                use zgui::vocab::{Key,NamedKey};
                match event.key {
                    Key::Named(NamedKey::ArrowRight) =>open.set(true),
                    Key::Named(NamedKey::ArrowLeft) =>open.set(false),
                    Key::Named(NamedKey::Enter|NamedKey::Space) =>open.update(|open|*open = !*open),
                    _=>return,
                }
                event.prevent_default();event.stop_propagation();
            }) {
            vector(class="tree-arrow",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if open.get(){"M2 4 L6 8 L10 4"}else{"M4 2 L8 6 L4 10"}),a11y:hidden=true)
            text(class="tree-name") {{name}}
            text(class="tree-count") {{move ||count.get().to_string()}}
        }
    }}
}
