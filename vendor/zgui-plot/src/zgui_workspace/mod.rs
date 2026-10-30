//! Optional workspace UI: animated docking tabs, split resize, settings, and geometry.
//! Install `stylesheet()` alongside plot surface styles and a zgui-ui theme.
mod canvas;
mod divider;
mod fit_menu;
mod group;
mod group_guide;
mod group_overlay;
mod layout_preview;
mod links;
mod menu;
mod overlay;
mod pane;
mod plot_links;
mod preview_pane;
mod proposal;
mod screenshot;
mod series_source;
mod settings;
pub use screenshot::*;
mod tabs;
mod time_series_settings;
mod workspace_overlay;
use crate::workspace::{
    TabStripSnapshot,
    layout::{DropZone, Layout},
    tab_strip_target,
};
pub use canvas::*;
pub use divider::*;
pub use fit_menu::*;
pub use group::*;
pub use group_guide::*;
pub use group_overlay::*;
pub use layout_preview::*;
pub use links::*;
pub use menu::*;
pub use overlay::*;
pub use pane::*;
pub use plot_links::*;
pub use preview_pane::*;
pub use series_source::*;
pub use settings::*;
use std::{collections::HashMap, rc::Rc, time::Instant};
pub use tabs::*;
pub use time_series_settings::*;
pub use workspace_overlay::*;
use zgui::prelude::*;
use zgui::view::FrameHandle;

pub fn stylesheet() -> &'static str {
    include_str!("workspace.css")
}

/// UI geometry and overflow state for one dockable pane.
#[derive(Clone, Copy)]
pub struct DockPane {
    pub id: u64,
    pub node: NodeRef,
    pub tabs_navigation: NodeRef,
    pub tabs_node: NodeRef,
    pub tabs_track: NodeRef,
    pub tab_offset: RwSignal<f64, LocalStorage>,
    pub tab_limit: Signal<f64, LocalStorage>,
    pub tabs_overflow: Signal<bool, LocalStorage>,
    pub tabs_width: Signal<f64, LocalStorage>,
}
impl DockPane {
    pub fn new(id: u64) -> Self {
        let (
            tabs_navigation,
            tabs_node,
            tabs_track,
            tab_offset,
            tab_limit,
            tabs_overflow,
            tabs_width,
        ) = tab_navigation();
        Self {
            id,
            node: NodeRef::new(),
            tabs_navigation,
            tabs_node,
            tabs_track,
            tab_offset,
            tab_limit,
            tabs_overflow,
            tabs_width,
        }
    }
}
type TabNavigation = (
    NodeRef,
    NodeRef,
    NodeRef,
    RwSignal<f64, LocalStorage>,
    Signal<f64, LocalStorage>,
    Signal<bool, LocalStorage>,
    Signal<f64, LocalStorage>,
);

fn tab_navigation() -> TabNavigation {
    let navigation = NodeRef::new();
    let navigation_bounds = navigation.observe_border_box();
    let viewport = NodeRef::new();
    let track = NodeRef::new();
    let viewport_bounds = viewport.observe_border_box();
    let track_bounds = track.observe_border_box();
    let limit = Signal::derive_local(move || {
        let viewport_width = viewport_bounds.get().map_or(0.0, |bounds| {
            f64::from(bounds.size.width.0 / viewport.scale())
        });
        let track_width = track_bounds
            .get()
            .map_or(0.0, |bounds| f64::from(bounds.size.width.0 / track.scale()));
        (track_width - viewport_width).max(0.0)
    });
    // Compare with full strip width so hiding arrows cannot trigger overflow again.
    let overflow = Signal::derive_local(move || {
        let Some(bounds) = navigation_bounds.get() else {
            return false;
        };
        let available = f64::from(bounds.size.width.0 / navigation.scale()) - 25.0;
        let content = track_bounds
            .get()
            .map_or(0.0, |bounds| f64::from(bounds.size.width.0 / track.scale()));
        content > available + 0.5
    });
    let width = Signal::derive_local(move || {
        let available = navigation_bounds.get().map_or(0.0, |bounds| {
            f64::from(bounds.size.width.0 / navigation.scale())
        });
        let content = track_bounds
            .get()
            .map_or(0.0, |bounds| f64::from(bounds.size.width.0 / track.scale()));
        content.min((available - 25.0 - if overflow.get() { 50.0 } else { 0.0 }).max(0.0))
    });
    (
        navigation,
        viewport,
        track,
        RwSignal::new_local(0.0),
        limit,
        overflow,
        width,
    )
}

/// Dragged tab ID, initial pointer X/Y, threshold passed, and original layout.
pub type TabDrag = (u64, f32, f32, bool, Layout);
/// Animated offsets keyed by pane ID and tab ID.
pub type TabOffsets = HashMap<(u64, u64), f64>;

