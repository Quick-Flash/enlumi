//! Plot snapshot registration and capture of visible panes.
use crate::workspace::layout::{Layout, Rect};
use std::rc::Rc;
use zgui::prelude::*;

pub struct PlotSnapshot {
    pub svg: super::SvgPair,
    pub node: NodeRef,
    pub width: f64,
    pub height: f64,
}

type PlotSnapshotFn = Rc<dyn Fn() -> Result<PlotSnapshot, String>>;
pub type PlotSnapshotSource = StoredValue<Option<PlotSnapshotFn>, LocalStorage>;

pub fn register_snapshot(
    source: PlotSnapshotSource,
    snapshot: impl Fn() -> Result<PlotSnapshot, String> + 'static,
) {
    let callback: PlotSnapshotFn = Rc::new(snapshot);
    source.set_value(Some(Rc::clone(&callback)));
    on_cleanup_local(move || {
        source.try_update_value(|source| {
            if source
                .as_ref()
                .is_some_and(|current| Rc::ptr_eq(current, &callback))
            {
                *source = None;
            }
        });
    });
}

/// Pane identity, fallback bounds, and live snapshot callback.
#[derive(Clone, Copy)]
pub struct WorkspaceSnapshot {
    pub id: u64,
    pub node: NodeRef,
    pub snapshot: PlotSnapshotSource,
}

/// Window bounds in CSS pixels.
pub fn node_rect(node: NodeRef) -> Option<Rect> {
    let bounds = node.window_bounds()?;
    let scale = f64::from(node.scale());
    Some(Rect {
        x: f64::from(bounds.origin.x.0) / scale,
        y: f64::from(bounds.origin.y.0) / scale,
        width: f64::from(bounds.size.width.0) / scale,
        height: f64::from(bounds.size.height.0) / scale,
    })
}

pub fn capture_workspace(
    slots: &[WorkspaceSnapshot],
    maximized: Option<u64>,
    layout: &Layout,
    background: &str,
) -> Result<String, String> {
    capture_workspace_with_overlay(slots, maximized, layout, background, false)
}

pub fn capture_workspace_with_overlay(
    slots: &[WorkspaceSnapshot],
    maximized: Option<u64>,
    layout: &Layout,
    background: &str,
    include_overlay: bool,
) -> Result<String, String> {
    let mut snapshots = Vec::new();
    for slot in slots
        .iter()
        .filter(|slot| maximized.map_or_else(|| layout.is_visible(slot.id), |id| slot.id == id))
    {
        let (rect, svg) = if let Some(snapshot) = slot.snapshot.get_value() {
            let snapshot = snapshot().map_err(|error| format!("Plot {}: {error}", slot.id))?;
            let bounds = node_rect(snapshot.node)
                .ok_or_else(|| format!("Plot {} is not ready to export", slot.id))?;
            (
                Rect {
                    width: snapshot.width,
                    height: snapshot.height,
                    ..bounds
                },
                if include_overlay {
                    snapshot.svg.with_overlay
                } else {
                    snapshot.svg.clean
                },
            )
        } else {
            let bounds = node_rect(slot.node)
                .ok_or_else(|| format!("Plot {} is not ready to export", slot.id))?;
            let mut description =
                crate::PlotDescription::new(bounds.width, bounds.height, background);
            description.push_title(format!("Plot {} — no rendered data", slot.id));
            (bounds, crate::zgui_svg::scenes_to_svg(&[], &description)?)
        };
        snapshots.push((rect, svg));
    }
    if snapshots.is_empty() {
        return Err("workspace has no visible plots".into());
    }
    let left = snapshots
        .iter()
        .map(|(rect, _)| rect.x)
        .fold(f64::INFINITY, f64::min);
    let top = snapshots
        .iter()
        .map(|(rect, _)| rect.y)
        .fold(f64::INFINITY, f64::min);
    let right = snapshots
        .iter()
        .map(|(rect, _)| rect.x + rect.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = snapshots
        .iter()
        .map(|(rect, _)| rect.y + rect.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let panels = snapshots
        .into_iter()
        .map(|(rect, svg)| crate::zgui_svg::SvgPanel {
            x: rect.x - left,
            y: rect.y - top,
            svg,
        })
        .collect::<Vec<_>>();
    crate::zgui_svg::compose_workspace_svg(right - left, bottom - top, background, &panels)
}
