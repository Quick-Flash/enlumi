use super::DockState;
use crate::workspace::layout::DropZone;
use zgui::prelude::*;

/// Edge/center docking feedback while tab drag is active.
#[component]
pub fn DockOverlay(id: u64, dock: DockState) -> impl IntoView {
    view! {
        if move || show_overlay(id, dock) {
            column(class = "dock-drop-overlay", class:hidden = move || !dock.layout.get().is_visible(id)) {
                text(class = "dock-drop-zone", class:dock-zone-left = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Left))) { "Left" }
                text(class = "dock-drop-zone", class:dock-zone-right = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Right))) { "Right" }
                text(class = "dock-drop-zone", class:dock-zone-top = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Top))) { "Top" }
                text(class = "dock-drop-zone", class:dock-zone-bottom = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Bottom))) { "Bottom" }
                text(class = "dock-drop-zone", class:dock-zone-center = true, class:dock-zone-active = move || dock.group_target.get().is_none() && dock.target.get() == Some((id, DropZone::Center))) { "Center \u{b7} tab group" }
            }
        }

    }
}

fn show_overlay(id: u64, dock: DockState) -> bool {
    let Some(drag) = dock.drag.get().filter(|drag| drag.3) else {
        return false;
    };
    if !dock.layout.get().is_visible(id) || dock.group_target.get().is_some() {
        return false;
    }
    if dock.target.get().is_some_and(|(target, _)| target == id) {
        return true;
    }
    // Center of source tab group is a no-op drop, but must retain its guides.
    let tabs = drag.4.tab_ids(id);
    if tabs.len() < 2 || !tabs.contains(&drag.0) {
        return false;
    }
    let (x, y) = dock.pointer.get();
    dock.panes.with(|panes| {
        panes.iter().find(|pane| pane.id == id).is_some_and(|pane| {
            let Some(bounds) = pane.node.window_bounds() else {
                return false;
            };
            let scale = pane.node.scale();
            let (x, y) = (x * scale, y * scale);
            x >= bounds.origin.x.0
                && x < bounds.origin.x.0 + bounds.size.width.0
                && y >= bounds.origin.y.0 + 28.0 * scale
                && y < bounds.origin.y.0 + bounds.size.height.0
        })
    })
}
