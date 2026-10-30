//! Pointer interaction in screen pixels: what a press grabs, and how dragging,
//! box selection and the wheel change the visible ranges.
//!
//! Positions are pixels in the plot's own coordinates (the same space as its
//! [`PlotGeometry`]), so hit zones stay a constant size on screen.

use std::time::{Duration, Instant};

use crate::range::{self, Edge};
use crate::{PlotGeometry, Range, Ranges};

/// What a press at a point manipulates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// Drag the visible ranges.
    Pan,
    /// Draw a zoom box.
    Select,
    /// Move one end of the X range.
    XEdge(Edge),
    /// Move one end of the Y range.
    YEdge(Edge),
}

impl Gesture {
    /// CSS `cursor` value hinting at this gesture.
    pub fn cursor(gesture: Option<Self>) -> &'static str {
        match gesture {
            Some(Self::XEdge(_)) => "ew-resize",
            Some(Self::YEdge(_)) => "ns-resize",
            Some(Self::Select) => "zoom-in",
            Some(Self::Pan) | None => "crosshair",
        }
    }
}

/// Which axes pan and zoom move without grabbing an axis handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axes {
    pub x: bool,
    pub y: bool,
}

impl Axes {
    /// Both axes pan and zoom freely.
    pub const BOTH: Self = Self { x: true, y: true };
    /// For plots whose Y range is fixed by the data, such as histogram counts.
    pub const X_ONLY: Self = Self { x: true, y: false };
}

/// Sizes of the grab handles around the data area, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitZones {
    /// Distance from the left/right data edges that grabs an X edge.
    pub x_edge: f64,
    /// Band around the bottom edge holding the X edge handles: (above, below).
    pub bottom_band: (f64, f64),
    /// Width of the Y-axis band inside the left data edge.
    pub y_axis_width: f64,
    /// Whether bottom band between X handles grabs Y minimum instead of X bound.
    pub y_min_handle: bool,
}

impl Default for HitZones {
    fn default() -> Self {
        Self {
            x_edge: 38.0,
            bottom_band: (16.0, 18.0),
            y_axis_width: 56.0,
            y_min_handle: true,
        }
    }
}

/// Gesture a press at `(x, y)` starts, or `None` outside every zone. With
/// `select` (for example Shift held) a press inside the data area box-selects.
pub fn hit_test(
    (x, y): (f64, f64),
    geometry: PlotGeometry,
    zones: &HitZones,
    select: bool,
) -> Option<Gesture> {
    let in_x = (geometry.left..=geometry.right).contains(&x);
    let in_data = in_x && (geometry.top..=geometry.bottom).contains(&y);
    if select {
        return in_data.then_some(Gesture::Select);
    }
    let bottom = geometry.bottom - zones.bottom_band.0..=geometry.bottom + zones.bottom_band.1;
    if bottom.contains(&y) {
        if (x - geometry.left).abs() <= zones.x_edge {
            return Some(Gesture::XEdge(Edge::Min));
        }
        if (x - geometry.right).abs() <= zones.x_edge {
            return Some(Gesture::XEdge(Edge::Max));
        }
        if in_x {
            return Some(if zones.y_min_handle {
                Gesture::YEdge(Edge::Min)
            } else {
                Gesture::XEdge(if x < (geometry.left + geometry.right) / 2.0 {
                    Edge::Min
                } else {
                    Edge::Max
                })
            });
        }
    }
    if in_data && x - geometry.left <= zones.y_axis_width {
        return Some(Gesture::YEdge(
            if y < (geometry.top + geometry.bottom) / 2.0 {
                Edge::Max
            } else {
                Edge::Min
            },
        ));
    }
    in_data.then_some(Gesture::Pan)
}

use self::drag as drag_ranges;

