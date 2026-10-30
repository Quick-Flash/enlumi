use super::{LegendHost, SeriesPane as Pane, legend::PlotLegendProps};
use crate::{PlotGeometry, PlotPoint, zgui_time_series::TimeSeriesPlotProps};
use zgui::{
    prelude::*,
    reactive::{RenderEffect, UnsyncCallback},
};

/// Related SeriesPlot settings.
#[derive(Clone)]
pub struct SeriesPlotOptions {
    /// Lock connection style and omit connection controls from line settings.
    pub fixed_connection: Option<crate::line_draw::Connection>,
    pub snapshot: Option<crate::zgui_svg::PlotSnapshotSource>,
    pub format: Option<UnsyncCallback<(crate::labels::Axis, f64), String>>,
    pub cursor_format: Option<UnsyncCallback<(f64, f64), String>>,
    /// Content drawn below grid and series.
    pub background_layer: Option<ChildrenFn>,
    /// Content drawn above series and below legend.
    pub overlay: Option<ChildrenFn>,
    pub legend_extra_visible: Signal<bool, LocalStorage>,
    /// Replace default legend rows with caller-provided children.
    pub custom_legend: bool,
}
impl Default for SeriesPlotOptions {
    fn default() -> Self {
        Self {
            fixed_connection: None,
            snapshot: None,
            format: None,
            cursor_format: None,
            background_layer: None,
            overlay: None,
            legend_extra_visible: Signal::derive_local(|| false),
            custom_legend: false,
        }
    }
}

#[component]
pub fn SeriesPlot(
    host_actions: LegendHost,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
    linked: RwSignal<Option<(u64, PlotPoint)>, LocalStorage>,
    #[prop(optional)] children: Option<ChildrenFn>,
    #[prop(attrs)] attrs: Attrs,
    #[prop(default = SeriesPlotOptions::default())] options: SeriesPlotOptions,
) -> impl IntoView {
    let SeriesPlotOptions {
        fixed_connection,
        snapshot,
        format,
        cursor_format,
        background_layer,
        overlay,
        legend_extra_visible,
        custom_legend,
    } = options;

    super::install();
    crate::zgui_svg::SnapshotLayers::current_or_new();
    let settings_open = RwSignal::new_local(false);
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
                crate::zgui_link::linked_pointer_path(point, pane.plot.ranges.get(), geometry);
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
    let readout = move || {
        local
            .get()
            .or_else(|| peer.get())
            .map_or_else(String::new, |point| {
                cursor_format.map_or_else(
                    || format!("X: {:.6} | Y: {:.6}", point.x, point.y),
                    |format| format.run((point.x, point.y)),
                )
            })
    };
    let drawing = Signal::derive_local(move || {
        pane.lines
            .get()
            .into_iter()
            .map(|line| {
                let style = pane
                    .drawing
                    .with(|drawing| drawing.get(&line.id).copied().unwrap_or_default());
                (line.id, style)
            })
            .map(|(id, mut style)| {
                if let Some(connection) = fixed_connection {
                    style.connection = connection;
                }
                (id.to_string(), style)
            })
            .collect()
    });
    let legend_options = crate::zgui_series::legend::LegendOptions {
        fixed_connection,
        custom_entries: custom_legend,
        settings_open,
        extra_visible: legend_extra_visible,
    };
    let plot_options = crate::zgui_plot::PlotOptions {
        visible,
        line_drawing: drawing,
        show_legend: false,
        show_pointer: false,
        show_cursor_values: false,
        on_line_settings: (UnsyncCallback::new(move |()| settings_open.set(true))).into(),
        line_settings_enabled: Signal::derive_local(move || !pane.lines.with(Vec::is_empty)),
        title: Signal::derive_local(String::new),
        format,
        snapshot,
        ..Default::default()
    };
    view! {
            column(class = "cursor-plot", node_ref = host, {..attrs},
                on:pointer_move = move |event| {
                    if !dragging.get_untracked() { position.set(crate::zgui_renderer::local_position(&host, event.position)); }
                },
                on:pointer_down:capture = move |_| { dragging.set(true); position.set(None); },
                on:pointer_up = move |_| dragging.set(false),
                on:pointer_cancel = move |_| { dragging.set(false); position.set(None); },
                on:pointer_leave = move |_| position.set(None),
                on:wheel:capture = move |_| position.set(None)) {
                TimeSeriesPlot(series=pane.series,state=pane.plot,options=plot_options,
                    background_layer=move || AnyView::new(background_layer.as_ref().map(|layer| layer.view()))) {}
                {overlay.map(|layer| layer.view())}
                {linked_canvas.into_view()}
                PlotLegend(host = host_actions,
    pane = pane,
    node_ref = legend,
    children = move || AnyView::new(children.as_ref().map(|children| children.view())),
    options = legend_options)
                if move || !pane.lines.get().is_empty() {
                text(class = "snapped-cursor-values", style:top = readout_top) {{readout}}
                }
            }
        }
}
