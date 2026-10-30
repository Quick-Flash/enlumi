use super::DockState;
use crate::workspace::layout::{DropZone, Layout, Rect};
use zgui::prelude::*;

#[derive(Clone, Debug)]
pub struct GroupGuide {
    pub id: u64,
    pub members: Vec<u64>,
    pub rect: Rect,
    pub center: (f64, f64),
    pub count: usize,
}
impl GroupGuide {
    pub fn button(&self, zone: DropZone) -> Rect {
        let (dx, dy) = match zone {
            DropZone::Left => (-26.0, 0.0),
            DropZone::Right => (26.0, 0.0),
            DropZone::Top => (0.0, -26.0),
            _ => (0.0, 26.0),
        };
        Rect {
            x: self.center.0 + dx - 11.0,
            y: self.center.1 + dy - 11.0,
            width: 22.0,
            height: 22.0,
        }
    }
    pub fn hit(&self, x: f64, y: f64) -> Option<DropZone> {
        [
            DropZone::Left,
            DropZone::Right,
            DropZone::Top,
            DropZone::Bottom,
        ]
        .into_iter()
        .find(|&zone| {
            let r = self.button(zone);
            x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
        })
    }
    pub fn preview(&self, zone: DropZone) -> Rect {
        let mut r = self.rect;
        match zone {
            DropZone::Left => r.width /= 2.0,
            DropZone::Right => {
                r.x += r.width / 2.0;
                r.width /= 2.0;
            }
            DropZone::Top => r.height /= 2.0,
            _ => {
                r.y += r.height / 2.0;
                r.height /= 2.0;
            }
        }
        r
    }
}

impl DockState {
    pub fn group_guides(self) -> Vec<GroupGuide> {
        let layout = self.layout.get_untracked();
        let rects: Vec<_> = self
            .panes
            .get_untracked()
            .iter()
            .filter(|pane| layout.is_visible(pane.id))
            .filter_map(|pane| crate::zgui_svg::node_rect(pane.node).map(|rect| (pane.id, rect)))
            .collect();
        let mut guides = vec![];
        collect(&layout, &rects, &mut guides);
        if let Some(drag) = self.drag.get_untracked() {
            guides.retain(|guide| guide.members.iter().any(|id| *id != drag.0));
        }
        guides.sort_by(|a, b| {
            (a.rect.width * a.rect.height).total_cmp(&(b.rect.width * b.rect.height))
        });
        // Different scopes can share a center (three rows and middle row). Separate crosses.
        let mut placed: Vec<(f64, f64)> = vec![];
        for guide in &mut guides {
            let original = guide.center;
            'pick: for dy in [0.0, 80.0, -80.0, 160.0, -160.0] {
                for dx in [0.0, 80.0, -80.0, 160.0, -160.0] {
                    let candidate = (original.0 + dx, original.1 + dy);
                    if candidate.0 < guide.rect.x + 37.0
                        || candidate.0 > guide.rect.x + guide.rect.width - 37.0
                        || candidate.1 < guide.rect.y + 37.0
                        || candidate.1 > guide.rect.y + guide.rect.height - 37.0
                    {
                        continue;
                    }
                    if placed.iter().all(|p| {
                        (p.0 - candidate.0).abs() >= 78.0 || (p.1 - candidate.1).abs() >= 78.0
                    }) {
                        guide.center = candidate;
                        break 'pick;
                    }
                }
            }
            placed.push(guide.center);
        }
        guides
    }
    pub fn viewport_size(self) -> (f64, f64) {
        self.group_guides()
            .iter()
            .max_by(|a, b| {
                (a.rect.width * a.rect.height).total_cmp(&(b.rect.width * b.rect.height))
            })
            .map_or((0.0, 0.0), |g| (g.rect.width, g.rect.height))
    }
}

#[derive(Clone)]
struct Region {
    rect: Rect,
    members: Vec<u64>,
    count: usize,
}
fn region(layout: &Layout, panes: &[(u64, Rect)]) -> Option<Region> {
    match layout {
        Layout::Empty => None,
        Layout::Plot(id) => panes
            .iter()
            .find(|(pane, _)| pane == id)
            .map(|(_, r)| Region {
                rect: *r,
                members: vec![*id],
                count: 1,
            }),
        Layout::Tabs { active, plots } => {
            panes.iter().find(|(pane, _)| pane == active).map(|(_, r)| {
                let mut members = plots.clone();
                members.sort_unstable();
                Region {
                    rect: *r,
                    members,
                    count: 1,
                }
            })
        }
        Layout::Split { first, second, .. } => {
            Some(join(region(first, panes)?, region(second, panes)?))
        }
    }
}
fn join(a: Region, b: Region) -> Region {
    let rect = Rect {
        x: a.rect.x.min(b.rect.x),
        y: a.rect.y.min(b.rect.y),
        width: (a.rect.x + a.rect.width).max(b.rect.x + b.rect.width) - a.rect.x.min(b.rect.x),
        height: (a.rect.y + a.rect.height).max(b.rect.y + b.rect.height) - a.rect.y.min(b.rect.y),
    };
    let mut members = a.members;
    members.extend(b.members);
    members.sort_unstable();
    Region {
        rect,
        members,
        count: a.count + b.count,
    }
}
fn add(id: u64, region: Region, guides: &mut Vec<GroupGuide>) {
    if guides.iter().any(|g| g.members == region.members) {
        return;
    }
    let rect = region.rect;
    guides.push(GroupGuide {
        id,
        members: region.members,
        rect,
        center: (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0),
        count: region.count,
    });
}
fn bands(layout: &Layout, axis: bool, panes: &[(u64, Rect)], parts: &mut Vec<Region>) {
    if let Layout::Split {
        stacked,
        first,
        second,
        ..
    } = layout
        && *stacked == axis
    {
        bands(first, axis, panes, parts);
        bands(second, axis, panes, parts);
    } else if let Some(region) = region(layout, panes) {
        parts.push(region);
    }
}
fn collect(layout: &Layout, panes: &[(u64, Rect)], guides: &mut Vec<GroupGuide>) {
    if let Layout::Split {
        id,
        stacked,
        first,
        second,
        ..
    } = layout
    {
        collect(first, panes, guides);
        collect(second, panes, guides);
        let mut parts = vec![];
        bands(layout, *stacked, panes, &mut parts);
        for start in 0..parts.len() {
            let mut band = parts[start].clone();
            for part in &parts[start + 1..] {
                band = join(band, part.clone());
                add(*id, band.clone(), guides);
            }
        }
    } else if let Some(region) = region(layout, panes) {
        let id = region.members[0];
        add(id, region, guides);
    }
}
