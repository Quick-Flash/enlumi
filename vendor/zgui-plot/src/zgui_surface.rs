//! Interactive plot surface for zgui: the shell every plot shares.
//!
//! [`PlotSurface`] stacks the caller's layers (canvases, legend, overlays),
//! positions tick labels, and covers them with a focusable input layer that
//! turns pointer, wheel and keyboard events into range updates through a
//! [`PlotPointer`]. Drag updates are coalesced to one per frame.
//!
//! Include [`stylesheet`] once in the app stylesheet for the surface layout.

mod hover;

use crate::zgui_link::{PlotLinks, linked_pointer_path};
use zgui::prelude::*;
use zgui::reactive::{Callable, UnsyncCallback};
use zgui::vocab::ScrollDelta;
use zgui::{component, view};

use crate::interaction::{Gesture, RangeUpdate, Release};
use crate::labels::{Axis, TickLabel};
use crate::zgui_pointer::PlotPointer;
use crate::zgui_renderer::local_position;
use crate::{PlotGeometry, Ranges};

/// What the pointer is doing over the plot, for the caller's hover state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlotHover {
    /// Hovering at this plot-local position (pixels).
    At((f64, f64)),
    /// Pressing, dragging or zooming; hover readouts should clear.
    Busy,
    /// The pointer left the plot.
    Left,
}

/// Layout rules for the surface's classes: absolutely positioned tick labels
/// (`plot-tick`, `plot-tick-x`, `plot-tick-y`) and the input layer
/// (`plot-input`), and the outer box (`plot-surface`), which stacks and clips
/// them. Sizing, text styling and label widths are left to the app.
pub fn stylesheet() -> &'static str {
    ".plot-surface { position: relative; overflow: hidden; }\n\
     .plot-tick { position: absolute; pointer-events: none; }\n\
     .plot-tick-x { text-align: center; }\n\
     .plot-tick-y { text-align: left; }\n\
     .plot-cursor-values { position: absolute; height: 18px; text-align: right; pointer-events: none; overflow: hidden; white-space: nowrap; background: transparent; }\n\
     .plot-title { position: absolute; left: 4px; right: 4px; height: 20px; text-align: center; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; pointer-events: none; background: transparent; }\n\
     .plot-surface.plot-has-title .plot-legend { top: 24px; }\n\
     .plot-linked-pointer { position: absolute; left: 0; top: 0; width: 100%; height: 100%; pointer-events: none; }\n\
     .plot-input { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }\n"
}

/// Zoom factor for a wheel or trackpad scroll; fractional deltas compose to
/// the same result as whole notches.
pub fn wheel_factor(delta: &ScrollDelta) -> f64 {
    let pixels = match delta {
        ScrollDelta::Lines { y, .. } => f64::from(*y) * 24.0,
        ScrollDelta::Pixels(delta) => f64::from(delta.height.0),
        _ => 0.0,
    };
    crate::range::wheel_zoom_factor(pixels)
}