/// Ranges after dragging `gesture` from `start` to `current` (pixels),
/// starting from `initial`. Selection returns `None`; see [`box_zoom`].
pub fn drag(
    gesture: Gesture,
    initial: impl Into<Ranges>,
    start: (f64, f64),
    current: (f64, f64),
    geometry: PlotGeometry,
    axes: Axes,
) -> Option<Ranges> {
    let initial = initial.into();
    let (width, height) = (geometry.data_width(), geometry.data_height());
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let dx = (current.0 - start.0) / width * initial.x.span();
    // Screen y grows downward; data y grows upward.
    let dy = (start.1 - current.1) / height * initial.y.span();
    match gesture {
        Gesture::Select => None,
        Gesture::Pan => Some(Ranges {
            x: if axes.x {
                range::pan(initial.x, -dx)?
            } else {
                initial.x
            },
            y: if axes.y {
                range::pan(initial.y, -dy)?
            } else {
                initial.y
            },
        }),
        Gesture::XEdge(edge) => Some(Ranges {
            x: range::move_edge(initial.x, dx, edge)?,
            y: initial.y,
        }),
        Gesture::YEdge(edge) => Some(Ranges {
            x: initial.x,
            y: range::move_edge(initial.y, dy, edge)?,
        }),
    }
}

/// Ranges inside a box drawn between two corners (pixels, either order),
/// clamped to the data area. Boxes thinner than `min_size` pixels on an axis
/// that zooms are ignored. Axes not in `axes` keep their initial range.
pub fn box_zoom(
    initial: impl Into<Ranges>,
    ((x0, x1), (y0, y1)): ((f64, f64), (f64, f64)),
    geometry: PlotGeometry,
    axes: Axes,
    min_size: f64,
) -> Option<Ranges> {
    let initial = initial.into();
    if (axes.x && (x1 - x0).abs() < min_size) || (axes.y && (y1 - y0).abs() < min_size) {
        return None;
    }
    let (y0, y1) = if axes.y {
        (y0, y1)
    } else {
        (geometry.top, geometry.bottom)
    };
    let selected = range::selection(initial, ((x0, x1), (y0, y1)), geometry)?;
    Some(Ranges {
        x: if axes.x { selected.x } else { initial.x },
        y: if axes.y { selected.y } else { initial.y },
    })
}

/// Ranges after a wheel step of `factor` at `pointer` over `gesture`'s zone.
/// The value under the pointer stays fixed. Axis bands zoom only that axis;
/// the data area zooms the axes enabled by `axes`.
pub fn wheel_zoom(
    initial: impl Into<Ranges>,
    pointer: (f64, f64),
    factor: f64,
    gesture: Gesture,
    geometry: PlotGeometry,
    axes: Axes,
) -> Option<Ranges> {
    let initial = initial.into();
    let x_anchor = ((pointer.0 - geometry.left) / geometry.data_width()).clamp(0.0, 1.0);
    let y_anchor = ((geometry.bottom - pointer.1) / geometry.data_height()).clamp(0.0, 1.0);
    let zoom_x = |range: Range| range::zoom(range, x_anchor, factor);
    Some(match gesture {
        Gesture::YEdge(_) => Ranges {
            x: initial.x,
            y: range::zoom(initial.y, y_anchor, factor)?,
        },
        Gesture::XEdge(_) => Ranges {
            x: zoom_x(initial.x)?,
            y: initial.y,
        },
        Gesture::Pan | Gesture::Select => Ranges {
            x: if axes.x {
                zoom_x(initial.x)?
            } else {
                initial.x
            },
            y: if axes.y {
                range::zoom(initial.y, y_anchor, factor)?
            } else {
                initial.y
            },
        },
    })
}

/// Whether a click `elapsed` after the previous one, at most 4px away from
/// it, completes a double click (within 500ms).
pub fn is_double_click(elapsed: Duration, previous: (f64, f64), current: (f64, f64)) -> bool {
    elapsed <= Duration::from_millis(500)
        && (current.0 - previous.0).hypot(current.1 - previous.1) <= 4.0
}

/// Axis ranges an interaction changed; `None` leaves that axis alone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RangeUpdate {
    pub x: Option<Range>,
    pub y: Option<Range>,
}

impl RangeUpdate {
    /// The axes `gesture` moves: one for an edge handle, otherwise those in `axes`.
    fn of(gesture: Gesture, ranges: Ranges, axes: Axes) -> Self {
        match gesture {
            Gesture::XEdge(_) => Self {
                x: Some(ranges.x),
                y: None,
            },
            Gesture::YEdge(_) => Self {
                x: None,
                y: Some(ranges.y),
            },
            Gesture::Pan | Gesture::Select => Self {
                x: axes.x.then_some(ranges.x),
                y: axes.y.then_some(ranges.y),
            },
        }
    }

    /// `ranges` with the changed axes replaced.
    pub fn apply(self, ranges: impl Into<Ranges>) -> Ranges {
        let ranges = ranges.into();
        Ranges {
            x: self.x.unwrap_or(ranges.x),
            y: self.y.unwrap_or(ranges.y),
        }
    }
}

