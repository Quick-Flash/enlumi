//! Asking a settled window which elements carry a class, and where they are.
//!
//! A person points at a region and sees where it is. A headless test has no pointer, so it names
//! the region the way the sheet does — by class — and reads the box the layout gave it. This is
//! that seam, and it keeps an application's own tests off the document arena.

use zgui_geom::{Device, DevicePx, Rect};
use zgui_view::NodeId;

use crate::window::Window;

impl Window {
    /// Every element that carries `class`, in document order.
    #[must_use]
    pub fn nodes_with_class(&self, class: &str) -> Vec<NodeId> {
        let document = self.document.borrow();
        let store = document.store();
        let Some(root) = document.root_index() else {
            return Vec::new();
        };

        let mut found = Vec::new();
        let mut stack = vec![root];
        while let Some(index) = stack.pop() {
            let carries = store
                .classes_of(index)
                .iter()
                .any(|held| held.0.as_ref() == class);
            if carries {
                found.push(zgui_view_dom::id::to_view(store.key_of(index)));
            }
            let mut children = Vec::new();
            let mut child = store.core(index).first_child();
            while let Some(at) = child {
                children.push(at);
                child = store.core(at).next_sibling();
            }
            stack.extend(children.into_iter().rev());
        }
        found
    }

    /// Where the first element that carries `class` sits on the surface.
    ///
    /// Absolute device pixels, against the frame that was last drawn. `None` when nothing carries
    /// the class or the element has no box.
    #[must_use]
    pub fn box_with_class(&self, class: &str) -> Option<Rect<DevicePx, Device>> {
        let node = self.nodes_with_class(class).first().copied()?;
        zgui_view::ViewHost::window_box(self.host.as_ref(), node)
    }
}