/// Geometry and range callbacks for an interactive plot surface.
#[derive(Clone)]
pub struct SurfaceInteraction {
    /// Current plot geometry, or `None` before the box is measured.
    pub geometry: UnsyncCallback<(), Option<PlotGeometry>>,
    /// Ranges currently shown.
    pub ranges: UnsyncCallback<(), Ranges>,
    /// Show ranges changed by dragging, box selection or the wheel.
    pub on_update: UnsyncCallback<RangeUpdate>,
    /// Double click: return to the full view.
    pub on_reset: UnsyncCallback<()>,
    pub on_hover: UnsyncCallback<PlotHover>,
}
/// Presentation and optional gestures for an interactive plot surface.
#[derive(Clone)]
pub struct SurfaceOptions {
    /// Double-click on an axis handle: fit only that axis. Without callback,
    /// axis double-clicks fall back to `on_reset`.
    pub on_fit_axis: Option<UnsyncCallback<Axis>>,
    /// Called on every primary press before a gesture starts; return `true`
    /// to consume the press (for example a pending export click).
    pub on_press: Option<UnsyncCallback<(f64, f64), bool>>,
    /// Escape or pointer cancellation during a drag: restore starting ranges. Without it,
    /// Escape is not handled.
    pub on_cancel: Option<UnsyncCallback<Ranges>>,
    /// Positioned tick labels.
    pub ticks: Memo<Vec<TickLabel>>,
    /// Centered title inside plot; legend is moved below it.
    pub title: Option<Signal<String, LocalStorage>>,
    /// Histograms show only the cursor's X value.
    pub cursor_x_only: bool,
    /// Format cursor data coordinates, for example a date on the X axis.
    pub cursor_format: Option<UnsyncCallback<(f64, f64), String>>,
    /// Anchor cursor values below the caller's legend, aligned to its right edge.
    pub legend_ref: Option<NodeRef>,
    /// Background colour of the plot box.
    pub background: String,
    pub show_pointer: bool,
    pub show_cursor_values: bool,
}
impl Default for SurfaceOptions {
    fn default() -> Self {
        Self {
            on_fit_axis: None,
            on_press: None,
            on_cancel: None,
            ticks: Memo::new(|_| Vec::new()),
            title: None,
            cursor_x_only: false,
            cursor_format: None,
            legend_ref: None,
            background: "#12181c".into(),
            show_pointer: true,
            show_cursor_values: true,
        }
    }
}

