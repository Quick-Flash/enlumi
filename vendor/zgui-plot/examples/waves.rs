//! An interactive plot of large data: a noisy damped sine line and scattered
//! cosine samples, up to five million line points and a million markers.
//!
//! Run it with `cargo run --release --example waves --features zgui`.
//!
//! Drag to pan, scroll to zoom, drag near an axis edge to stretch that axis,
//! Shift-drag to box zoom, Escape to cancel a drag, double-click to reset.
//! Click a legend entry to hide its series; hover one to emphasise it.
//!
//! The data is built once per size and kept in data space. A pan or a zoom
//! only moves the canvas view, so the series are never rebuilt and nothing is
//! uploaded again; the grid and the hover overlay are small path layers that
//! follow the ranges. The toolbar switches the data size, the marker shape and
//! the line's level of detail, and starts a sweep that pans on every frame so
//! the frame rate can be read without touching the plot; it pans and zooms. `WAVES_SWEEP=1`
//! starts the sweep at launch, for an unattended measurement.

mod support;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use zgui::canvas::zgui_color::Color;
use zgui::canvas::{Lod, Marker};
use zgui::elements::canvas::CanvasHandle;
use zgui::prelude::*;
use zgui::reactive::UnsyncCallback;
use zgui::vocab::Timestamp;
use zgui_plot::interaction::{Axes, HitZones, RangeUpdate};
use zgui_plot::labels::{AxisLabels, TickLabelLayout, XLabelRow, tick_labels};
use zgui_plot::overlay::{HoverTarget, hover_crosshair};
use zgui_plot::zgui_legend::{LegendItemProps, LegendMarker, toggle_member};
use zgui_plot::zgui_pointer::PlotPointer;
use zgui_plot::zgui_renderer::{
    push_grid, push_line, push_points, push_selection, push_stroke, redraw_on_change, series_data,
    show_ranges,
};
use zgui_plot::zgui_surface::{PlotHover, PlotSurfaceProps};
use zgui_plot::{
    GridOptions, LabelOptions, LegendEntryStyle, LegendTheme, PlotGeometry, PlotPoint, Ranges,
    TextStyle, cross_marker, format_number, grid_paths, nearest_sorted, triangle_marker,
};

const LAYOUT: TickLabelLayout = TickLabelLayout {
    x_label: (64.0, 16.0),
    y_label: (56.0, 16.0),
    inset: 4.0,
    x_row: XLabelRow::Inside,
};

/// The x extent of the data.
const SPAN: f64 = 4.0 * std::f64::consts::PI;

const LINE_SIZES: [usize; 4] = [600, 100_000, 5_000_000, 10_000_000];
const POINT_SIZES: [usize; 4] = [80, 100_000, 1_000_000, 2_000_000];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Series {
    Wave,
    Samples,
}

impl Series {
    const ALL: [Self; 2] = [Self::Wave, Self::Samples];

    fn label(self) -> &'static str {
        match self {
            Self::Wave => "sin(x)·e^(-x/8) + noise",
            Self::Samples => "noisy cos(x)",
        }
    }

    fn marker(self) -> LegendMarker {
        match self {
            Self::Wave => LegendMarker::Line,
            Self::Samples => LegendMarker::Dot,
        }
    }

    fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Wave => (110, 168, 255),
            Self::Samples => (255, 182, 72),
        }
    }

    fn color(self, alpha: f32) -> Color {
        let (r, g, b) = self.rgb();
        Color::srgb_u8(r, g, b, 255).with_alpha(alpha)
    }

    fn css(self) -> String {
        let (r, g, b) = self.rgb();
        format!("rgb({r}, {g}, {b})")
    }
}

/// The marker shapes the toolbar offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Circle,
    Square,
    Triangle,
    Cross,
}

impl Shape {
    const ALL: [Self; 4] = [Self::Circle, Self::Square, Self::Triangle, Self::Cross];

    fn label(self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::Square => "square",
            Self::Triangle => "triangle",
            Self::Cross => "cross",
        }
    }
}

