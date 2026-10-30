//! Docking tree, stable divider IDs, tab groups, and pane geometry.
mod region;

pub const MIN_PLOT_SIZE: (f64, f64) = (220.0, 160.0);

#[derive(Clone, Debug, PartialEq)]
pub enum Layout {
    Empty,
    Plot(u64),
    Tabs {
        plots: Vec<u64>,
        active: u64,
    },
    Split {
        id: u64,
        stacked: bool,
        ratio: f64,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropZone {
    Center,
    TabBefore,
    TabAfter,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Divider {
    pub id: u64,
    pub stacked: bool,
    pub ratio: f64,
    pub rect: Rect,
    pub span: f64,
}

impl Layout {
    pub fn split(&mut self, target: u64, new: u64, stacked: bool) -> bool {
        self.split_position(target, new, stacked, false)
    }

    fn split_position(&mut self, target: u64, new: u64, stacked: bool, before: bool) -> bool {
        // Moving a plot can leave its old divider alive, so plot IDs cannot identify splits.
        let split_id = new.max(
            self.max_split_id()
                .checked_add(1)
                .expect("split IDs exhausted"),
        );
        self.split_position_with_id(target, new, stacked, before, split_id)
    }

    fn max_split_id(&self) -> u64 {
        match self {
            Self::Split {
                id, first, second, ..
            } => (*id).max(first.max_split_id()).max(second.max_split_id()),
            _ => 0,
        }
    }

    fn split_position_with_id(
        &mut self,
        target: u64,
        new: u64,
        stacked: bool,
        before: bool,
        split_id: u64,
    ) -> bool {
        match self {
            Self::Empty => {
                *self = Self::Plot(new);
                true
            }
            Self::Plot(id) if *id == target => {
                *self = split_pair(Self::Plot(target), new, stacked, before, split_id);
                true
            }
            Self::Tabs { plots, .. } if plots.contains(&target) => {
                let current = std::mem::replace(self, Self::Empty);
                *self = split_pair(current, new, stacked, before, split_id);
                true
            }
            Self::Split { first, second, .. } => {
                first.split_position_with_id(target, new, stacked, before, split_id)
                    || second.split_position_with_id(target, new, stacked, before, split_id)
            }
            _ => false,
        }
    }

    pub fn remove(&mut self, target: u64) {
        match self {
            Self::Plot(id) if *id == target => *self = Self::Empty,
            Self::Tabs { plots, active } if plots.contains(&target) => {
                plots.retain(|id| *id != target);
                match plots.len() {
                    0 => *self = Self::Empty,
                    1 => *self = Self::Plot(plots[0]),
                    _ if *active == target => *active = plots[0],
                    _ => {}
                }
            }
            Self::Split { first, second, .. } => {
                first.remove(target);
                second.remove(target);
                if **first == Self::Empty {
                    *self = std::mem::replace(&mut **second, Self::Empty);
                } else if **second == Self::Empty {
                    *self = std::mem::replace(&mut **first, Self::Empty);
                }
            }
            _ => {}
        }
    }

    pub fn swap(&mut self, a: u64, b: u64) {
        match self {
            Self::Plot(id) => {
                if *id == a {
                    *id = b;
                } else if *id == b {
                    *id = a;
                }
            }
            Self::Tabs { plots, active } => {
                for id in plots.iter_mut() {
                    if *id == a {
                        *id = b;
                    } else if *id == b {
                        *id = a;
                    }
                }
                if *active == a {
                    *active = b;
                } else if *active == b {
                    *active = a;
                }
            }
            Self::Split { first, second, .. } => {
                first.swap(a, b);
                second.swap(a, b);
            }
            Self::Empty => {}
        }
    }

    pub fn activate(&mut self, target: u64) -> bool {
        match self {
            Self::Plot(id) => *id == target,
            Self::Tabs { plots, active } if plots.contains(&target) => {
                *active = target;
                true
            }
            Self::Split { first, second, .. } => first.activate(target) || second.activate(target),
            _ => false,
        }
    }

    pub fn is_visible(&self, target: u64) -> bool {
        match self {
            Self::Plot(id) => *id == target,
            Self::Tabs { plots, active } => plots.contains(&target) && *active == target,
            Self::Split { first, second, .. } => {
                first.is_visible(target) || second.is_visible(target)
            }
            Self::Empty => false,
        }
    }

    pub fn tab_ids(&self, target: u64) -> Vec<u64> {
        match self {
            Self::Plot(id) if *id == target => vec![target],
            Self::Tabs { plots, .. } if plots.contains(&target) => plots.clone(),
            Self::Split { first, second, .. } => {
                let first_tabs = first.tab_ids(target);
                if first_tabs.is_empty() {
                    second.tab_ids(target)
                } else {
                    first_tabs
                }
            }
            _ => Vec::new(),
        }
    }

    pub fn dock(&mut self, source: u64, target: u64) -> bool {
        let original_ids = self.plot_ids();
        let mut updated = self.clone();
        if !updated.dock_inner(source, target) || updated.plot_ids() != original_ids {
            return false;
        }
        *self = updated;
        true
    }

    fn dock_inner(&mut self, source: u64, target: u64) -> bool {
        if !self.contains(source) || !self.contains(target) {
            return false;
        }
        if source == target {
            let Some(survivor) = self.tab_ids(source).into_iter().find(|id| *id != source) else {
                return false;
            };
            self.remove(source);
            return self.add_tab(survivor, source);
        }
        self.remove(source);
        self.add_tab(target, source)
    }

    pub fn dock_split(&mut self, source: u64, target: u64, zone: DropZone) -> bool {
        let original_ids = self.plot_ids();
        let mut updated = self.clone();
        if !updated.dock_split_inner(source, target, zone) || updated.plot_ids() != original_ids {
            return false;
        }
        *self = updated;
        true
    }

    /// Move existing pane to an outer edge, retaining whole remaining tree.
    pub fn dock_workspace(&mut self, source: u64, zone: DropZone) -> bool {
        self.dock_group(source, None, zone)
    }

    /// Split any subtree, including nested groups in middle of workspace.
    pub fn dock_group(&mut self, source: u64, group: Option<u64>, zone: DropZone) -> bool {
        let (stacked, before) = match zone {
            DropZone::Left => (false, true),
            DropZone::Right => (false, false),
            DropZone::Top => (true, true),
            DropZone::Bottom => (true, false),
            _ => return false,
        };
        let Some(target) = group.map_or(Some(&*self), |id| self.split_group(id)) else {
            return false;
        };
        let mut members = target.plot_ids();
        members.retain(|id| *id != source);
        if !self.contains(source) || members.is_empty() {
            return false;
        }
        let split_id = self
            .max_split_id()
            .checked_add(1)
            .expect("split IDs exhausted");
        let mut updated = self.clone();
        updated.remove(source);
        if !updated.wrap_group(&members, source, stacked, before, split_id) {
            return false;
        }
        *self = updated;
        true
    }

    fn split_group(&self, target: u64) -> Option<&Self> {
        if let Self::Split {
            id, first, second, ..
        } = self
        {
            if *id == target {
                Some(self)
            } else {
                first
                    .split_group(target)
                    .or_else(|| second.split_group(target))
            }
        } else {
            None
        }
    }

    fn wrap_group(
        &mut self,
        members: &[u64],
        source: u64,
        stacked: bool,
        before: bool,
        split_id: u64,
    ) -> bool {
        // Source removal can collapse target split; remaining membership still identifies subtree.
        if self.plot_ids() == members {
            let current = std::mem::replace(self, Self::Empty);
            *self = split_pair(current, source, stacked, before, split_id);
            return true;
        }
        if let Self::Split { first, second, .. } = self {
            first.wrap_group(members, source, stacked, before, split_id)
                || second.wrap_group(members, source, stacked, before, split_id)
        } else {
            false
        }
    }

    fn dock_split_inner(&mut self, source: u64, target: u64, zone: DropZone) -> bool {
        if !self.contains(source) || !self.contains(target) {
            return false;
        }
        let (stacked, before) = match zone {
            DropZone::Left => (false, true),
            DropZone::Right => (false, false),
            DropZone::Top => (true, true),
            DropZone::Bottom => (true, false),
            DropZone::Center | DropZone::TabBefore | DropZone::TabAfter => return false,
        };
        let target = if source == target {
            let Some(survivor) = self.tab_ids(source).into_iter().find(|id| *id != source) else {
                return false;
            };
            survivor
        } else {
            target
        };
        self.remove(source);
        self.split_position(target, source, stacked, before)
    }

    pub fn dock_tab(&mut self, source: u64, target: u64, after: bool) -> bool {
        if source == target || !self.contains(source) || !self.contains(target) {
            return false;
        }
        let original_ids = self.plot_ids();
        let mut updated = self.clone();
        updated.remove(source);
        if !updated.add_tab(target, source)
            || !updated.reorder_tab(source, target, after)
            || updated.plot_ids() != original_ids
        {
            return false;
        }
        *self = updated;
        true
    }

    pub fn add_plot_tab(&mut self, target: u64, new: u64) -> bool {
        if self.contains(new) || !self.contains(target) {
            return false;
        }
        let original_ids = self.plot_ids();
        let mut updated = self.clone();
        if !updated.add_tab(target, new) {
            return false;
        }
        let mut expected_ids = original_ids;
        expected_ids.push(new);
        expected_ids.sort_unstable();
        if updated.plot_ids() != expected_ids {
            return false;
        }
        *self = updated;
        true
    }

    fn reorder_tab(&mut self, source: u64, target: u64, after: bool) -> bool {
        match self {
            Self::Tabs { plots, active } if plots.contains(&source) && plots.contains(&target) => {
                let source_index = plots.iter().position(|id| *id == source).unwrap();
                plots.remove(source_index);
                let target_index = plots.iter().position(|id| *id == target).unwrap();
                plots.insert(target_index + usize::from(after), source);
                *active = source;
                true
            }
            Self::Split { first, second, .. } => {
                first.reorder_tab(source, target, after)
                    || second.reorder_tab(source, target, after)
            }
            _ => false,
        }
    }

    fn plot_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        self.collect_plot_ids(&mut ids);
        ids.sort_unstable();
        ids
    }

    fn collect_plot_ids(&self, ids: &mut Vec<u64>) {
        match self {
            Self::Empty => {}
            Self::Plot(id) => ids.push(*id),
            Self::Tabs { plots, .. } => ids.extend(plots.iter().copied()),
            Self::Split { first, second, .. } => {
                first.collect_plot_ids(ids);
                second.collect_plot_ids(ids);
            }
        }
    }

    fn contains(&self, target: u64) -> bool {
        match self {
            Self::Plot(id) => *id == target,
            Self::Tabs { plots, .. } => plots.contains(&target),
            Self::Split { first, second, .. } => first.contains(target) || second.contains(target),
            Self::Empty => false,
        }
    }

    fn add_tab(&mut self, target: u64, source: u64) -> bool {
        match self {
            Self::Plot(id) if *id == target => {
                *self = Self::Tabs {
                    plots: vec![target, source],
                    active: source,
                };
                true
            }
            Self::Tabs { plots, active } if plots.contains(&target) => {
                if let Some(index) = plots.iter().position(|id| *id == source) {
                    plots.remove(index);
                }
                plots.push(source);
                *active = source;
                true
            }
            Self::Split { first, second, .. } => {
                first.add_tab(target, source) || second.add_tab(target, source)
            }
            _ => false,
        }
    }

    pub fn resize(&mut self, target: u64, value: f64) {
        if !value.is_finite() {
            return;
        }
        if let Self::Split {
            id,
            ratio,
            first,
            second,
            ..
        } = self
        {
            if *id == target {
                *ratio = value.clamp(0.15, 0.85);
            } else {
                first.resize(target, value);
                second.resize(target, value);
            }
        }
    }

    pub fn flat(ids: &[(u64, f64)], stacked: bool) -> Self {
        match ids {
            [] => Self::Empty,
            [(id, _)] => Self::Plot(*id),
            [(id, weight), rest @ ..] => Self::Split {
                id: rest[0].0,
                stacked,
                ratio: weight / ids.iter().map(|(_, w)| w).sum::<f64>(),
                first: Box::new(Self::Plot(*id)),
                second: Box::new(Self::flat(rest, stacked)),
            },
        }
    }

    pub fn minimum(&self) -> (f64, f64) {
        match self {
            Self::Empty => (0.0, 0.0),
            Self::Plot(_) => MIN_PLOT_SIZE,
            Self::Tabs { .. } => MIN_PLOT_SIZE,
            Self::Split {
                stacked,
                first,
                second,
                ..
            } => {
                let (aw, ah) = first.minimum();
                let (bw, bh) = second.minimum();
                if *stacked {
                    (aw.max(bw), ah + bh + 6.0)
                } else {
                    (aw + bw + 6.0, ah.max(bh))
                }
            }
        }
    }

    pub fn geometry(&self, rect: Rect, plots: &mut Vec<(u64, Rect)>, dividers: &mut Vec<Divider>) {
        match self {
            Self::Empty => {}
            Self::Plot(id) => plots.push((*id, rect)),
            Self::Tabs { plots: ids, .. } => plots.extend(ids.iter().map(|id| (*id, rect))),
            Self::Split {
                id,
                stacked,
                ratio,
                first,
                second,
            } => {
                let span = (if *stacked { rect.height } else { rect.width } - 6.0).max(0.0);
                let (aw, ah) = first.minimum();
                let (bw, bh) = second.minimum();
                let (minimum, maximum) = if *stacked {
                    (ah, span - bh)
                } else {
                    (aw, span - bw)
                };
                let size = (span * ratio).clamp(minimum.min(span), maximum.max(minimum).min(span));
                let (a, grip, b) = if *stacked {
                    (
                        Rect {
                            height: size,
                            ..rect
                        },
                        Rect {
                            y: rect.y + size,
                            height: 6.0,
                            ..rect
                        },
                        Rect {
                            y: rect.y + size + 6.0,
                            height: span - size,
                            ..rect
                        },
                    )
                } else {
                    (
                        Rect {
                            width: size,
                            ..rect
                        },
                        Rect {
                            x: rect.x + size,
                            width: 6.0,
                            ..rect
                        },
                        Rect {
                            x: rect.x + size + 6.0,
                            width: span - size,
                            ..rect
                        },
                    )
                };
                dividers.push(Divider {
                    id: *id,
                    stacked: *stacked,
                    ratio: size / span.max(1.0),
                    rect: grip,
                    span,
                });
                first.geometry(a, plots, dividers);
                second.geometry(b, plots, dividers);
            }
        }
    }
}

fn split_pair(target: Layout, new: u64, stacked: bool, before: bool, split_id: u64) -> Layout {
    let new_plot = Layout::Plot(new);
    let (first, second) = if before {
        (new_plot, target)
    } else {
        (target, new_plot)
    };
    Layout::Split {
        id: split_id,
        stacked,
        ratio: 0.5,
        first: Box::new(first),
        second: Box::new(second),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_tab_keeps_split_divider_keys_unique() {
        let mut tree = Layout::Plot(1);
        assert!(tree.split(1, 2, false));
        assert!(tree.add_plot_tab(2, 3));
        assert!(tree.split(1, 4, false));
        assert!(tree.split(4, 5, true));
        assert!(tree.split(5, 6, false));
        assert!(tree.dock_split(2, 6, DropZone::Right));
        assert_eq!(tree.plot_ids(), vec![1, 2, 3, 4, 5, 6]);
        let (width, height) = tree.minimum();
        let mut plots = vec![];
        let mut dividers = vec![];
        tree.geometry(
            Rect {
                width,
                height,
                ..Rect::default()
            },
            &mut plots,
            &mut dividers,
        );
        assert_eq!(dividers.len(), 5);
        let keys = dividers
            .iter()
            .map(|divider| divider.id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(keys.len(), dividers.len());
    }

    #[test]
    fn tab_moves_work_in_both_directions_and_survive_edge_docking() {
        let mut tree = Layout::Plot(1);
        assert!(tree.add_plot_tab(1, 2));
        assert!(tree.add_plot_tab(1, 3));
        assert!(tree.dock_tab(1, 3, true));
        assert_eq!(tree.tab_ids(1), vec![2, 3, 1]);
        assert!(tree.dock_tab(1, 2, false));
        assert_eq!(tree.tab_ids(1), vec![1, 2, 3]);
        assert!(tree.dock_split(3, 1, DropZone::Top));
        assert_eq!(tree.tab_ids(1), vec![1, 2]);
        assert_eq!(tree.plot_ids(), vec![1, 2, 3]);
        assert!(tree.dock_tab(3, 2, true));
        assert_eq!(tree.tab_ids(1), vec![1, 2, 3]);
        assert_eq!(tree.plot_ids(), vec![1, 2, 3]);
    }

    #[test]
    fn mixed_splits_partition_canvas_without_overlap_or_clipped_leaves() {
        let mut tree = Layout::Plot(1);
        tree.split(1, 2, false);
        tree.split(2, 3, true);
        tree.split(3, 4, false);
        tree.resize(2, 0.2);
        let (width, height) = tree.minimum();
        let mut plots = vec![];
        let mut dividers = vec![];
        tree.geometry(
            Rect {
                width,
                height,
                ..Rect::default()
            },
            &mut plots,
            &mut dividers,
        );
        for (index, (_, a)) in plots.iter().enumerate() {
            assert!(a.width >= 220.0 - 1e-9 && a.height >= 160.0 - 1e-9);
            for (_, b) in plots.iter().skip(index + 1) {
                assert!(
                    a.x + a.width <= b.x
                        || b.x + b.width <= a.x
                        || a.y + a.height <= b.y
                        || b.y + b.height <= a.y
                );
            }
        }
        let area = plots.iter().map(|(_, r)| r.width * r.height).sum::<f64>()
            + dividers
                .iter()
                .map(|d| d.rect.width * d.rect.height)
                .sum::<f64>();
        assert!((area - width * height).abs() < 1e-6);
    }
    #[test]
    fn nested_split_close_and_swap_preserve_tree() {
        let mut tree = Layout::Plot(1);
        assert!(tree.split(1, 2, false));
        assert!(tree.split(2, 3, true));
        assert!(!tree.split(99, 4, false));
        let mut plots = vec![];
        let mut dividers = vec![];
        tree.geometry(
            Rect {
                width: 1000.0,
                height: 800.0,
                ..Rect::default()
            },
            &mut plots,
            &mut dividers,
        );
        assert_eq!(plots.len(), 3);
        assert_eq!(dividers.len(), 2);
        assert_eq!(plots[0].1.height, 800.0);
        assert_eq!(plots[1].1.x, plots[2].1.x);
        assert_eq!(plots[2].1.y, plots[1].1.height + 6.0);
        tree.swap(1, 3);
        tree.remove(2);
        tree.remove(1);
        assert_eq!(tree, Layout::Plot(3));
        tree.remove(3);
        assert_eq!(tree, Layout::Empty);
    }
    #[test]
    fn resize_clamps_and_minimum_preserves_leaf_space() {
        let mut tree = Layout::flat(&[(1, 1.0), (2, 1.0), (3, 1.0)], false);
        tree.resize(2, 0.99);
        tree.resize(3, f64::NAN);
        let (width, height) = tree.minimum();
        let mut plots = vec![];
        let mut dividers = vec![];
        tree.geometry(
            Rect {
                width,
                height,
                ..Rect::default()
            },
            &mut plots,
            &mut dividers,
        );
        assert!(
            plots
                .iter()
                .all(|(_, r)| r.width >= 220.0 - 1e-9 && r.height >= 160.0)
        );
        assert!(matches!(tree, Layout::Split { ratio: 0.85, .. }));
    }
}