#[component]
pub fn PlotSurface(
    node_ref: NodeRef,
    #[prop(optional)] class: Option<&'static str>,
    label: String,
    pointer: PlotPointer,
    children: Children,
    interaction: SurfaceInteraction,
    #[prop(default = SurfaceOptions::default())] options: SurfaceOptions,
) -> impl IntoView {
    let SurfaceInteraction {
        geometry,
        ranges,
        on_update,
        on_reset,
        on_hover,
    } = interaction;
    let SurfaceOptions {
        on_fit_axis,
        on_press,
        on_cancel,
        ticks,
        title,
        cursor_x_only,
        cursor_format,
        legend_ref,
        background,
        show_pointer,
        show_cursor_values,
    } = options;

    let reset_at = move |position: Option<(f64, f64)>| {
        if let Some(axis) = position.and_then(|position| {
            geometry
                .run(())
                .and_then(|geometry| pointer.axis_at(position, geometry))
        }) && let Some(on_fit_axis) = on_fit_axis
        {
            on_fit_axis.run(axis);
        } else {
            on_reset.run(());
        }
    };
    let links = use_local_context::<PlotLinks>();
    if let Some(links) = links {
        links.bind(ranges, on_update);
    }
    let hover_callback = on_hover;
    let on_hover = hover::coalesced(UnsyncCallback::new(move |event: PlotHover| {
        if let Some(links) = links {
            let point = match event {
                PlotHover::At((x, y)) => geometry.run(()).and_then(|geometry| {
                    if !(geometry.left..=geometry.right).contains(&x)
                        || !(geometry.top..=geometry.bottom).contains(&y)
                    {
                        return None;
                    }
                    let ranges = ranges.run(());
                    Some(crate::PlotPoint {
                        x: geometry.x_value(x, ranges.x),
                        y: geometry.y_value(y, ranges.y),
                    })
                }),
                _ => None,
            };
            links.publish_pointer(point);
        }
        hover_callback.run(event);
    }));
    let linked_canvas = zgui::elements::canvas()
        .class("plot-linked-pointer")
        .draw(move |cx| {
            let Some(point) = links.and_then(PlotLinks::pointer) else {
                return;
            };
            let Some(geometry) = geometry.run(()) else {
                return;
            };
            let path = linked_pointer_path(point, ranges.run(()), geometry);
            crate::zgui_renderer::push_stroke(
                cx.scene,
                path.clone(),
                zgui::canvas::zgui_color::Color::srgb_u8(0, 0, 0, 210),
                3.0,
            );
            crate::zgui_renderer::push_stroke(
                cx.scene,
                path,
                zgui::canvas::zgui_color::Color::srgb_u8(244, 247, 245, 255),
                1.0,
            );
        });
    let linked_canvas = show_pointer.then_some(linked_canvas.into_view());
    let input = NodeRef::new();
    let surface_bounds = node_ref.observe_border_box();
    let legend_bounds = legend_ref.map(|node| node.observe_border_box());
    let cursor_position = RwSignal::new_local(None::<(f64, f64)>);
    let cursor_values = move || {
        let geometry = geometry.run(())?;
        let current_ranges = ranges.run(());
        let peer = links.and_then(PlotLinks::pointer);
        let (x, y) = cursor_position.get().or_else(|| {
            peer.map(|point| {
                (
                    geometry.x(point.x, current_ranges.x),
                    geometry.y(point.y, current_ranges.y),
                )
            })
        })?;
        let outside = !(geometry.left..=geometry.right).contains(&x)
            || !(geometry.top..=geometry.bottom).contains(&y);
        if outside
            && peer.is_none_or(|point| {
                !current_ranges.x.contains(point.x) && !current_ranges.y.contains(point.y)
            })
        {
            return None;
        }
        let ranges = ranges.run(());
        let values = (geometry.x_value(x, ranges.x), geometry.y_value(y, ranges.y));
        let text = cursor_format.map_or_else(
            || {
                let x = crate::format_number(values.0);
                if cursor_x_only {
                    format!("X: {x}")
                } else {
                    format!("X: {x} | Y: {}", crate::format_number(values.1))
                }
            },
            |format| format.run(values),
        );
        // Tick labels live inside the data area too. Reserve their actual boxes.
        let (left, bottom) = ticks.with(|ticks| {
            let left = ticks
                .iter()
                .filter(|tick| {
                    tick.axis == Axis::Y && tick.left < (geometry.left + geometry.right) / 2.0
                })
                .map(|tick| tick.left + tick.width + 6.0)
                .fold(geometry.left + 6.0, f64::max);
            let bottom = ticks
                .iter()
                .filter(|tick| tick.axis == Axis::X)
                .map(|tick| tick.top)
                .fold(geometry.bottom, f64::min);
            (left, bottom)
        });
        let (top, right) = match legend_bounds {
            Some(bounds) => {
                let legend = bounds.get()?;
                let surface = surface_bounds.get()?;
                let scale = node_ref.scale();
                (
                    f64::from(
                        (legend.origin.y.0 + legend.size.height.0 - surface.origin.y.0) / scale,
                    ) + 6.0,
                    f64::from(
                        (legend.origin.x.0 + legend.size.width.0 - surface.origin.x.0) / scale,
                    )
                    .min(geometry.right - 6.0),
                )
            }
            None => (bottom - 24.0, geometry.right - 6.0),
        };
        let width = right - left;
        if width <= 0.0 || top < geometry.top + 24.0 || top + 18.0 > bottom - 6.0 {
            return None;
        }
        Some((text, left, top, width))
    };
    let mut cursor_style = crate::title_text_style();
    cursor_style.font_size = 11.0;
    cursor_style.line_height = 16.0;
    let pending = StoredValue::new_local(None::<(f64, f64)>);
    // Coalesce high-rate trackpad input into one range update per frame.
    let wheeled = StoredValue::new_local(None::<((f64, f64), f64)>);
    let frame = StoredValue::new_local(None::<zgui::view::FrameHandle>);
    let clock = StoredValue::new_local(Timers::current().expect("plot surface window clock"));
    let apply_wheel = move || {
        let wheel = wheeled.get_value();
        wheeled.set_value(None);
        if let Some((position, factor)) = wheel
            && let Some(geometry) = geometry.run(())
            && let Some(update) = pointer.wheel(position, factor, geometry, ranges.run(()))
        {
            on_update.run(update);
        }
    };
    let flush = move || {
        frame.set_value(None);
        let position = pending.get_value();
        pending.set_value(None);
        if let Some(position) = position
            && let Some(geometry) = geometry.run(())
            && let Some(update) = pointer.drag_to(position, geometry)
        {
            on_update.run(update);
        }
        apply_wheel();
    };
    let request = move || {
        if frame.with_value(|frame| frame.is_none()) {
            let handle = clock.with_value(|clock| clock.request_frame(move |_| flush()));
            frame.set_value(Some(handle));
        }
    };
    on_cleanup_local(move || frame.set_value(None));
    let cursor = move || match pointer.active().or(pointer.hint()) {
        None | Some(Gesture::Pan) => None,
        gesture => Some(Gesture::cursor(gesture).to_owned()),
    };
    let tick = move |slot: usize, read: fn(&TickLabel) -> String| {
        ticks.with(|ticks| ticks.get(slot).map(read))
    };
    let is_axis = move |slot: usize, axis: Axis| {
        ticks.with(|ticks| ticks.get(slot).is_some_and(|tick| tick.axis == axis))
    };
    view! {
        box(class = match class { Some(class) => format!("plot-surface {class}"), None => "plot-surface".to_owned() },
            node_ref = node_ref, style:background = Some(background),
            class:plot-has-title = move || title.is_some_and(|title| !title.get().is_empty())) {
            {children.into_view_once()}
            {linked_canvas.into_view()}
            for slot in move || (0..ticks.with(|ticks| ticks.len())).collect::<Vec<_>>(), key = |slot| *slot {
                text(class = "plot-tick",
                    class:plot-tick-x = move || is_axis(slot, Axis::X),
                    class:plot-tick-y = move || is_axis(slot, Axis::Y),
                    style:opacity = move || tick(slot, |tick| tick.opacity.to_string()),
                    style:left = move || tick(slot, |tick| format!("{}px", tick.left)),
                    style:top = move || tick(slot, |tick| format!("{}px", tick.top))) {
                    {move || tick(slot, |tick| tick.text.clone()).unwrap_or_default()}
                }
            }
            if move || title.is_some_and(|title| !title.get().is_empty()) {
                text(class = "plot-title", style = Some(crate::title_text_style().css()),
                    style:top = Some(format!("{}px", crate::TITLE_TOP))) {
                    {move || title.map(|title| title.get()).unwrap_or_default()}
                }
            }
            if move || show_cursor_values {
            text(class = "plot-cursor-values", style = Some(cursor_style.css()),
                style:left = move || cursor_values().map(|(_, left, _, _)| format!("{left}px")),
                style:top = move || cursor_values().map(|(_, _, top, _)| format!("{top}px")),
                style:width = move || cursor_values().map(|(_, _, _, width)| format!("{width}px"))) {
                {move || cursor_values().map(|(text, _, _, _)| text).unwrap_or_default()}
            }
            }
            box(class = "plot-input", node_ref = input, tabindex = Focus::Sequential, a11y:label = label,
                style:cursor = cursor,
                on:key_down = move |ev| {
                    if ev.key == Key::Named(NamedKey::Escape)
                        && let Some(on_cancel) = on_cancel
                        && let Some(initial) = pointer.cancel()
                    {
                        frame.set_value(None); pending.set_value(None); wheeled.set_value(None);
                        on_cancel.run(initial);
                        ev.prevent_default(); ev.stop_propagation();
                    }
                },
                on:key_up = move |ev| {
                    if ev.key == Key::Named(NamedKey::Shift)
                        && let Some(initial) = pointer.cancel_selection(false) {
                        frame.set_value(None); pending.set_value(None); wheeled.set_value(None);
                        if let Some(on_cancel) = on_cancel { on_cancel.run(initial); }
                        ev.release_pointer(); ev.prevent_default(); ev.stop_propagation();
                    }
                },
                on:pointer_down = move |ev| {
                    if ev.button != Some(PointerButton::Primary) { return; }
                    cursor_position.set(None);
                    let Some(position) = local_position(&node_ref, ev.position) else { return };
                    if on_press.is_some_and(|on_press| on_press.run(position)) {
                        ev.prevent_default(); ev.stop_propagation(); return;
                    }
                    let Some(geometry) = geometry.run(()) else { return };
                    apply_wheel();
                    frame.set_value(None); pending.set_value(None);
                    if pointer.press(position, geometry, ranges.run(()), ev.modifiers.shift()).is_some() {
                        input.focus();
                        on_hover.run(PlotHover::Busy);
                        ev.capture_pointer();
                    }
                },
                on:pointer_move = move |ev| {
                    let Some(position) = local_position(&node_ref, ev.position) else { return };
                    let Some(geometry) = geometry.run(()) else { return };
                    if let Some(initial) = pointer.cancel_selection(ev.modifiers.shift()) {
                        frame.set_value(None); pending.set_value(None); wheeled.set_value(None);
                        if let Some(on_cancel) = on_cancel { on_cancel.run(initial); }
                        ev.release_pointer();
                    }
                    pointer.hover(position, geometry, ev.modifiers.shift());
                    if pointer.is_dragging() {
                        cursor_position.set(None);
                        pending.set_value(Some(position));
                        request();
                        on_hover.run(PlotHover::Busy);
                    } else {
                        cursor_position.set(Some(position));
                        on_hover.run(PlotHover::At(position));
                    }
                },
                on:pointer_up = move |ev| {
                    if let Some(initial) = pointer.cancel_selection(ev.modifiers.shift()) {
                        frame.set_value(None); pending.set_value(None); wheeled.set_value(None);
                        if let Some(on_cancel) = on_cancel { on_cancel.run(initial); }
                        ev.release_pointer(); return;
                    }
                    if let Some(position) = local_position(&node_ref, ev.position) {
                        pending.set_value(Some(position));
                    }
                    flush();
                    if let Some(geometry) = geometry.run(()) {
                        match pointer.release(ev.modifiers.shift(), geometry) {
                            Release::DoubleClick => reset_at(local_position(&node_ref, ev.position)),
                            Release::Zoom(update) => on_update.run(update),
                            _ => {}
                        }
                    }
                    ev.release_pointer();
                },
                on:pointer_cancel = move |ev| {
                    cursor_position.set(None);
                    frame.set_value(None); pending.set_value(None); wheeled.set_value(None);
                    if let Some(initial) = pointer.cancel() && let Some(on_cancel) = on_cancel { on_cancel.run(initial); }
                    pointer.leave();
                    on_hover.run(PlotHover::Left);
                    ev.release_pointer();
                },
                on:pointer_leave = move |_| { cursor_position.set(None); pointer.leave(); on_hover.run(PlotHover::Left); },
                on:double_click = move |ev| { cursor_position.set(None); reset_at(local_position(&node_ref, ev.position)); },
                on:wheel = move |ev| {
                    let factor = wheel_factor(&ev.delta);
                    if factor == 1.0 { return; }
                    let Some(position) = local_position(&node_ref, ev.position) else { return };
                    if geometry.run(()).is_none() { return; }
                    let total = wheeled.get_value().map_or(1.0, |(_, total)| total) * factor;
                    wheeled.set_value(Some((position, total)));
                    request();
                    cursor_position.set(None);
                    on_hover.run(PlotHover::Busy);
                    ev.prevent_default(); ev.stop_propagation();
                }
            ) {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_factor_preserves_fractional_input() {
        let zoom = |y| wheel_factor(&ScrollDelta::Lines { x: 0.0, y });
        assert!((zoom(0.1).powi(10) - zoom(1.0)).abs() < 1e-8);
        assert!((zoom(1.0) * zoom(-1.0) - 1.0).abs() < 1e-12);
        assert_eq!(zoom(0.0), 1.0);
        assert_eq!(zoom(f32::NAN), 1.0);
    }

    #[test]
    fn stylesheet_has_one_rule_per_line() {
        let css = stylesheet();
        assert!(css.lines().all(|line| line.trim().ends_with('}')));
        assert_eq!(css.lines().count(), 9);
    }
}