/// The markers, with the path outlines built once.
struct Markers {
    triangle: Arc<kurbo::BezPath>,
    cross: Arc<kurbo::BezPath>,
}

impl Markers {
    fn new() -> Self {
        Self {
            triangle: Arc::new(triangle_marker(3.6)),
            cross: Arc::new(cross_marker(3.4, 1.4)),
        }
    }

    fn of(&self, shape: Shape) -> Marker {
        match shape {
            Shape::Circle => Marker::Circle { radius: 2.4 },
            Shape::Square => Marker::Square { half: 2.1 },
            Shape::Triangle => Marker::Path(self.triangle.clone()),
            Shape::Cross => Marker::Path(self.cross.clone()),
        }
    }
}

/// One data set: the points for hover snapping, the same points as series
/// data, and how long they took to build. Equal by allocation, so a memo of it
/// changes only when the size changes.
#[derive(Clone)]
struct Dataset {
    points: Arc<[PlotPoint]>,
    data: Arc<[[f32; 2]]>,
    built: Duration,
}

impl PartialEq for Dataset {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}

impl Dataset {
    fn build(points: impl FnOnce() -> Vec<PlotPoint>) -> Self {
        let start = Instant::now();
        let points: Arc<[PlotPoint]> = points().into();
        let data = series_data(&points);
        Self {
            points,
            data,
            built: start.elapsed(),
        }
    }

    fn len(&self) -> usize {
        self.points.len()
    }
}

/// Deterministic noise in [-0.5, 0.5), so the example needs no RNG dependency.
fn noise(i: usize, seed: f64) -> f64 {
    let value = ((i as f64) * 12.9898 + seed * 78.233).sin() * 43_758.545;
    value - value.floor() - 0.5
}

fn wave(count: usize) -> Vec<PlotPoint> {
    let amplitude = if count > 1_000 { 0.08 } else { 0.0 };
    (0..count)
        .map(|i| {
            let x = i as f64 * SPAN / (count - 1) as f64;
            PlotPoint {
                x,
                y: x.sin() * (-x / 8.0).exp() + noise(i, 1.0) * amplitude,
            }
        })
        .collect()
}

/// Samples sorted by x, which [`nearest_sorted`] needs for hover snapping.
fn samples(count: usize) -> Vec<PlotPoint> {
    let step = SPAN / count as f64;
    (0..count)
        .map(|i| {
            let x = (i as f64 + 0.5 + noise(i, 2.0) * 0.9) * step;
            PlotPoint {
                x,
                y: x.cos() * 0.8 + noise(i, 3.0) * 0.3,
            }
        })
        .collect()
}

fn full_view() -> Ranges {
    Ranges::new((0.0, SPAN), (-1.4, 1.4))
}

/// The ranges the sweep shows at `seconds`: panned back and forth across the
/// data while zooming between the whole of it and a sixty-fourth of it.
fn sweep_view(seconds: f64) -> Ranges {
    let zoom = 0.5 - 0.5 * (seconds * 0.35).cos();
    let span = SPAN * 64f64.powf(-zoom);
    let centre = SPAN / 2.0 + (SPAN - span) / 2.0 * (seconds * 0.5).sin();
    Ranges::new((centre - span / 2.0, centre + span / 2.0), (-1.4, 1.4))
}

/// Counts frames while the plot moves, and drives the sweep.
///
/// It runs one frame callback at a time, only while sweeping or for half a
/// second after the view last moved, so an idle window stays idle.
struct FrameMeter {
    clock: Timers,
    ranges: RwSignal<Ranges>,
    sweeping: RwSignal<bool>,
    rate: RwSignal<Option<f64>>,
    state: RefCell<MeterState>,
}

#[derive(Default)]
struct MeterState {
    handle: Option<zgui::view::FrameHandle>,
    last: Option<Timestamp>,
    moved: Option<Timestamp>,
    sweep_start: Option<Timestamp>,
    interval: Option<f64>,
}