/// Shared tab drag lifetime, preview, and measured drop targets.
#[derive(Clone, Copy)]
pub struct DockState {
    pub layout: RwSignal<Layout, LocalStorage>,
    pub panes: Signal<Vec<DockPane>, LocalStorage>,
    pub drag: RwSignal<Option<TabDrag>, LocalStorage>,
    pub target: RwSignal<Option<(u64, DropZone)>, LocalStorage>,
    pub group_target: RwSignal<Option<(Vec<u64>, DropZone)>, LocalStorage>,
    pub preview: RwSignal<Option<(u64, Layout)>, LocalStorage>,
    pub nodes: StoredValue<HashMap<(u64, u64), NodeRef>, LocalStorage>,
    pub strips: StoredValue<Vec<TabStripSnapshot>, LocalStorage>,
    pub pointer: RwSignal<(f32, f32), LocalStorage>,
    pub settling: RwSignal<Option<TabOffsets>, LocalStorage>,
    pub rebase: RwSignal<bool, LocalStorage>,
    pub settle_frame: StoredValue<Option<FrameHandle>, LocalStorage>,
    pub settle_done: StoredValue<Option<Rc<dyn Fn()>>, LocalStorage>,
    pub suppress_click: RwSignal<Option<u64>, LocalStorage>,
}
impl DockState {
    pub fn new(
        layout: RwSignal<Layout, LocalStorage>,
        panes: Signal<Vec<DockPane>, LocalStorage>,
    ) -> Self {
        let tab_drag = RwSignal::new_local(None::<(u64, f32, f32, bool, Layout)>);
        let tab_drop_target = RwSignal::new_local(None::<(u64, DropZone)>);
        let tab_preview = RwSignal::new_local(None::<(u64, Layout)>);
        let tab_nodes = StoredValue::new_local(HashMap::<(u64, u64), NodeRef>::new());
        let tab_strips = StoredValue::new_local(Vec::<TabStripSnapshot>::new());
        let tab_pointer = RwSignal::new_local((0.0_f32, 0.0_f32));
        let tab_settling = RwSignal::new_local(None::<HashMap<(u64, u64), f64>>);
        let tab_rebase = RwSignal::new_local(false);
        let tab_settle_frame = StoredValue::new_local(None::<FrameHandle>);
        let tab_settle_done = StoredValue::new_local(None::<Rc<dyn Fn()>>);
        let suppress_tab_click = RwSignal::new_local(None::<u64>);
        zgui::reactive::on_cleanup_local(move || {
            tab_settle_frame.set_value(None);
            tab_settle_done.set_value(None);
        });
        Self {
            layout,
            panes,
            drag: tab_drag,
            target: tab_drop_target,
            group_target: RwSignal::new_local(None),
            preview: tab_preview,
            nodes: tab_nodes,
            strips: tab_strips,
            pointer: tab_pointer,
            settling: tab_settling,
            rebase: tab_rebase,
            settle_frame: tab_settle_frame,
            settle_done: tab_settle_done,
            suppress_click: suppress_tab_click,
        }
    }
    pub fn hit_target(self, x: f32, y: f32, source: u64) -> Option<(u64, DropZone)> {
        self.group_target.set(None);
        if let Some((group, zone)) = self
            .group_guides()
            .iter()
            .filter(|guide| guide.count > 1)
            .filter_map(|guide| {
                guide
                    .hit(f64::from(x), f64::from(y))
                    .map(|zone| (guide, zone))
            })
            .min_by(|(a, _), (b, _)| {
                (a.rect.width * a.rect.height).total_cmp(&(b.rect.width * b.rect.height))
            })
        {
            self.group_target.set(Some((group.members.clone(), zone)));
            return Some((source, zone));
        }
        let tab_strips = self.strips;
        let tab_drag = self.drag;
        let layout = self.layout;
        let tab_target = tab_strips
            .with_value(|strips| tab_strip_target(strips, f64::from(x), f64::from(y), source));
        if tab_target.is_some() {
            return tab_target;
        }
        let plots = self.panes.get_untracked();
        let hit_layout = tab_drag
            .get_untracked()
            .map(|drag| drag.4)
            .unwrap_or_else(|| layout.get_untracked());
        plots.iter().find_map(|plot| {
            let tabs = hit_layout.tab_ids(plot.id);
            if !hit_layout.is_visible(plot.id) || (plot.id == source && tabs.len() == 1) {
                return None;
            }
            let bounds = plot.node.window_bounds()?;
            let scale = plot.node.scale();
            let px = f64::from(x - bounds.origin.x.0 / scale);
            let py = f64::from(y - bounds.origin.y.0 / scale);
            let width = f64::from(bounds.size.width.0 / scale);
            let height = f64::from(bounds.size.height.0 / scale);
            if width <= 0.0 || height <= 0.0 || px < 0.0 || py < 0.0 || px >= width || py >= height
            {
                return None;
            }
            if py < 28.0 {
                return None;
            }
            let py = py - 28.0;
            let height = height - 28.0;
            let edges = [
                (px / width, DropZone::Left),
                ((width - px) / width, DropZone::Right),
                (py / height, DropZone::Top),
                ((height - py) / height, DropZone::Bottom),
            ];
            let zone = edges
                .into_iter()
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .filter(|(distance, _)| *distance < 0.22)
                .map_or(DropZone::Center, |(_, zone)| zone);
            if zone == DropZone::Center && tabs.contains(&source) {
                return None;
            }
            Some((plot.id, zone))
        })
    }
}
fn finish_tab_after(
    deadline: Instant,
    frame: StoredValue<Option<FrameHandle>, LocalStorage>,
    done: Rc<dyn Fn()>,
) {
    if Instant::now() >= deadline {
        frame.set_value(None);
        done();
    } else {
        frame.set_value(Some(request_frame(move |_| {
            finish_tab_after(deadline, frame, done)
        })));
    }
}

mod data_plot_settings;
mod plot_bounds_settings;
mod series_control;
mod series_controls;
pub use data_plot_settings::*;
pub use plot_bounds_settings::*;
pub use series_control::*;
pub use series_controls::*;
