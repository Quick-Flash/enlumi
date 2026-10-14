use crate::{
    legend::PlotLegendProps,
    plot_lines::PlotLinesProps,
    state::{Pane, Workbench},
};
use plot::{PlotGeometry, PlotPoint, zgui_time_series::TimeSeriesPlotProps};
use zgui::{
    prelude::*,
    reactive::{RenderEffect, UnsyncCallback},
};

#[component]
pub fn CursorPlot(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
    linked: RwSignal<Option<(u64, PlotPoint)>, LocalStorage>,
) -> impl IntoView {
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    let geometry = Memo::new(move |_| {
        let size = bounds.get()?.size;
        let (w, h) = (
            f64::from(size.width.0 / host.scale()),
            f64::from(size.height.0 / host.scale()),
        );
        PlotGeometry::with_bounds(0.0, 0.0, w, h, w, h)
    });
    let legend = NodeRef::new();
    let legend_bounds = legend.observe_border_box();
    let readout_top = move || {
        let legend = legend_bounds.get()?;
        let surface = bounds.get()?;
        Some(format!(
            "{}px",
            (legend.origin.y.0 + legend.size.height.0 - surface.origin.y.0) / host.scale() + 6.0
        ))
    };
    let position = RwSignal::new_local(None::<(f64, f64)>);
    let dragging = RwSignal::new_local(false);
    let local = Memo::new(move |_| {
        if !visible.get() {
            return None;
        }
        let (x, y) = position.get()?;
        let geometry = geometry.get()?;
        let ranges = pane.plot.ranges.get();
        if !(geometry.left..=geometry.right).contains(&x)
            || !(geometry.top..=geometry.bottom).contains(&y)
        {
            return None;
        }
        Some(pane.plot.hover.get().map_or(
            PlotPoint {
                x: geometry.x_value(x, ranges.x),
                y: geometry.y_value(y, ranges.y),
            },
            |hit| hit.point,
        ))
    });
    let clear = move || {
        if linked.get_untracked().is_some_and(|(id, _)| id == pane.id) {
            linked.set(None);
        }
    };
    let publish = RenderEffect::new(move |_| {
        let point = local.get();
        if pane.links.get().pointer
            && let Some(point) = point
        {
            let next = Some((pane.id, point));
            if linked.get_untracked() != next {
                linked.set(next);
            }
        } else {
            clear();
        }
    });
    on_cleanup_local(move || {
        drop(publish);
        clear();
    });
    let peer = Memo::new(move |_| {
        if !visible.get() || !pane.links.get().pointer {
            return None;
        }
        linked
            .get()
            .filter(|(id, _)| *id != pane.id)
            .map(|(_, point)| point)
    });
    let linked_canvas = zgui::elements::canvas()
        .class("snapped-linked-pointer")
        .draw(move |cx| {
            let Some(point) = peer.get() else {
                return;
            };
            let Some(geometry) = geometry.get() else {
                return;
            };
            let path =
                plot::zgui_link::linked_pointer_path(point, pane.plot.ranges.get(), geometry);
            plot::zgui_renderer::push_stroke(
                cx.scene,
                path.clone(),
                zgui::canvas::zgui_color::Color::srgb_u8(0, 0, 0, 210),
                3.0,
            );
            plot::zgui_renderer::push_stroke(
                cx.scene,
                path,
                zgui::canvas::zgui_color::Color::srgb_u8(244, 247, 245, 255),
                1.0,
            );
        });
    let readout = move || {
        local
            .get()
            .or_else(|| peer.get())
            .map_or_else(String::new, |point| {
                format!("X: {:.6}s | Y: {:.6}", point.x, point.y)
            })
    };
    view! {
        column(class = "cursor-plot", node_ref = host,
            on:pointer_move = move |event| {
                if !dragging.get_untracked() { position.set(plot::zgui_renderer::local_position(&host, event.position)); }
            },
            on:pointer_down:capture = move |_| { dragging.set(true); position.set(None); },
            on:pointer_up = move |_| dragging.set(false),
            on:pointer_cancel = move |_| { dragging.set(false); position.set(None); },
            on:pointer_leave = move |_| position.set(None),
            on:wheel:capture = move |_| position.set(None)) {
            TimeSeriesPlot(series = pane.series, state = pane.plot, visible = visible, draw_series = Signal::derive_local(|| false),
                title = Signal::derive_local(String::new),
                format = UnsyncCallback::new(move |(axis, value)| if axis == plot::labels::Axis::X { format!("{value:.2}s") } else { plot::format_number(value) })) {}
            PlotLines(workbench = workbench, pane = pane, geometry = geometry, visible = visible) {}
            {linked_canvas.into_view()}
            PlotLegend(workbench = workbench, pane = pane, node_ref = legend)
            if move || !pane.lines.get().is_empty() {
            text(class = "snapped-cursor-values", style:top = readout_top) {{readout}}
            }
        }
    }
}