/// What releasing the pointer completed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Release {
    /// Nothing was pressed.
    Idle,
    /// A press and release without movement.
    Click,
    /// A click close in time and space to the previous one.
    DoubleClick,
    /// A box selection; zoom to these ranges.
    Zoom(RangeUpdate),
    /// A drag that already applied its updates while moving.
    Dragged,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    gesture: Gesture,
    start: (f64, f64),
    current: (f64, f64),
    initial: Ranges,
    moved: bool,
}

/// Pointer state machine for one plot: press, drag, release, wheel.
///
/// Positions are plot pixels. Callers apply the returned [`RangeUpdate`]s to
/// their own range state, so the controller works with any storage.
#[derive(Clone, Debug)]
pub struct Controller {
    /// Grab handles around the data area.
    pub zones: HitZones,
    /// Axes that pan, box zoom and wheel zoom move outside the handles.
    pub axes: Axes,
    /// Box selections thinner than this, in pixels, do not zoom.
    pub min_box: f64,
    /// Optional width/height ratio of the selection rectangle, in pixels.
    /// Only applies when both axes can zoom; the same box is shown and applied.
    pub selection_aspect: Option<f64>,
    drag: Option<Drag>,
    last_click: Option<(Instant, (f64, f64))>,
}

impl Controller {
    /// Idle controller with a 4px minimum zoom box.
    pub fn new(zones: HitZones, axes: Axes) -> Self {
        Self {
            zones,
            axes,
            min_box: 4.0,
            selection_aspect: None,
            drag: None,
            last_click: None,
        }
    }

    /// Gesture of the drag in progress.
    pub fn active(&self) -> Option<Gesture> {
        self.drag.map(|drag| drag.gesture)
    }

    /// Gesture a press at `pointer` would start.
    pub fn hint(
        &self,
        pointer: (f64, f64),
        geometry: PlotGeometry,
        select: bool,
    ) -> Option<Gesture> {
        hit_test(pointer, geometry, &self.zones, select)
    }

    /// Start a drag at `pointer` from the visible `ranges`; returns what it grabbed.
    pub fn press(
        &mut self,
        pointer: (f64, f64),
        geometry: PlotGeometry,
        ranges: impl Into<Ranges>,
        select: bool,
    ) -> Option<Gesture> {
        let gesture = self.hint(pointer, geometry, select)?;
        self.drag = Some(Drag {
            gesture,
            start: pointer,
            current: pointer,
            initial: ranges.into(),
            moved: false,
        });
        Some(gesture)
    }

    /// Follow the pointer during a drag. Returns the ranges to show, or `None`
    /// when not dragging, not yet moved, or drawing a selection box.
    pub fn drag_to(&mut self, pointer: (f64, f64), geometry: PlotGeometry) -> Option<RangeUpdate> {
        let drag = self.drag.as_mut()?;
        if pointer == drag.start {
            return None;
        }
        drag.moved = true;
        drag.current = pointer;
        let Drag {
            gesture,
            start,
            initial,
            ..
        } = *drag;
        self.last_click = None;
        let ranges = drag_ranges(gesture, initial, start, pointer, geometry, self.axes)?;
        Some(RangeUpdate::of(gesture, ranges, self.axes))
    }

    /// Box being selected, clamped to the data area. Without Y zoom the box
    /// spans the full data height.
    pub fn selection(&self, geometry: PlotGeometry) -> Option<((f64, f64), (f64, f64))> {
        let drag = self
            .drag
            .filter(|drag| drag.gesture == Gesture::Select && drag.moved)?;
        let x = |x: f64| x.clamp(geometry.left, geometry.right);
        let y = |y: f64| y.clamp(geometry.top, geometry.bottom);
        let rows = if self.axes.y {
            (y(drag.start.1), y(drag.current.1))
        } else {
            (geometry.top, geometry.bottom)
        };
        let start = (x(drag.start.0), y(drag.start.1));
        if self.axes.x
            && self.axes.y
            && let Some(aspect) = self
                .selection_aspect
                .filter(|value| value.is_finite() && *value > 0.)
        {
            let dx = x(drag.current.0) - start.0;
            let dy = y(drag.current.1) - start.1;
            let sx = if dx < 0. { -1. } else { 1. };
            let sy = if dy < 0. { -1. } else { 1. };
            let available_x = if sx < 0. {
                start.0 - geometry.left
            } else {
                geometry.right - start.0
            };
            let available_y = if sy < 0. {
                start.1 - geometry.top
            } else {
                geometry.bottom - start.1
            };
            let width = dx
                .abs()
                .max(dy.abs() * aspect)
                .min(available_x)
                .min(available_y * aspect);
            return Some((
                (start.0, start.0 + sx * width),
                (start.1, start.1 + sy * width / aspect),
            ));
        }
        Some(((x(drag.start.0), x(drag.current.0)), rows))
    }