impl FrameMeter {
    /// Notes that the view moved, and keeps the meter running for at least
    /// another half second.
    fn kick(self: &Rc<Self>) {
        let mut state = self.state.borrow_mut();
        state.moved = None;
        if state.handle.is_none() {
            drop(state);
            self.schedule();
        }
    }

    fn schedule(self: &Rc<Self>) {
        let meter = self.clone();
        let handle = self.clock.request_frame(move |at| meter.tick(at));
        self.state.borrow_mut().handle = Some(handle);
    }

    fn tick(self: &Rc<Self>, at: Timestamp) {
        let sweeping = self.sweeping.get_untracked();
        let mut state = self.state.borrow_mut();
        state.handle = None;
        if let Some(last) = state.last {
            let seconds = at.saturating_since(last).as_secs_f64();
            if seconds > 0.0 && seconds < 0.25 {
                let interval = state
                    .interval
                    .map_or(seconds, |old| old * 0.9 + seconds * 0.1);
                state.interval = Some(interval);
                self.rate.set(Some(1.0 / interval));
            }
        }
        state.last = Some(at);
        let moved = *state.moved.get_or_insert(at);
        let start = *state.sweep_start.get_or_insert(at);
        if !sweeping {
            state.sweep_start = None;
        }
        let quiet = at.saturating_since(moved) > Duration::from_millis(500);
        drop(state);
        if sweeping {
            self.ranges
                .set(sweep_view(at.saturating_since(start).as_secs_f64()));
        }
        if sweeping || !quiet {
            self.schedule();
        } else {
            let mut state = self.state.borrow_mut();
            state.last = None;
            state.interval = None;
            state.moved = None;
            drop(state);
            self.rate.set(None);
        }
    }
}

/// A row of toggle buttons, one per option, with the selected one marked.
fn choices<T: Copy + PartialEq + Send + Sync + 'static>(
    selected: RwSignal<T>,
    options: Vec<(T, String)>,
) -> impl IntoView {
    let buttons = options
        .into_iter()
        .map(|(value, label)| {
            view! {
                control(
                    class = "choice",
                    class:choice-selected = move || selected.get() == value,
                    tabindex = Focus::Sequential,
                    on:click = move |_| selected.set(value)
                ) {
                    {label}
                }
            }
        })
        .collect::<Vec<_>>();
    view! {
        row(class = "choices") {
            {buttons}
        }
    }
}

fn count_label(count: usize) -> String {
    match count {
        n if n >= 1_000_000 => format!("{}M", n / 1_000_000),
        n if n >= 1_000 => format!("{}k", n / 1_000),
        n => n.to_string(),
    }
}

