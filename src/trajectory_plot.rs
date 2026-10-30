use crate::{
    action_button::ActionButtonProps,
    plot_appearance::PlotBackgroundProps,
    plot_menu::PlotMenuEditorProps,
    state::{Pane, Workbench},
    trajectory::{Spec, Tracks},
    trajectory_axes::SwapAxesProps,
};
use plot::{
    PlotGeometry,
    plot_data::{PlotData, ScatterSet},
    zgui_plot::{DataPlotProps, PlotOptions},
    zgui_series::LegendEntryProps,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zgui::{canvas::zgui_color::Color, prelude::*};
use zgui_ui::prelude::*;

#[cfg(test)]
#[derive(Clone)]
struct PreparationHook(Arc<PreparationFn>);
#[cfg(test)]
type PreparationFn = dyn Fn(&[crate::state::Line], &Spec) -> Result<Tracks, String> + Send + Sync;

#[cfg(test)]
#[derive(Clone)]
struct OverlayProbe(std::rc::Rc<std::cell::RefCell<Option<OverlayHandles>>>);
#[cfg(test)]
struct OverlayHandles {
    canvas: zgui::elements::canvas::CanvasHandle,
    paths: RwSignal<crate::trajectory_render::Paths, LocalStorage>,
}

#[cfg(test)]
#[path = "trajectory_plot_tests.rs"]
mod preparation_tests;

impl Workbench {
    pub fn new_trajectory(self, gps: bool) {
        let original = self.pane();
        let mut spec = Spec::default();
        if gps {
            let ids = self.log.with_untracked(|log| {
                log.as_ref()
                    .map(|log| {
                        log.catalogue
                            .iter()
                            .map(|signal| signal.id.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            });
            let Some(gps) = crate::trajectory::gps_pair(&ids) else {
                self.status.set("No supported GPS longitude/latitude pair. Add a 2D plot and drag coordinate fields into its X/Y slots.".into());
                return;
            };
            spec = gps;
        }
        let builder = self.trajectory_builder();
        if gps {
            builder.pane.trajectory.set(Some(spec));
        }
        builder.pane.lines.set(original.lines.get_untracked());
        if gps {
            builder.target.set(None);
        }
        builder.open.set(true);
        self.analysis().open.set(false);
        self.filters_open.set(false);
    }
    pub fn convert_to_trajectory(self, pane: Pane) {
        if pane.analysis.with_untracked(Option::is_some) {
            return;
        }
        let mut spec = pane.trajectory.get_untracked().unwrap_or_default();
        if pane.trajectory.with_untracked(Option::is_none) {
            for line in pane.lines.get_untracked() {
                spec.add_source(&line.source);
            }
        }
        self.edit_trajectory(pane);
        self.trajectory_builder().pane.trajectory.set(Some(spec));
    }
}

#[component]
pub fn TrajectoryPlot(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
    group: plot::zgui_link::LinkGroup,
) -> impl IntoView {
    install_stylesheet("enlumi-trajectory", include_str!("trajectory.css"));
    install_stylesheet("plot-series-controls", plot::zgui_series::stylesheet());
    plot::zgui_svg::SnapshotLayers::current_or_new();
    provide_local_context(plot::zgui_link::PlotLinks::with_options(
        pane.id, group, pane.links,
    ));
    let message = RwSignal::new_local(String::new());
    let busy = RwSignal::new_local(false);
    let revision = RwSignal::new_local(0_u64);
    let running = RwSignal::new_local(false);
    let pending = StoredValue::new_local(None);
    let cancel = StoredValue::new_local(None::<Arc<AtomicBool>>);
    #[cfg(test)]
    let preparation_hook = use_local_context::<PreparationHook>();
    let inputs = Memo::new(move |_| pane.trajectory.get().map(|spec| spec.pairs()));
    let content = Memo::new(move |_| {
        let mut keys = pane.lines.with(|lines| {
            lines
                .iter()
                .map(|line| {
                    (
                        line.id,
                        line.source.clone(),
                        line.name.clone(),
                        line.revision,
                        line.data.samples().as_ptr() as usize,
                        line.data.samples().len(),
                    )
                })
                .collect::<Vec<_>>()
        });
        keys.sort_by_key(|line| line.0);
        keys
    });
    let request = Memo::new(move |_| visible.get().then(|| (inputs.get(), content.get())));
    let prepare = zgui::reactive::RenderEffect::new(move |_| {
        let request = request.get();
        // Cancel work already running or waiting for a pool thread before
        // replacing the single pending snapshot.
        cancel.update_value(|held| {
            if let Some(held) = held.take() {
                held.store(true, Ordering::Relaxed);
            }
        });
        if request.is_none() {
            revision.update(|value| *value = value.wrapping_add(1));
            pending.set_value(None);
            busy.set(false);
            return;
        }
        let lines = pane.lines.get_untracked();
        let Some(spec) = pane.trajectory.get_untracked() else {
            return;
        };
        if let Some(warnings) = pane.trajectory_data.with_untracked(|data| {
            data.as_ref()
                .filter(|data| data.matches(&lines, &spec))
                .map(|data| data.warnings.join("; "))
        }) {
            // Returning to the displayed inputs supersedes any queued/in-flight
            // replacement, even though no new work is needed.
            revision.update(|value| *value = value.wrapping_add(1));
            pending.set_value(None);
            busy.set(false);
            message.set(warnings);
            return;
        }
        let next = revision.get_untracked().wrapping_add(1);
        revision.set(next);
        pane.plot.hover.set(None);
        busy.set(true);
        message.set(String::new());
        let cancelled = Arc::new(AtomicBool::new(false));
        cancel.set_value(Some(cancelled.clone()));
        pending.set_value(Some((next, lines, spec, cancelled)));
        if running.get_untracked() {
            return;
        }
        running.set(true);
        #[cfg(test)]
        let preparation_hook = preparation_hook.clone();
        workbench.task(async move {
            loop {
                let mut job = None;
                pending.update_value(|pending| job = pending.take());
                let Some((next, lines, spec, cancelled)) = job else {
                    running.set(false);
                    return;
                };
                #[cfg(test)]
                let hook = preparation_hook.clone();
                let result = plot::preparation::prepare("xy-alignment", move || {
                    #[cfg(test)]
                    if let Some(hook) = hook {
                        return (hook.0)(&lines, &spec).map(Arc::new);
                    }
                    Tracks::prepare_cancelled(&lines, &spec, &|| cancelled.load(Ordering::Relaxed))
                        .map(Arc::new)
                })
                .await;
                if running.try_get_untracked().is_none() || workbench.find(pane.id).is_none() {
                    return;
                }
                if revision.get_untracked() != next || !visible.get_untracked() {
                    continue;
                }
                busy.set(false);
                match result {
                    Ok(data) => {
                        let ranges = PlotData::Scatter(data.scatter.clone())
                            .fitted_ranges(&pane.plot.hidden.get_untracked(), 32);
                        let _ = pane.plot.set_view(ranges);
                        message.set(data.warnings.join("; "));
                        pane.trajectory_data.set(Some(data));
                    }
                    Err(error) => {
                        // Retain geometry while work is pending, but do not
                        // present the old axes as a successful invalid setup.
                        pane.trajectory_data.set(None);
                        message.set(error);
                    }
                }
            }
        });
    });
    on_cleanup_local(move || {
        cancel.update_value(|held| {
            if let Some(held) = held.take() {
                held.store(true, Ordering::Relaxed);
            }
        });
        drop(prepare);
    });
    // Import legacy plot-wide appearance into the normal per-line controls once inputs exist.
    let migrate = zgui::reactive::RenderEffect::new(move |_| {
        let Some(spec) = pane.trajectory.get() else {
            return;
        };
        let lines = pane.view_lines.get();
        if lines.is_empty() || (spec.connected && !spec.points) {
            return;
        }
        pane.drawing.update(|drawing| {
            for line in lines {
                let draw = drawing.entry(line.id).or_default();
                draw.markers = spec.points;
                if !spec.connected {
                    draw.pattern = plot::line_draw::LinePattern::None;
                }
            }
        });
        pane.trajectory.update(|spec| {
            if let Some(spec) = spec {
                spec.connected = true;
                spec.points = false;
            }
        });
    });
    on_cleanup_local(move || drop(migrate));
    let hover = zgui::reactive::RenderEffect::new(move |_| {
        if !visible.get() || !pane.timeline_link.get() {
            return;
        }
        if let Some(time) = hovered_time(pane) {
            workbench.timeline.hover(time);
        }
    });
    on_cleanup_local(move || drop(hover));
    let series = Signal::derive_local(move || {
        PlotData::Scatter(pane.trajectory_data.with(|data| {
            data.as_ref()
                .map_or_else(ScatterSet::default, |data| data.scatter.clone())
        }))
    });
    let chart_visible = Signal::derive_local(move || visible.get());
    let options = PlotOptions {
        visible: chart_visible,
        snapshot: Some(pane.snapshot),
        show_legend: false,
        draw_series: Signal::stored_local(false),
        lock_selection_aspect: Signal::derive_local(move || {
            pane.trajectory
                .with(|spec| spec.as_ref().is_some_and(|spec| spec.equal))
        }),
        ..Default::default()
    };
    let area = NodeRef::new();
    pane.menu_area.set(Some(area));
    on_cleanup_local(move || {
        if pane.menu_area.get_untracked() == Some(area) {
            pane.menu_area.set(None);
        }
    });
    let press = StoredValue::new_local(None::<(f32, f32)>);
    view! {column(class="trajectory-pane") {
        column(class="trajectory-chart",node_ref=area,on:pointer_down:capture=move |event| {if event.button==Some(zgui::vocab::PointerButton::Primary){press.set_value(Some((event.position.x.0,event.position.y.0)));}},on:pointer_up:capture=move |event| {
            if pane.timeline_link.get_untracked() && let Some((x,y))=press.get_value() && (event.position.x.0-x).hypot(event.position.y.0-y)<4. && let Some(time)=hovered_time(pane) {let _=workbench.timeline.pin(time);}press.set_value(None);
        },on:pointer_cancel=move |_|press.set_value(None)) {
            DataPlot(series=series,state=pane.plot,options=options,background_layer=move ||AnyView::new(view!{PlotBackground(background=pane.background,visible=visible)}))
            TrajectoryOverlay(workbench=workbench,pane=pane,visible=chart_visible)
            column(class="trajectory-plot-controls"){
                TrajectoryLegend(workbench=workbench,pane=pane)
                row(class="trajectory-plot-tools"){
                    ActionButton(class="trajectory-equal-scale",class:setting-active=move ||pane.trajectory.with(|spec|spec.as_ref().is_some_and(|spec|spec.equal)),on:pointer_down=move |event|{press.set_value(None);event.stop_propagation();},on:click=move |_|pane.trajectory.update(|spec|if let Some(spec)=spec{spec.equal= !spec.equal;})){"Equal scale"}
                }
            }
            if move ||busy.get() || !message.get().is_empty() {
                text(class="trajectory-status muted",a11y:role=Role::Status) {{move ||if busy.get(){"Preparing 2D traces...".into()}else{message.get()}}}
            }
        }
    }}
}

fn hovered_time(pane: Pane) -> Option<f64> {
    let hit = pane.plot.hover.get()?;
    pane.trajectory_data.with(|data| {
        data.as_ref()?
            .traces
            .iter()
            .find(|trace| trace.id.to_string() == hit.series_id)?
            .track
            .times
            .get(hit.sample_index)
            .copied()
    })
}

#[component]
fn TrajectoryLegend(workbench: Workbench, pane: Pane) -> impl IntoView {
    let collapsed = RwSignal::new_local(false);
    let settings = RwSignal::new_local(false);
    plot::zgui_svg::register_description(
        plot::zgui_svg::SnapshotLayer::Foreground,
        move |description| {
            let entries = pane
                .view_lines
                .get_untracked()
                .iter()
                .map(|line| {
                    plot::zgui_plot::theme::LEGEND.entry(
                        plot::LegendMarker::Line,
                        &line.name,
                        pane.plot.style(&line.id.to_string()).css_color(),
                        !pane
                            .plot
                            .hidden
                            .with_untracked(|hidden| hidden.contains(&line.id.to_string())),
                    )
                })
                .collect();
            description.push_legend(
                entries,
                (
                    description.width - plot::zgui_plot::theme::LEGEND_RIGHT,
                    plot::zgui_plot::theme::LEGEND_TOP,
                ),
                &plot::zgui_plot::theme::legend_box(),
            );
        },
    );
    view! {column(class="workbench-legend trajectory-legend",on:pointer_down=move |event|event.stop_propagation(),on:pointer_up=move |event|event.stop_propagation()) {
        row(class="legend-heading") {
            Button(size=ButtonSize::Xs,variant=ButtonVariant::Ghost,class="legend-collapse",a11y:label="Collapse or expand legend",on:click=move |_|collapsed.update(|value|*value= !*value)) {
                vector(class="legend-chevron",prop:viewBox="0 0 12 12",prop:d=move ||PropValue::from(if collapsed.get(){"M4 2 L8 6 L4 10"}else{"M2 4 L6 8 L10 4"}),a11y:hidden=true)
                text {"Legend"}
            }
            PlotMenuEditor(workbench=workbench,pane=pane,open=settings,legend_trigger=true)
        }
        if move ||!collapsed.get() {
            for line in move ||{let keys=workbench.legend_keys(pane);let mut lines=pane.view_lines.get();lines.sort_by_key(|line|keys.iter().position(|id|*id==line.id).unwrap_or(usize::MAX));lines},key=|line|(line.id,line.name.clone()) {
                TrajectoryLegendRow(workbench=workbench,pane=pane,line=line)
            }
        }
    }}
}

#[component]
fn TrajectoryLegendRow(
    workbench: Workbench,
    pane: Pane,
    line: plot::zgui_series::SeriesLine,
) -> impl IntoView {
    let id = line.id;
    let index = Signal::derive_local(move || {
        pane.trajectory
            .with(|spec| {
                spec.as_ref().and_then(|spec| {
                    let source = pane.lines.with(|lines| {
                        lines
                            .iter()
                            .find(|input| input.id == id)
                            .map(|input| input.source.clone())
                    })?;
                    spec.pairs().iter().position(|pair| pair.y == source)
                })
            })
            .unwrap_or(usize::MAX)
    });
    view! {row(class="trajectory-legend-row"){
        LegendEntry(host=workbench.legend_host(),pane=pane.display(),line=line)
        SwapAxes(workbench=workbench,pane=pane,index=index)
    }}
}

#[component]
fn TrajectoryOverlay(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let host = NodeRef::new();
    let size = host.observe_border_box();
    let aspect = zgui::reactive::RenderEffect::new(move |_| {
        if !visible.get()
            || !pane
                .trajectory
                .with(|spec| spec.as_ref().is_some_and(|spec| spec.equal))
        {
            return;
        }
        let Some(size) = size.get() else {
            return;
        };
        let (w, h) = (
            f64::from(size.size.width.0 / host.scale()),
            f64::from(size.size.height.0 / host.scale()),
        );
        if w <= 0. || h <= 0. {
            return;
        }
        let ranges = pane.plot.ranges.get();
        let links = pane.links.get();
        let equal = match (links.x, links.y) {
            (true, true) => ranges,
            (true, false) => {
                let half = ranges.x.span() * h / w / 2.;
                let center = (ranges.y.min + ranges.y.max) / 2.;
                plot::Ranges {
                    x: ranges.x,
                    y: plot::Range::new(center - half, center + half),
                }
            }
            (false, true) => {
                let half = ranges.y.span() * w / h / 2.;
                let center = (ranges.x.min + ranges.x.max) / 2.;
                plot::Ranges {
                    x: plot::Range::new(center - half, center + half),
                    y: ranges.y,
                }
            }
            _ => crate::trajectory::equal_ranges(ranges, w, h),
        };
        if (ranges.x.span() - equal.x.span()).abs() + (ranges.y.span() - equal.y.span()).abs()
            > 1e-9 * (ranges.x.span() + ranges.y.span())
        {
            let _ = pane.plot.set_view(equal);
        }
    });
    on_cleanup_local(move || drop(aspect));
    let geometry = Signal::derive_local(move || {
        let size = size.get()?;
        let (w, h) = (
            f64::from(size.size.width.0 / host.scale()),
            f64::from(size.size.height.0 / host.scale()),
        );
        PlotGeometry::with_bounds(0., 0., w, h, w, h)
    });
    let patterns = crate::trajectory_render::patterns(workbench, pane, geometry, visible);
    let handle = zgui::elements::canvas::CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Foreground);
    #[cfg(test)]
    if let Some(probe) = use_local_context::<OverlayProbe>() {
        *probe.0.borrow_mut() = Some(OverlayHandles {
            canvas: handle.clone(),
            paths: patterns,
        });
    }
    let laid_out = Memo::new(move |_| geometry.get().is_some());
    let viewport = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        if visible.get()
            && let Some(g) = geometry.get()
        {
            plot::zgui_renderer::show_ranges(&viewport, g, pane.plot.ranges.get());
        }
    });
    let drawing = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        drawing.draw(|scene| {
            scene.clear();
            if !visible.get() || !laid_out.get() {
                return;
            }
            let Some(data) = pane.trajectory_data.get() else {
                return;
            };
            let hidden = pane.plot.hidden.get();
            let emphasized = pane.plot.emphasized.get();
            let order = pane.legend_order.get();
            let mut traces = data.traces.iter().collect::<Vec<_>>();
            traces.sort_by_key(|trace| {
                (
                    emphasized.as_deref() == Some(trace.id.to_string().as_str()),
                    order
                        .iter()
                        .position(|id| *id == trace.id)
                        .unwrap_or(usize::MAX),
                )
            });
            for trace in traces {
                let key = trace.id.to_string();
                if hidden.contains(&key) && emphasized.as_deref() != Some(&key) {
                    continue;
                }
                let style = pane.plot.style(&key);
                let draw = pane
                    .drawing
                    .with(|draw| draw.get(&trace.id).copied().unwrap_or_default());
                let alpha = if emphasized.as_ref().is_some_and(|id| id != &key) {
                    0.3
                } else {
                    1.
                };
                let [r, gc, b, a] = style.color;
                let color = Color::srgb_u8(r, gc, b, (f64::from(a) * alpha) as u8);
                let transform =
                    kurbo::Affine::translate((trace.track.origin.x, trace.track.origin.y));
                let width = style.width
                    + if emphasized.as_deref() == Some(&key) {
                        1.5
                    } else {
                        0.
                    };
                patterns.with(|patterns| {
                    crate::trajectory_render::paint(
                        scene,
                        trace,
                        draw,
                        plot::plot_data::SeriesStyle { width, ..style },
                        color,
                        transform,
                        patterns,
                    )
                });
            }
        })
    });
    // Timeline motion only changes this small marker scene, never the retained paths.
    let markers = zgui::elements::canvas::CanvasHandle::new();
    plot::zgui_svg::register_canvas(&markers, plot::zgui_svg::SnapshotLayer::Foreground);
    let marker_drawing = markers.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        marker_drawing.draw(|scene| {
            scene.clear();
            if !visible.get() || !pane.timeline_link.get() {
                return;
            }
            let Some(g) = geometry.get() else {
                return;
            };
            let Some(data) = pane.trajectory_data.get() else {
                return;
            };
            let ranges = pane.plot.ranges.get();
            let hidden = pane.plot.hidden.get();
            let emphasized = pane.plot.emphasized.get();
            for trace in &data.traces {
                let key = trace.id.to_string();
                if hidden.contains(&key) && emphasized.as_deref() != Some(&key) {
                    continue;
                }
                let [r, gc, b, a] = pane.plot.style(&key).color;
                let color = Color::srgb_u8(r, gc, b, a);
                if let Some(point) = workbench
                    .timeline
                    .time
                    .get()
                    .and_then(|time| trace.track.at(time))
                    && ranges.x.contains(point.x)
                    && ranges.y.contains(point.y)
                {
                    let mut path = kurbo::BezPath::new();
                    plot::append_circle(
                        &mut path,
                        (g.x(point.x, ranges.x), g.y(point.y, ranges.y)),
                        6.,
                    );
                    plot::zgui_renderer::push_fill_stroke(
                        scene,
                        path,
                        Color::srgb_u8(255, 190, 60, 255),
                        color,
                        2.,
                    );
                }
            }
        })
    });
    let canvas = zgui::elements::canvas()
        .class("trajectory-canvas")
        .scene(&handle);
    let marker_canvas = zgui::elements::canvas()
        .class("trajectory-marker-canvas")
        .scene(&markers);
    view! {column(class="trajectory-overlay",node_ref=host) { {canvas.into_view()} {marker_canvas.into_view()} }}
}
