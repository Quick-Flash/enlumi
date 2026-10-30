//! Opt-in axes and pointer linking. A group can serve any number of plot surfaces.
use crate::{PlotGeometry, PlotPoint, Range, Ranges, interaction::RangeUpdate};
use zgui::prelude::*;
use zgui::reactive::{Callable, RenderEffect, UnsyncCallback};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkOptions {
    pub x: bool,
    pub y: bool,
    pub pointer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LinkedRange {
    source: u64,
    range: Range,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Bounds {
    x: Option<LinkedRange>,
    y: Option<LinkedRange>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LinkedPointer {
    source: u64,
    point: PlotPoint,
}

/// Shared state, owned above the plots that participate in this group.
#[derive(Clone, Copy)]
pub struct LinkGroup {
    bounds: RwSignal<Bounds, LocalStorage>,
    pointer: RwSignal<Option<LinkedPointer>, LocalStorage>,
}

impl Default for LinkGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkGroup {
    pub fn new() -> Self {
        Self {
            bounds: RwSignal::new_local(Bounds::default()),
            pointer: RwSignal::new_local(None),
        }
    }
}

/// A plot's identity, independent link controls, and shared group.
/// Provide as local context above `PlotSurface` to enable linking.
#[derive(Clone, Copy)]
pub struct PlotLinks {
    pub id: u64,
    pub options: RwSignal<LinkOptions, LocalStorage>,
    group: LinkGroup,
}

impl PlotLinks {
    pub fn new(id: u64, group: LinkGroup) -> Self {
        Self::with_options(id, group, RwSignal::new_local(LinkOptions::default()))
    }

    pub fn with_options(
        id: u64,
        group: LinkGroup,
        options: RwSignal<LinkOptions, LocalStorage>,
    ) -> Self {
        Self { id, group, options }
    }

    pub(crate) fn bind(
        self,
        ranges: UnsyncCallback<(), Ranges>,
        apply: UnsyncCallback<RangeUpdate>,
    ) {
        let tracker = StoredValue::new_local(std::cell::RefCell::new(Tracker::default()));
        let effect = RenderEffect::new(move |_| {
            let local = ranges.run(());
            let options = self.options.get();
            let shared = self.group.bounds.get();
            let (target, next) = tracker.with_value(|tracker| {
                tracker
                    .borrow_mut()
                    .observe(self.id, local, options, shared)
            });
            if next != shared {
                self.group.bounds.set(next);
            }
            let update = RangeUpdate {
                x: (target.x != local.x).then_some(target.x),
                y: (target.y != local.y).then_some(target.y),
            };
            if update.x.is_some() || update.y.is_some() {
                apply.run(update);
            }
            if !options.pointer {
                self.clear_pointer();
            }
        });
        on_cleanup_local(move || {
            drop(effect);
            self.clear_pointer();
        });
    }

    pub(crate) fn publish_pointer(self, point: Option<PlotPoint>) {
        if !self.options.get_untracked().pointer {
            return;
        }
        if let Some(point) = point.filter(|point| point.x.is_finite() && point.y.is_finite()) {
            let next = Some(LinkedPointer {
                source: self.id,
                point,
            });
            if self.group.pointer.get_untracked() != next {
                self.group.pointer.set(next);
            }
        } else {
            self.clear_pointer();
        }
    }

    fn clear_pointer(self) {
        if self
            .group
            .pointer
            .try_get_untracked()
            .flatten()
            .is_some_and(|pointer| pointer.source == self.id)
        {
            self.group.pointer.set(None);
        }
    }

    /// Current pointer from another participating plot, in data coordinates.
    pub fn pointer(self) -> Option<PlotPoint> {
        self.options
            .get()
            .pointer
            .then(|| self.group.pointer.get())
            .flatten()
            .filter(|pointer| pointer.source != self.id)
            .map(|pointer| pointer.point)
    }
}

#[derive(Default)]
struct Tracker {
    previous: Option<(Ranges, LinkOptions, Bounds)>,
}

impl Tracker {
    fn observe(
        &mut self,
        id: u64,
        local: Ranges,
        options: LinkOptions,
        shared: Bounds,
    ) -> (Ranges, Bounds) {
        let (old, enabled, previous) =
            self.previous
                .unwrap_or((local, LinkOptions::default(), shared));
        let (x, shared_x) = reconcile_axis(
            id, options.x, enabled.x, local.x, old.x, shared.x, previous.x,
        );
        let (y, shared_y) = reconcile_axis(
            id, options.y, enabled.y, local.y, old.y, shared.y, previous.y,
        );
        let target = Ranges { x, y };
        let next = Bounds {
            x: shared_x,
            y: shared_y,
        };
        self.previous = Some((target, options, next));
        (target, next)
    }
}

fn reconcile_axis(
    id: u64,
    enabled: bool,
    was_enabled: bool,
    local: Range,
    old: Range,
    shared: Option<LinkedRange>,
    previous: Option<LinkedRange>,
) -> (Range, Option<LinkedRange>) {
    if !enabled || crate::range::valid(local).is_none() {
        return (local, shared);
    }
    if !was_enabled || (shared != previous && shared.is_some_and(|range| range.source != id)) {
        return shared.map_or_else(
            || {
                (
                    local,
                    Some(LinkedRange {
                        source: id,
                        range: local,
                    }),
                )
            },
            |range| (range.range, shared),
        );
    }
    if local != old && shared.is_none_or(|range| range.range != local) {
        (
            local,
            Some(LinkedRange {
                source: id,
                range: local,
            }),
        )
    } else {
        (local, shared)
    }
}

/// Crosshair in data coordinates; draw each axis only when within its visible bounds.
pub fn linked_pointer_path(
    point: PlotPoint,
    ranges: Ranges,
    geometry: PlotGeometry,
) -> kurbo::BezPath {
    let mut path = kurbo::BezPath::new();
    if ranges.x.contains(point.x) {
        let x = geometry.x(point.x, ranges.x);
        crate::append_line(&mut path, (x, geometry.top), (x, geometry.bottom));
    }
    if ranges.y.contains(point.y) {
        let y = geometry.y(point.y, ranges.y);
        crate::append_line(&mut path, (geometry.left, y), (geometry.right, y));
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joining_pan_reset_and_unlink_keep_axes_independent_without_echo() {
        let mut first = Tracker::default();
        let mut second = Tracker::default();
        let options = LinkOptions {
            x: true,
            ..Default::default()
        };
        let original = Ranges::new((0.0, 10.0), (0.0, 5.0));
        let (_, shared) = first.observe(1, original, options, Bounds::default());
        let local = Ranges::new((20.0, 30.0), (100.0, 200.0));
        let (joined, shared) = second.observe(2, local, options, shared);
        assert_eq!(
            joined,
            Ranges {
                x: original.x,
                y: local.y
            }
        );
        let moved = Ranges {
            x: Range::new(2.0, 6.0),
            ..joined
        };
        let (_, changed) = second.observe(2, moved, options, shared);
        let (received, echoed) = first.observe(1, original, options, changed);
        assert_eq!(received.x, moved.x);
        assert_eq!(received.y, original.y);
        assert_eq!(echoed, changed);
        assert_eq!(
            first.observe(1, received, options, changed),
            (received, changed)
        );
        let (_, reset) = first.observe(1, original, options, changed);
        assert_eq!(second.observe(2, moved, options, reset).0.x, original.x);
        let independent = Ranges::new((-10.0, -5.0), (2.0, 3.0));
        assert_eq!(
            second.observe(2, independent, LinkOptions::default(), reset),
            (independent, reset)
        );
    }

    #[test]
    fn y_only_link_and_pointer_geometry_support_different_plot_sizes() {
        let mut first = Tracker::default();
        let options = LinkOptions {
            y: true,
            ..Default::default()
        };
        let local = Ranges::new((0.0, 10.0), (5.0, 15.0));
        let (_, shared) = first.observe(1, local, options, Bounds::default());
        let mut second = Tracker::default();
        let other = Ranges::new((100.0, 200.0), (-5.0, 5.0));
        assert_eq!(
            second.observe(2, other, options, shared).0,
            Ranges {
                x: other.x,
                y: local.y
            }
        );
        let geometry = PlotGeometry::with_bounds(10.0, 20.0, 110.0, 220.0, 120.0, 240.0).unwrap();
        let path = linked_pointer_path(PlotPoint { x: 5.0, y: 10.0 }, local, geometry);
        assert_eq!(path.elements().len(), 4);
        assert_eq!(
            linked_pointer_path(PlotPoint { x: 5.0, y: 30.0 }, local, geometry)
                .elements()
                .len(),
            2
        );
    }
}
