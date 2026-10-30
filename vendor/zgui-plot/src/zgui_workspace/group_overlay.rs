use super::{DockState, GroupDockGuideProps};
use zgui::prelude::*;

#[component]
pub fn GroupDockOverlay(dock: DockState) -> impl IntoView {
    let guides: Vec<_> = dock
        .group_guides()
        .into_iter()
        .filter(|guide| guide.count > 1)
        .collect();
    let origin = guides
        .iter()
        .max_by(|a, b| (a.rect.width * a.rect.height).total_cmp(&(b.rect.width * b.rect.height)))
        .map_or((0.0, 0.0), |guide| (guide.rect.x, guide.rect.y));
    view! {
        for guide in move || guides.clone(), key=|guide|guide.members.clone() {
            GroupDockGuide(dock=dock,guide=guide,origin=origin)
        }
    }
}
