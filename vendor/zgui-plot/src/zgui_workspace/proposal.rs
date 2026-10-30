use super::DockState;
use crate::workspace::layout::{DropZone, Layout};
use zgui::prelude::*;

impl DockState {
    /// Preview and drop share same transaction, always starting from drag snapshot.
    pub fn proposal(self, source: u64) -> Option<Layout> {
        let mut tree = self.drag.get_untracked()?.4;
        let applied = if let Some((members, zone)) = self.group_target.get_untracked() {
            tree.dock_region(source, &members, zone, self.viewport_size())
        } else {
            let (target, zone) = self.target.get_untracked()?;
            match zone {
                DropZone::Center => tree.dock(source, target),
                DropZone::TabBefore => tree.dock_tab(source, target, false),
                DropZone::TabAfter => tree.dock_tab(source, target, true),
                zone => tree.dock_split(source, target, zone),
            }
        };
        applied.then_some(tree)
    }
}