    /// End the drag at `now`. A press with `select` held never counts as a click.
    pub fn release(&mut self, now: Instant, select: bool, geometry: PlotGeometry) -> Release {
        let selection = self.selection(geometry);
        let Some(drag) = self.drag.take() else {
            return Release::Idle;
        };
        if !drag.moved && !select {
            let double = self.last_click.is_some_and(|(time, position)| {
                is_double_click(now.saturating_duration_since(time), position, drag.start)
            });
            self.last_click = if double {
                None
            } else {
                Some((now, drag.start))
            };
            return if double {
                Release::DoubleClick
            } else {
                Release::Click
            };
        }
        self.last_click = None;
        match selection {
            Some(rect) => box_zoom(drag.initial, rect, geometry, self.axes, self.min_box)
                .map_or(Release::Dragged, |ranges| {
                    Release::Zoom(RangeUpdate::of(Gesture::Select, ranges, self.axes))
                }),
            None => Release::Dragged,
        }
    }

    /// Zoom by `factor` at `pointer`, ending any drag in progress.
    pub fn wheel(
        &mut self,
        pointer: (f64, f64),
        factor: f64,
        geometry: PlotGeometry,
        ranges: impl Into<Ranges>,
    ) -> Option<RangeUpdate> {
        self.drag = None;
        self.last_click = None;
        let gesture = self.hint(pointer, geometry, false)?;
        let ranges = wheel_zoom(ranges, pointer, factor, gesture, geometry, self.axes)?;
        Some(RangeUpdate::of(gesture, ranges, self.axes))
    }