#[component]
fn Waves() -> impl IntoView {
    // Start small so window appears immediately; benchmark sizes remain selectable.
    let line_size = RwSignal::new(LINE_SIZES[0]);
    let point_size = RwSignal::new(POINT_SIZES[0]);
    let shape = RwSignal::new(Shape::Circle);
    let reduced = RwSignal::new(true);
    let sweeping = RwSignal::new(std::env::var_os("WAVES_SWEEP").is_some());
    let rate = RwSignal::new(None::<f64>);
    let markers = Rc::new(Markers::new());

    let wave = Memo::new(move |_| Dataset::build(|| wave(line_size.get())));
    let samples = Memo::new(move |_| Dataset::build(|| samples(point_size.get())));

    let ranges = RwSignal::new(full_view());
    let hidden = RwSignal::new(BTreeSet::<Series>::new());
    let emphasised = RwSignal::new_local(None::<Series>);
    let hover = RwSignal::new(None::<HoverTarget>);
    let pointer = PlotPointer::new(HitZones::default(), Axes::BOTH);
    let meter = Rc::new(FrameMeter {
        clock: Timers::current().expect("waves window clock"),
        ranges,
        sweeping,
        rate,
        state: RefCell::default(),
    });

    // The surface box is measured in device pixels; the plot works in CSS pixels.
    let surface = NodeRef::new();
    let size = surface.observe_content_size();
    let geometry = Memo::new(move |_| {
        let size = size.get();
        let scale = f64::from(surface.scale());
        let (width, height) = (
            f64::from(size.width.0) / scale,
            f64::from(size.height.0) / scale,
        );
        PlotGeometry::with_bounds(0.0, 0.0, width, height, width, height)
            .filter(|geometry| geometry.data_width() > 0.0 && geometry.data_height() > 0.0)
    });
    let ticks = Memo::new(move |_| {
        let Some(geometry) = geometry.get() else {
            return Vec::new();
        };
        tick_labels(
            ranges.get(),
            geometry,
            &LAYOUT,
            AxisLabels {
                options: LabelOptions::horizontal(0.0),
                format: format_number,
            },
            AxisLabels {
                options: LabelOptions::vertical(0.0),
                format: format_number,
            },
        )
    });

    let alpha = move |series: Series| match emphasised.get() {
        Some(other) if other != series => 0.25,
        _ => 1.0,
    };
    let shown = move |series: Series| !hidden.with(|hidden| hidden.contains(&series));

    // The grid follows the ranges: a few lines, rebuilt on every change.
    let grid = zgui::elements::canvas()
        .class("plot-layer")
        .draw(move |cx| {
            let Some(geometry) = geometry.get() else {
                return;
            };
            push_grid(
                cx.scene,
                grid_paths(ranges.get(), geometry, GridOptions::default()),
                Color::srgb_u8(140, 150, 170, 90),
            );
        });

    // The data is drawn once per data set, style or visibility change, in data
    // space. The ranges only move the view.
    let data = CanvasHandle::new();
    {
        let (data, markers) = (data.clone(), markers.clone());
        redraw_on_change(move || {
            let (wave, samples) = (wave.get(), samples.get());
            let marker = markers.of(shape.get());
            let lod = if reduced.get() {
                Lod::Columns
            } else {
                Lod::Exact
            };
            let (wave_alpha, samples_alpha) = (alpha(Series::Wave), alpha(Series::Samples));
            let (show_wave, show_samples) = (shown(Series::Wave), shown(Series::Samples));
            data.draw(|scene| {
                scene.clear();
                if show_wave {
                    let color = Series::Wave.color(wave_alpha);
                    push_line(scene, wave.data.clone(), color, 1.5, lod);
                }
                if show_samples {
                    let color = Series::Samples.color(samples_alpha);
                    push_points(scene, samples.data.clone(), marker, color);
                }
            });
        });
    }
    {
        let (data, meter) = (data.clone(), meter.clone());
        redraw_on_change(move || {
            let Some(geometry) = geometry.get() else {
                return;
            };
            show_ranges(&data, geometry, ranges.get());
            meter.kick();
        });
    }
    {
        let meter = meter.clone();
        redraw_on_change(move || {
            if sweeping.get() {
                meter.kick();
            }
        });
    }
    let data_layer = zgui::elements::canvas().class("plot-layer").scene(&data);

    let overlay = zgui::elements::canvas()
        .class("plot-layer")
        .draw(move |cx| {
            let Some(geometry) = geometry.get() else {
                return;
            };
            if let Some(target) = hover.get() {
                let path = hover_crosshair(target, ranges.get(), geometry, 5.0);
                push_stroke(cx.scene, path, Color::srgb_u8(232, 236, 244, 140), 1.0);
            }
            if let Some(selection) = pointer.selection() {
                push_selection(
                    cx.scene,
                    selection,
                    Color::srgb_u8(110, 168, 255, 40),
                    Color::srgb_u8(110, 168, 255, 200),
                );
            }
        });

    let on_hover = UnsyncCallback::new(move |event: PlotHover| {
        let PlotHover::At(position) = event else {
            hover.set(None);
            return;
        };
        let Some(geometry) = geometry.get_untracked() else {
            return;
        };
        let ranges = ranges.get_untracked();
        let cursor = PlotPoint {
            x: geometry.x_value(position.0, ranges.x),
            y: geometry.y_value(position.1, ranges.y),
        };
        let pixels = (geometry.data_width(), geometry.data_height());
        let snapped = (!hidden.with_untracked(|hidden| hidden.contains(&Series::Samples)))
            .then(|| {
                samples.with_untracked(|samples| {
                    nearest_sorted(&samples.points, cursor, ranges, pixels, 12.0)
                        .map(|index| HoverTarget::Point(samples.points[index]))
                })
            })
            .flatten();
        hover.set(Some(snapped.unwrap_or(HoverTarget::Cursor(position))));
    });

    let readout = move || {
        let point = match hover.get()? {
            HoverTarget::Point(point) => point,
            HoverTarget::Cursor((x, y)) => {
                let geometry = geometry.get()?;
                let ranges = ranges.get();
                PlotPoint {
                    x: geometry.x_value(x, ranges.x),
                    y: geometry.y_value(y, ranges.y),
                }
            }
        };
        Some(format!(
            "x {}   y {}",
            format_number(point.x),
            format_number(point.y)
        ))
    };
    let stats = move || {
        let (wave, samples) = (wave.get(), samples.get());
        let built = (wave.built + samples.built).as_secs_f64() * 1000.0;
        let rate = rate.get().map_or_else(
            || "idle".to_owned(),
            |rate| format!("{rate:.0} fps ({:.1} ms)", 1000.0 / rate),
        );
        format!(
            "{} line points · {} markers · data built in {built:.0} ms · {rate}",
            count_label(wave.len()),
            count_label(samples.len()),
        )
    };

    let legend = Series::ALL.map(|series| {
        view! {
            LegendItem(
                label = series.label().to_owned(),
                appearance = {zgui_plot::zgui_legend::LegendAppearance {color: series.css(),marker:series.marker(),color_signal:None}},
                visible = Signal::derive_local(move || shown(series)),
                on_toggle = UnsyncCallback::new(move |()| hidden.update(|hidden| toggle_member(hidden, &series))),
                key = series,
                hovered = emphasised
            )
        }
    });
    let [wave_entry, samples_entry] = legend;

    let line_choices = choices(
        line_size,
        LINE_SIZES.iter().map(|&n| (n, count_label(n))).collect(),
    );
    let point_choices = choices(
        point_size,
        POINT_SIZES.iter().map(|&n| (n, count_label(n))).collect(),
    );
    let shape_choices = choices(
        shape,
        Shape::ALL
            .iter()
            .map(|&s| (s, s.label().to_owned()))
            .collect(),
    );
    let lod_choices = choices(
        reduced,
        vec![(false, "exact".to_owned()), (true, "per column".to_owned())],
    );
    let on_update = UnsyncCallback::new(move |update: RangeUpdate| {
        sweeping.set(false);
        ranges.update(|current| *current = update.apply(*current));
    });

    view! {
        column(class = "app") {
            row(class = "header") {
                label(class = "title") {"Waves"}
                spacer()
                text(class = "readout") {{move || readout().unwrap_or_default()}}
            }
            row(class = "toolbar") {
                label(class = "group") {"line"}
                {line_choices}
                label(class = "group") {"detail"}
                {lod_choices}
                label(class = "group") {"markers"}
                {point_choices}
                {shape_choices}
                spacer()
                control(
                    class = "choice sweep",
                    class:choice-selected = move || sweeping.get(),
                    tabindex = Focus::Sequential,
                    on:click = move |_| sweeping.update(|on| *on = !*on)
                ) {
                    {move || if sweeping.get() { "stop sweep" } else { "sweep" }}
                }
            }
            PlotSurface(node_ref=surface,class="plot",label="Waves plot".to_owned(),pointer=pointer,
                interaction={zgui_plot::zgui_surface::SurfaceInteraction {
                    geometry: UnsyncCallback::new(move |()| geometry.get_untracked()),
                    ranges: UnsyncCallback::new(move |()| ranges.get_untracked()),
                    on_update, on_reset: UnsyncCallback::new(move |()| { sweeping.set(false); ranges.set(full_view()); }), on_hover
                }},
                options={zgui_plot::zgui_surface::SurfaceOptions {
                    on_cancel: Some(UnsyncCallback::new(move |initial| ranges.set(initial))), ticks, background:"#191d26".to_owned(),..Default::default()
                }}) {
                {grid.into_view()}
                box(class = "plot-clip") {
                    {data_layer.into_view()}
                }
                {overlay.into_view()}
                column(class = "legend") {
                    {wave_entry}
                    {samples_entry}
                }
            }
            row(class = "footer") {
                text(class = "stats") {{stats}}
                spacer()
                label(class = "hint") {
                    "drag: pan · wheel: zoom · edges: stretch axis · shift-drag: box zoom · double-click: reset"
                }
            }
        }
    }
}

