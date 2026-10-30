use super::DockState;
use crate::workspace::layout::Rect;
use zgui::prelude::*;

#[component]
pub fn DockPreviewPane(id: u64, panes: Memo<Vec<(u64, Rect)>>, dock: DockState) -> impl IntoView {
    let rect = move || {
        panes
            .get()
            .into_iter()
            .find(|(pane, _)| *pane == id)
            .map_or(Rect::default(), |(_, rect)| rect)
    };
    let source = move || dock.drag.get().is_some_and(|drag| drag.0 == id);
    view! {
        column(class=format!("dock-preview-pane dock-preview-pane-{id}"),class:dock-preview-source=source,
            style:left=move ||Some(format!("{}px",rect().x)),style:top=move ||Some(format!("{}px",rect().y)),
            style:width=move ||Some(format!("{}px",rect().width)),style:height=move ||Some(format!("{}px",rect().height))) {
            text(class="dock-preview-label") {{move ||if source(){format!("Drop Plot {id}")}else{format!("Plot {id}")}}}
        }
    }
}