    /// Abandon the drag; returns the ranges it started from.
    pub fn cancel(&mut self) -> Option<Ranges> {
        self.last_click = None;
        self.drag.take().map(|drag| drag.initial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> PlotGeometry {
        PlotGeometry::with_bounds(0.0, 4.0, 490.0, 300.0, 490.0, 316.0).unwrap()
    }

    #[test]
    fn handles_sit_on_their_edges() {
        let zones = HitZones::default();
        let hit = |x, y, select| hit_test((x, y), geometry(), &zones, select);
        assert_eq!(hit(0.0, 300.0, false), Some(Gesture::XEdge(Edge::Min)));
        assert_eq!(hit(490.0, 300.0, false), Some(Gesture::XEdge(Edge::Max)));
        assert_eq!(hit(200.0, 300.0, false), Some(Gesture::YEdge(Edge::Min)));
        assert_eq!(hit(200.0, 4.0, false), Some(Gesture::Pan));
        assert_eq!(hit(200.0, 150.0, false), Some(Gesture::Pan));
        assert_eq!(hit(0.0, 300.0, true), Some(Gesture::Select));
        assert_eq!(hit(-1.0, 150.0, false), None);
        let no_y_min = HitZones {
            y_min_handle: false,
            ..zones
        };
        assert_eq!(
            hit_test((200.0, 300.0), geometry(), &no_y_min, false),
            Some(Gesture::XEdge(Edge::Min))
        );
    }

    #[test]
    fn axis_bands_only_cover_left_and_bottom_and_shift_keeps_box_zoom() {
        let zones = HitZones::default();
        assert_eq!(
            hit_test((10.0, 110.0), geometry(), &zones, false),
            Some(Gesture::YEdge(Edge::Max))
        );
        assert_eq!(
            hit_test((480.0, 220.0), geometry(), &zones, false),
            Some(Gesture::Pan)
        );
        assert_eq!(
            hit_test((10.0, 110.0), geometry(), &zones, true),
            Some(Gesture::Select)
        );
        let disabled = HitZones {
            y_axis_width: 0.0,
            ..zones
        };
        assert_eq!(
            hit_test((10.0, 110.0), geometry(), &disabled, false),
            Some(Gesture::Pan)
        );
    }

    #[test]
    fn pan_moves_ranges_against_the_pointer_and_respects_axes() {
        let initial = ((0.0, 10.0), (0.0, 10.0));
        // 49px right is 1 unit of X; 29.6px down is 1 unit of Y.
        let panned = drag(
            Gesture::Pan,
            initial,
            (100.0, 100.0),
            (149.0, 129.6),
            geometry(),
            Axes::BOTH,
        )
        .unwrap();
        assert!((panned.x.min + 1.0).abs() < 1e-9 && (panned.y.min - 1.0).abs() < 1e-9);
        let x_only = drag(
            Gesture::Pan,
            initial,
            (100.0, 100.0),
            (149.0, 129.6),
            geometry(),
            Axes::X_ONLY,
        )
        .unwrap();
        assert_eq!(x_only.y, Range::new(0.0, 10.0));
        assert_eq!(
            drag(
                Gesture::Select,
                initial,
                (0.0, 0.0),
                (1.0, 1.0),
                geometry(),
                Axes::BOTH
            ),
            None
        );
    }

    #[test]
    fn edge_drags_move_one_end() {
        let initial = ((0.0, 10.0), (0.0, 10.0));
        let x = drag(
            Gesture::XEdge(Edge::Min),
            initial,
            (0.0, 300.0),
            (98.0, 300.0),
            geometry(),
            Axes::BOTH,
        )
        .unwrap();
        assert!((x.x.min - 2.0).abs() < 1e-9 && x.x.max == 10.0);
        // Dragging the top handle up by a quarter of the height raises Y max.
        let y = drag(
            Gesture::YEdge(Edge::Max),
            initial,
            (200.0, 4.0),
            (200.0, -70.0),
            geometry(),
            Axes::BOTH,
        )
        .unwrap();
        assert!((y.y.max - 12.5).abs() < 1e-9 && y.y.min == 0.0);
        assert_eq!(
            drag(
                Gesture::XEdge(Edge::Min),
                initial,
                (0.0, 0.0),
                (980.0, 0.0),
                geometry(),
                Axes::BOTH
            ),
            None
        );
    }

    #[test]
    fn box_zoom_maps_either_direction_and_ignores_slivers() {
        let initial = ((0.0, 10.0), (0.0, 10.0));
        let zoomed = box_zoom(
            initial,
            ((98.0, 196.0), (152.0, 4.0)),
            geometry(),
            Axes::BOTH,
            4.0,
        )
        .unwrap();
        assert!((zoomed.x.min - 2.0).abs() < 1e-9 && (zoomed.x.max - 4.0).abs() < 1e-9);
        assert!((zoomed.y.min - 5.0).abs() < 1e-9 && (zoomed.y.max - 10.0).abs() < 1e-9);
        assert_eq!(
            box_zoom(
                initial,
                ((98.0, 100.0), (4.0, 152.0)),
                geometry(),
                Axes::BOTH,
                4.0
            ),
            None
        );
        let x_only = box_zoom(
            initial,
            ((98.0, 196.0), (150.0, 150.0)),
            geometry(),
            Axes::X_ONLY,
            4.0,
        )
        .unwrap();
        assert_eq!(x_only.y, Range::new(0.0, 10.0));
    }

    #[test]
    fn wheel_keeps_the_pointer_value_fixed() {
        let initial = ((0.0, 10.0), (0.0, 10.0));
        let zoom = |pointer, gesture, axes| {
            wheel_zoom(initial, pointer, 2.0, gesture, geometry(), axes).unwrap()
        };
        let both = zoom((0.0, 300.0), Gesture::Pan, Axes::BOTH);
        assert_eq!(
            (both.x, both.y),
            (Range::new(0.0, 20.0), Range::new(0.0, 20.0))
        );
        let top = zoom((200.0, 4.0), Gesture::YEdge(Edge::Max), Axes::BOTH);
        assert_eq!(
            (top.x, top.y),
            (Range::new(0.0, 10.0), Range::new(-10.0, 10.0))
        );
        let bottom = zoom((200.0, 300.0), Gesture::YEdge(Edge::Min), Axes::BOTH);
        assert_eq!(bottom.y, Range::new(0.0, 20.0));
        let edge = zoom((490.0, 300.0), Gesture::XEdge(Edge::Max), Axes::BOTH);
        assert_eq!(
            (edge.x, edge.y),
            (Range::new(-10.0, 10.0), Range::new(0.0, 10.0))
        );
        let x_only = zoom((0.0, 150.0), Gesture::Pan, Axes::X_ONLY);
        assert_eq!(x_only.y, Range::new(0.0, 10.0));
    }

    #[test]
    fn axis_wheel_preserves_mouse_anchor_and_other_axis() {
        let geometry = geometry();
        let initial = Ranges::new((10.0, 30.0), (-20.0, 60.0));
        let mut controller = Controller::new(
            HitZones {
                y_min_handle: false,
                ..HitZones::default()
            },
            Axes::BOTH,
        );
        for (point, x_axis) in [
            ((122.5, 308.0), true),
            ((367.5, 308.0), true),
            ((10.0, 78.0), false),
            ((10.0, 226.0), false),
        ] {
            for factor in [0.5, 2.0] {
                let update = controller.wheel(point, factor, geometry, initial).unwrap();
                let zoomed = update.apply(initial);
                if x_axis {
                    assert_eq!(update.y, None);
                    assert_eq!(zoomed.y, initial.y);
                    assert!(
                        (geometry.x_value(point.0, zoomed.x)
                            - geometry.x_value(point.0, initial.x))
                        .abs()
                            < 1e-9
                    );
                    assert!((zoomed.x.span() - initial.x.span() * factor).abs() < 1e-9);
                } else {
                    assert_eq!(update.x, None);
                    assert_eq!(zoomed.x, initial.x);
                    assert!(
                        (geometry.y_value(point.1, zoomed.y)
                            - geometry.y_value(point.1, initial.y))
                        .abs()
                            < 1e-9
                    );
                    assert!((zoomed.y.span() - initial.y.span() * factor).abs() < 1e-9);
                }
                let restored = controller
                    .wheel(point, 1.0 / factor, geometry, zoomed)
                    .unwrap()
                    .apply(zoomed);
                assert!((restored.x.min - initial.x.min).abs() < 1e-9);
                assert!((restored.x.max - initial.x.max).abs() < 1e-9);
                assert!((restored.y.min - initial.y.min).abs() < 1e-9);
                assert!((restored.y.max - initial.y.max).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn top_and_right_wheel_zoom_both_axes_around_pointer() {
        let geometry = geometry();
        let initial = Ranges::new((10.0, 30.0), (-20.0, 60.0));
        let mut controller = Controller::new(HitZones::default(), Axes::BOTH);
        for point in [(200.0, 4.0), (480.0, 220.0)] {
            assert_eq!(controller.hint(point, geometry, false), Some(Gesture::Pan));
            let update = controller.wheel(point, 0.5, geometry, initial).unwrap();
            assert!(update.x.is_some() && update.y.is_some());
            let zoomed = update.apply(initial);
            assert!(
                (geometry.x_value(point.0, zoomed.x) - geometry.x_value(point.0, initial.x)).abs()
                    < 1e-9
            );
            assert!(
                (geometry.y_value(point.1, zoomed.y) - geometry.y_value(point.1, initial.y)).abs()
                    < 1e-9
            );
        }
    }

    #[test]
    fn double_click_needs_close_clicks_in_time_and_space() {
        let ms = Duration::from_millis;
        assert!(is_double_click(ms(300), (100.0, 100.0), (102.0, 101.0)));
        assert!(!is_double_click(ms(501), (100.0, 100.0), (100.0, 100.0)));
        assert!(!is_double_click(ms(100), (100.0, 100.0), (105.0, 100.0)));
    }

    #[test]
    fn controller_pans_and_reports_only_moved_axes() {
        let mut pan = Controller::new(HitZones::default(), Axes::X_ONLY);
        let ranges = ((0.0, 10.0), (0.0, 5.0));
        assert_eq!(
            pan.press((200.0, 150.0), geometry(), ranges, false),
            Some(Gesture::Pan)
        );
        assert_eq!(
            pan.drag_to((200.0, 150.0), geometry()),
            None,
            "no movement yet"
        );
        let update = pan.drag_to((249.0, 150.0), geometry()).unwrap();
        assert!((update.x.unwrap().min + 1.0).abs() < 1e-9);
        assert_eq!(update.y, None);
        assert_eq!(
            pan.release(Instant::now(), false, geometry()),
            Release::Dragged
        );
        assert_eq!(pan.active(), None);
    }

    #[test]
    fn controller_box_zooms_on_release() {
        let mut controller = Controller::new(HitZones::default(), Axes::BOTH);
        let ranges = ((0.0, 10.0), (0.0, 10.0));
        controller.press((98.0, 152.0), geometry(), ranges, true);
        assert_eq!(controller.drag_to((196.0, 4.0), geometry()), None);
        assert_eq!(
            controller.selection(geometry()),
            Some(((98.0, 196.0), (152.0, 4.0)))
        );
        let Release::Zoom(update) = controller.release(Instant::now(), true, geometry()) else {
            panic!("box selection zooms");
        };
        let zoomed = update.apply(ranges);
        assert!((zoomed.x.min - 2.0).abs() < 1e-9 && (zoomed.y.min - 5.0).abs() < 1e-9);
    }

    #[test]
    fn aspect_locked_selection_matches_applied_zoom_in_every_direction() {
        let g = PlotGeometry::with_bounds(0., 0., 400., 200., 400., 200.).unwrap();
        let ranges = Ranges::new((0., 20.), (0., 10.));
        for pointer in [
            (300., 110.),
            (100., 110.),
            (300., 90.),
            (100., 90.),
            (400., 200.),
        ] {
            let mut controller = Controller::new(HitZones::default(), Axes::BOTH);
            controller.selection_aspect = Some(2.);
            controller.press((200., 100.), g, ranges, true);
            controller.drag_to(pointer, g);
            let ((x0, x1), (y0, y1)) = controller.selection(g).unwrap();
            assert!(((x1 - x0).abs() / (y1 - y0).abs() - 2.).abs() < 1e-9);
            assert!((0.0..=400.).contains(&x1) && (0.0..=200.).contains(&y1));
            let Release::Zoom(update) = controller.release(Instant::now(), true, g) else {
                panic!("Selection must zoom");
            };
            let zoomed = update.apply(ranges);
            assert!((zoomed.x.span() / zoomed.y.span() - 2.).abs() < 1e-9);
            assert!((zoomed.x.span() - (x1 - x0).abs() / 400. * 20.).abs() < 1e-9);
        }
        let mut controller = Controller::new(HitZones::default(), Axes::BOTH);
        controller.press((200., 100.), g, ranges, true);
        controller.drag_to((300., 110.), g);
        assert_eq!(controller.selection(g), Some(((200., 300.), (100., 110.))));
    }

    #[test]
    fn controller_detects_double_clicks_and_cancels_to_initial() {
        let mut controller = Controller::new(HitZones::default(), Axes::BOTH);
        let start = Instant::now();
        let ranges = ((0.0, 10.0), (0.0, 10.0));
        controller.press((200.0, 150.0), geometry(), ranges, false);
        assert_eq!(controller.release(start, false, geometry()), Release::Click);
        controller.press((201.0, 150.0), geometry(), ranges, false);
        let later = start + Duration::from_millis(200);
        assert_eq!(
            controller.release(later, false, geometry()),
            Release::DoubleClick
        );
        controller.press((200.0, 150.0), geometry(), ranges, false);
        controller.drag_to((300.0, 150.0), geometry());
        assert_eq!(controller.cancel(), Some(Ranges::from(ranges)));
        assert_eq!(controller.release(later, false, geometry()), Release::Idle);
    }

    #[test]
    fn controller_wheel_updates_the_grabbed_axis() {
        let mut controller = Controller::new(HitZones::default(), Axes::BOTH);
        let ranges = ((0.0, 10.0), (0.0, 10.0));
        let update = controller
            .wheel((10.0, 78.0), 2.0, geometry(), ranges)
            .unwrap();
        assert_eq!((update.x, update.y), (None, Some(Range::new(-7.5, 12.5))));
        assert_eq!(
            controller.wheel((-50.0, 150.0), 2.0, geometry(), ranges),
            None
        );
    }

    #[test]
    fn cursor_names_follow_the_gesture() {
        assert_eq!(
            Gesture::cursor(Some(Gesture::XEdge(Edge::Min))),
            "ew-resize"
        );
        assert_eq!(
            Gesture::cursor(Some(Gesture::YEdge(Edge::Max))),
            "ns-resize"
        );
        assert_eq!(Gesture::cursor(None), "crosshair");
    }
}
