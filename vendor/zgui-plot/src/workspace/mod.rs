//! Reusable plot configuration and docking interactions. No application data dependencies.
#[path = "layout/equalize.rs"]
mod equalize;
pub mod layout;
mod resize;
use layout::{DropZone, Layout, Rect};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlotKind {
    Trend,
    Histogram,
    Scatter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlotConfig {
    pub kind: PlotKind,
    pub x: String,
    pub y: String,
}

impl Default for PlotConfig {
    fn default() -> Self {
        Self {
            kind: PlotKind::Trend,
            x: String::new(),
            y: String::new(),
        }
    }
}

impl PlotConfig {
    pub fn assign(&mut self, metric: &str, x_axis: bool) {
        if x_axis {
            self.x = metric.to_owned();
        } else {
            self.y = metric.to_owned();
        }
    }
}

#[derive(Clone)]
pub struct TabStripSnapshot {
    pub pane: u64,
    pub rect: Rect,
    pub tabs: Vec<(u64, Rect)>,
}

pub fn tab_offsets(strips: &[TabStripSnapshot], tree: &Layout) -> HashMap<(u64, u64), f64> {
    let mut offsets = HashMap::new();
    for strip in strips {
        let Some((_, first)) = strip.tabs.first() else {
            continue;
        };
        let gap = strip.tabs.windows(2).next().map_or(1.0, |tabs| {
            (tabs[1].1.x - tabs[0].1.x - tabs[0].1.width).max(0.0)
        });
        let mut left = first.x;
        for id in tree.tab_ids(strip.pane) {
            if let Some((_, old)) = strip.tabs.iter().find(|(tab, _)| *tab == id) {
                offsets.insert((strip.pane, id), left - old.x);
            }
            let width = strips
                .iter()
                .flat_map(|strip| &strip.tabs)
                .find(|(tab, _)| *tab == id)
                .map_or(0.0, |(_, rect)| rect.width);
            left += width + gap;
        }
    }
    offsets
}

pub fn tab_strip_target(
    strips: &[TabStripSnapshot],
    x: f64,
    y: f64,
    source: u64,
) -> Option<(u64, DropZone)> {
    strips.iter().find_map(|strip| {
        let rect = strip.rect;
        if x < rect.x || x > rect.x + rect.width || y < rect.y || y > rect.y + rect.height {
            return None;
        }
        let mut last = None;
        for (id, bounds) in &strip.tabs {
            if *id == source {
                continue;
            }
            if x < bounds.x + bounds.width * 0.5 {
                return Some((*id, DropZone::TabBefore));
            }
            last = Some((*id, DropZone::TabAfter));
        }
        last
    })
}

pub fn drop_axis(kind: PlotKind, x: f64, y: f64, width: f64, height: f64) -> Option<bool> {
    (width > 0.0 && height > 0.0 && x >= 0.0 && y >= 0.0 && x < width && y < height)
        .then_some(kind == PlotKind::Scatter && x < width / 2.0)
}

pub fn drag_badge_position(x: f64, y: f64, width: f64, height: f64) -> (f64, f64) {
    let left = if x + 18.0 + 280.0 > width {
        x - 298.0
    } else {
        x + 18.0
    };
    let top = if y + 18.0 + 82.0 > height {
        y - 100.0
    } else {
        y + 18.0
    };
    (
        left.clamp(0.0, (width - 280.0).max(0.0)),
        top.clamp(0.0, (height - 82.0).max(0.0)),
    )
}

pub fn plot_rect(plots: &[(u64, Rect)], id: u64) -> Rect {
    plots
        .iter()
        .find(|(key, _)| *key == id)
        .map_or(Rect::default(), |(_, rect)| *rect)
}

/// Visibility with optional maximized pane.
pub fn plot_visible(id: u64, maximized: Option<u64>, layout: Layout) -> bool {
    maximized.map_or_else(|| layout.is_visible(id), |target| target == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measured_tab_targets_support_both_directions_and_unequal_widths() {
        let strips = vec![TabStripSnapshot {
            pane: 1,
            rect: Rect {
                x: 100.0,
                y: 20.0,
                width: 240.0,
                height: 28.0,
            },
            tabs: vec![
                (
                    1,
                    Rect {
                        x: 100.0,
                        y: 20.0,
                        width: 40.0,
                        height: 26.0,
                    },
                ),
                (
                    2,
                    Rect {
                        x: 141.0,
                        y: 20.0,
                        width: 90.0,
                        height: 26.0,
                    },
                ),
                (
                    3,
                    Rect {
                        x: 232.0,
                        y: 20.0,
                        width: 60.0,
                        height: 26.0,
                    },
                ),
            ],
        }];
        assert_eq!(
            tab_strip_target(&strips, 310.0, 30.0, 1),
            Some((3, DropZone::TabAfter))
        );
        assert_eq!(
            tab_strip_target(&strips, 105.0, 30.0, 3),
            Some((1, DropZone::TabBefore))
        );
        assert_eq!(
            tab_strip_target(&strips, 180.0, 30.0, 1),
            Some((2, DropZone::TabBefore))
        );
        assert_eq!(
            tab_strip_target(&strips, 190.0, 30.0, 1),
            Some((3, DropZone::TabBefore))
        );
        assert_eq!(tab_strip_target(&strips, 190.0, 70.0, 1), None);
        assert_eq!(tab_strip_target(&strips, 190.0, 48.5, 1), None);
        assert_eq!(tab_strip_target(&strips, 190.0, 19.5, 1), None);
        let mut tree = Layout::Plot(1);
        tree.add_plot_tab(1, 2);
        tree.add_plot_tab(1, 3);
        assert!(
            tab_offsets(&strips, &tree)
                .values()
                .all(|offset| *offset == 0.0)
        );
        tree.dock_tab(1, 3, true);
        let offsets = tab_offsets(&strips, &tree);
        assert_eq!(offsets[&(1, 1)], 152.0);
        assert_eq!(offsets[&(1, 2)], -41.0);
        assert_eq!(offsets[&(1, 3)], -41.0);
    }
    #[test]
    fn drag_badge_follows_pointer_and_flips_inside_edges() {
        assert_eq!(
            drag_badge_position(100.0, 100.0, 1000.0, 800.0),
            (118.0, 118.0)
        );
        assert_eq!(
            drag_badge_position(950.0, 750.0, 1000.0, 800.0),
            (652.0, 650.0)
        );
        assert_eq!(drag_badge_position(50.0, 20.0, 200.0, 60.0), (0.0, 0.0));
        assert_eq!(drag_badge_position(-50.0, -50.0, 1000.0, 800.0), (0.0, 0.0));
    }
    #[test]
    fn drop_feedback_and_assignment_share_axis_boundaries() {
        assert_eq!(
            drop_axis(PlotKind::Scatter, 49.0, 20.0, 100.0, 80.0),
            Some(true)
        );
        assert_eq!(
            drop_axis(PlotKind::Scatter, 50.0, 20.0, 100.0, 80.0),
            Some(false)
        );
        for kind in [PlotKind::Trend, PlotKind::Histogram] {
            assert_eq!(drop_axis(kind, 1.0, 20.0, 100.0, 80.0), Some(false));
        }
        for (x, y, w, h) in [
            (-1.0, 0.0, 100.0, 80.0),
            (100.0, 0.0, 100.0, 80.0),
            (0.0, 80.0, 100.0, 80.0),
            (0.0, 0.0, 0.0, 0.0),
        ] {
            assert_eq!(drop_axis(PlotKind::Scatter, x, y, w, h), None);
        }
    }
    #[test]
    fn maximize_exposes_only_target_and_restore_exposes_all() {
        let layout = Layout::flat(&[(1, 1.0), (2, 1.0), (3, 1.0)], false);
        for id in 1..=3 {
            assert_eq!(plot_visible(id, Some(2), layout.clone()), id == 2);
            assert!(plot_visible(id, None, layout.clone()));
        }
    }
    #[test]
    fn assignments_preserve_kind_and_other_axis() {
        let mut config = PlotConfig {
            kind: PlotKind::Scatter,
            x: "old-x".into(),
            y: "old-y".into(),
        };
        config.assign("new-y", false);
        assert_eq!(config.x, "old-x");
        assert_eq!(config.y, "new-y");
        config.assign("new-x", true);
        assert_eq!(config.y, "new-y");
        assert_eq!(config.kind, PlotKind::Scatter);
    }
}
