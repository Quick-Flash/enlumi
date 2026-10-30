//! Reactive pointer handling for zgui plots.
//!
//! [`PlotPointer`] wraps an [`Controller`] so event handlers can
//! share it, and exposes the cursor hint and selection box as signals for
//! drawing and styling. Handlers pass plot-local pixel positions (see
//! [`crate::zgui_renderer::local_position`]) and apply the returned updates
//! to their own range state.

use std::time::Instant;

use zgui::prelude::*;

use crate::interaction::{Axes, Controller, Gesture, HitZones, RangeUpdate, Release};
use crate::{PlotGeometry, Ranges};

/// Selection box corners in plot pixels: `((x0, x1), (y0, y1))`.
pub type SelectionBox = ((f64, f64), (f64, f64));

/// Shared pointer state for one plot; `Copy` into each event handler.
#[derive(Clone, Copy)]
pub struct PlotPointer {
    controller: StoredValue<Controller, LocalStorage>,
    hint: RwSignal<Option<Gesture>, LocalStorage>,
    active: RwSignal<Option<Gesture>, LocalStorage>,
    selection: RwSignal<Option<SelectionBox>, LocalStorage>,
}

impl PlotPointer {
    /// Idle pointer with the given grab handles and free axes.
    pub fn new(zones: HitZones, axes: Axes) -> Self {
        Self {
            controller: StoredValue::new_local(Controller::new(zones, axes)),
            hint: RwSignal::new_local(None),
            active: RwSignal::new_local(None),
            selection: RwSignal::new_local(None),
        }
    }

    /// Constrain selection rectangles to a pixel aspect ratio, or allow free selection.
    pub fn set_selection_aspect(&self, aspect: Option<f64>) {
        self.update(|controller| controller.selection_aspect = aspect);
    }

    /// Gesture a press would start where the pointer last hovered (reactive).
    pub fn hint(&self) -> Option<Gesture> {
        self.hint.get()
    }

    /// Gesture of the drag in progress (reactive).
    pub fn active(&self) -> Option<Gesture> {
        self.active.get()
    }

    /// CSS cursor for the active drag, or else the hovered zone (reactive).
    pub fn cursor(&self) -> &'static str {
        Gesture::cursor(self.active.get().or(self.hint.get()))
    }

    /// Box being selected, in plot pixels (reactive).
    pub fn selection(&self) -> Option<SelectionBox> {
        self.selection.get()
    }

    /// Whether a drag is in progress, without subscribing.
    pub fn is_dragging(&self) -> bool {
        self.active.get_untracked().is_some()
    }

    /// Update the hint for a pointer hovering at `pointer`.
    pub fn hover(&self, pointer: (f64, f64), geometry: PlotGeometry, select: bool) {
        let hint = self
            .controller
            .with_value(|controller| controller.hint(pointer, geometry, select));
        if self.hint.get_untracked() != hint {
            self.hint.set(hint);
        }
    }

    /// The pointer left the plot.
    pub fn leave(&self) {
        self.hint.set(None);
    }

    /// Axis under pointer, using the same zones as axis dragging.
    pub fn axis_at(
        &self,
        pointer: (f64, f64),
        geometry: PlotGeometry,
    ) -> Option<crate::labels::Axis> {
        self.controller.with_value(
            |controller| match controller.hint(pointer, geometry, false) {
                Some(Gesture::XEdge(_)) => Some(crate::labels::Axis::X),
                Some(Gesture::YEdge(_)) => Some(crate::labels::Axis::Y),
                _ => None,
            },
        )
    }

    /// Start a drag; returns what it grabbed, or `None` outside every zone.
    pub fn press(
        &self,
        pointer: (f64, f64),
        geometry: PlotGeometry,
        ranges: impl Into<Ranges>,
        select: bool,
    ) -> Option<Gesture> {
        let ranges = ranges.into();
        let gesture = self
            .update(|controller| controller.press(pointer, geometry, ranges, select))
            .flatten();
        self.active.set(gesture);
        self.selection.set(None);
        gesture
    }

    /// Follow a drag; returns ranges to show while panning or dragging a handle.
    pub fn drag_to(&self, pointer: (f64, f64), geometry: PlotGeometry) -> Option<RangeUpdate> {
        let (update, selection) = self.update(|controller| {
            let update = controller.drag_to(pointer, geometry);
            (update, controller.selection(geometry))
        })?;
        if selection.is_some() {
            self.selection.set(selection);
        }
        update
    }

    /// End the drag.
    pub fn release(&self, select: bool, geometry: PlotGeometry) -> Release {
        let release = self
            .update(|controller| controller.release(Instant::now(), select, geometry))
            .unwrap_or(Release::Idle);
        self.clear();
        release
    }

    /// Wheel zoom by `factor` at `pointer`, ending any drag.
    pub fn wheel(
        &self,
        pointer: (f64, f64),
        factor: f64,
        geometry: PlotGeometry,
        ranges: impl Into<Ranges>,
    ) -> Option<RangeUpdate> {
        let ranges = ranges.into();
        let update = self.update(|controller| controller.wheel(pointer, factor, geometry, ranges));
        self.clear();
        update.flatten()
    }

    /// Abandon the drag; returns the ranges it started from.
    pub fn cancel(&self) -> Option<Ranges> {
        let initial = self.update(Controller::cancel).flatten();
        self.clear();
        initial
    }

    /// Releasing selection modifier abandons box zoom until next press.
    pub fn cancel_selection(&self, select: bool) -> Option<Ranges> {
        if !select && self.active.get_untracked() == Some(Gesture::Select) {
            let initial = self.cancel();
            self.leave();
            initial
        } else {
            None
        }
    }

    fn clear(&self) {
        self.active.set(None);
        self.selection.set(None);
    }

    fn update<U>(&self, f: impl FnOnce(&mut Controller) -> U) -> Option<U> {
        self.controller.try_update_value(f)
    }
}