const SHEET: &str = css!(
    ":root {
        background-color: #12141a;
        color: #e8ecf4;
        font-family: sans-serif;
    }

    .app { width: 100%; height: 100%; padding: 16px 20px; gap: 10px; box-sizing: border-box; }
    .header { align-items: baseline; }
    .title { font-size: 15px; letter-spacing: 1px; color: #c4cbd9; }
    .readout { font-size: 12px; color: #8a93a6; }
    .footer { align-items: baseline; gap: 12px; }
    .stats { font-size: 12px; color: #a7b0c2; }
    .hint { font-size: 11px; color: #5d667a; }

    .toolbar { align-items: center; gap: 8px; }
    .group { font-size: 11px; letter-spacing: 1px; color: #5d667a; margin-left: 6px; }
    .choices { border: 1px solid #262b36; border-radius: 6px; overflow: hidden; }
    .choice {
        font-size: 12px;
        padding: 3px 9px;
        color: #8a93a6;
        background-color: #191d26;
        cursor: pointer;
    }
    .choice:hover { color: #e8ecf4; }
    .choice-selected { color: #e8ecf4; background-color: #2a3346; }
    .sweep { border: 1px solid #262b36; border-radius: 6px; }

    .plot { flex-grow: 1; min-height: 0; border-radius: 10px; border: 1px solid #262b36; }
    .plot-layer { position: absolute; left: 0; top: 0; width: 100%; height: 100%; }
    .plot-clip { position: absolute; left: 0; top: 0; right: 0; bottom: 0; overflow: hidden; }

    .plot-tick-x { width: 64px; }
    .plot-tick-y { width: 56px; }

    .legend {
        position: absolute;
        top: 10px;
        right: 12px;
        z-index: 1;
        gap: 2px;
        padding: 6px 10px;
        border-radius: 8px;
        background-color: #12141acc;
    }"
);

fn stylesheet() -> String {
    let tick_text = TextStyle {
        color: "#f4f7f5".to_owned(),
        font_size: 13.0,
        font_weight: 600,
        line_height: LAYOUT.x_label.1,
        baseline: 13.0,
        outline: Some("#12181c".to_owned()),
        outline_width: 2.0,
    };
    let tick_css = format!(".plot-tick {{ {} }}", tick_text.css());
    let entry = LegendEntryStyle {
        font_size: 12.0,
        font_weight: 400,
        line_height: 18.0,
        hidden_opacity: 0.35,
    };
    let legend = LegendTheme {
        dot: entry,
        line: entry,
    };
    [
        SHEET,
        zgui_plot::zgui_surface::stylesheet(),
        &tick_css,
        &zgui_plot::zgui_legend::stylesheet(&legend),
    ]
    .concat()
}

fn main() -> Result<(), zgui::Error> {
    app()
        .with_title("Waves")
        .with_application_id("dev.zgui.plot.Waves")
        .with_size(1080.0, 640.0)
        .with_stylesheet(stylesheet())
        .run(|| {
            let view = view! { Waves() };
            // Renderer setup happens before view construction and can outlast polling.
            #[cfg(windows)]
            support::force_first_paint("Waves");
            view
        })
}
