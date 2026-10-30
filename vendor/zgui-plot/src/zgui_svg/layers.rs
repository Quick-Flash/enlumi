//! Consumer-owned canvases and labels included in retained plot exports.
use std::rc::Rc;
use zgui::{elements::canvas::CanvasHandle, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SnapshotLayer {
    Background,
    Foreground,
    Overlay,
}
type DescriptionLayer = Rc<dyn Fn(&mut crate::PlotDescription)>;

#[derive(Clone, Copy)]
pub struct SnapshotLayers {
    next: StoredValue<u64, LocalStorage>,
    canvases: StoredValue<Vec<(u64, SnapshotLayer, CanvasHandle)>, LocalStorage>,
    descriptions: StoredValue<Vec<(u64, SnapshotLayer, DescriptionLayer)>, LocalStorage>,
}
impl SnapshotLayers {
    /// Reuse a plot owner's scope, or establish one for a standalone DataPlot.
    pub fn current_or_new() -> Self {
        use_local_context::<Self>().unwrap_or_else(|| {
            let layers = Self {
                next: StoredValue::new_local(0),
                canvases: StoredValue::new_local(Vec::new()),
                descriptions: StoredValue::new_local(Vec::new()),
            };
            provide_local_context(layers);
            layers
        })
    }
    fn id(self) -> u64 {
        self.next.update_value(|next| *next += 1);
        self.next.get_value()
    }
    pub(crate) fn scenes(
        self,
        layer: SnapshotLayer,
    ) -> Result<Vec<zgui::canvas::CanvasScene>, String> {
        self.canvases.with_value(|canvases| {
            super::snapshot_canvas_handles(
                &canvases
                    .iter()
                    .filter(|(_, kind, _)| *kind == layer)
                    .map(|(_, _, handle)| handle)
                    .collect::<Vec<_>>(),
            )
        })
    }
    pub(crate) fn describe(self, description: &mut crate::PlotDescription, overlay_only: bool) {
        self.descriptions.with_value(|layers| {
            for (_, kind, draw) in layers {
                if (*kind == SnapshotLayer::Overlay) == overlay_only {
                    draw(description);
                }
            }
        });
    }
}

/// Register a canvas in its plot scope. Removal follows component lifetime.
pub fn register_canvas(handle: &CanvasHandle, layer: SnapshotLayer) {
    let Some(scope) = use_local_context::<SnapshotLayers>() else {
        return;
    };
    let id = scope.id();
    scope
        .canvases
        .update_value(|layers| layers.push((id, layer, handle.clone())));
    on_cleanup_local(move || {
        scope
            .canvases
            .try_update_value(|layers| layers.retain(|(key, _, _)| *key != id));
    });
}

/// Add consumer titles, band labels, measurement boxes, or other vector chrome.
pub fn register_description(
    layer: SnapshotLayer,
    draw: impl Fn(&mut crate::PlotDescription) + 'static,
) {
    let Some(scope) = use_local_context::<SnapshotLayers>() else {
        return;
    };
    let id = scope.id();
    scope
        .descriptions
        .update_value(|layers| layers.push((id, layer, Rc::new(draw))));
    on_cleanup_local(move || {
        scope
            .descriptions
            .try_update_value(|layers| layers.retain(|(key, _, _)| *key != id));
    });
}
