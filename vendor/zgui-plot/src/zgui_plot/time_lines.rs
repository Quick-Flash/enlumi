//! Background viewport preparation shared by all native time plots.
use super::time_cache::{Plan, Prepared, Viewports, compatible, detail_plan, reusable};
use crate::{
    PlotGeometry,
    line_draw::{LineDraw, LinePattern, prepare},
    plot_data::MarkerShape,
    series_samples::{Accuracy, pan_viewport},
    time_series::{SeriesData, TimeSeriesSet},
    zgui_plot::PlotState,
    zgui_renderer::*,
};
use std::collections::BTreeMap;
use zgui::{
    canvas::{Lod, Marker, zgui_color::Color},
    prelude::*,
    reactive::RenderEffect,
};

pub(super) fn time_lines(
    series: Signal<TimeSeriesSet, LocalStorage>,
    state: PlotState,
    drawing: Signal<BTreeMap<String, LineDraw>, LocalStorage>,
    geometry: Memo<Option<PlotGeometry>>,
    visible: Signal<bool, LocalStorage>,
    handle: CanvasHandle,
) {
    let prepared = RwSignal::new_local(Prepared::new());
    let generation = RwSignal::new_local(0_u64);
    let running = RwSignal::new_local(false);
    let cached_plan = StoredValue::new_local(None::<Plan>);
    let history = StoredValue::new_local(Viewports::default());
    let restore = move |request: &Plan| {
        if cached_plan.with_value(|cached| {
            cached
                .as_ref()
                .is_some_and(|cached| reusable(cached, request))
        }) {
            return true;
        }
        if let Some((plan, data)) = history.with_value(|history| history.find(request)) {
            cached_plan.set_value(Some(plan));
            prepared.set(data);
            return true;
        }
        false
    };
    let plan = Memo::new(move |_| {
        if !visible.get() {
            return None;
        }
        let geometry = geometry.get()?;
        let range = state.ranges.get().x;
        let hidden = state.hidden.get();
        let emphasized = state.emphasized.get();
        let drawing = drawing.get();
        let keys = series.with(|set| {
            set.series()
                .iter()
                .filter(|line| {
                    !hidden.contains(line.id()) || emphasized.as_deref() == Some(line.id())
                })
                .map(|line| {
                    (
                        line.id().to_owned(),
                        line.data().samples().as_ptr() as usize,
                        {
                            let draw = drawing.get(line.id()).copied().unwrap_or_default();
                            let width =
                                if matches!(draw.pattern, LinePattern::Solid | LinePattern::None) {
                                    0
                                } else {
                                    state.style(line.id()).width.to_bits()
                                };
                            (draw.connection, draw.pattern, width)
                        },
                    )
                })
                .collect::<Vec<_>>()
        });
        let patterned = keys.iter().any(|(_, _, (_, pattern, _))| {
            !matches!(pattern, LinePattern::Solid | LinePattern::None)
        });
        let y = state.ranges.get().y;
        let view = if patterned {
            (
                y.min.to_bits(),
                y.max.to_bits(),
                geometry.data_height().ceil().clamp(1.0, 8192.0) as usize,
            )
        } else {
            (0, 0, 0)
        };
        Some((
            keys,
            range.min.to_bits(),
            range.max.to_bits(),
            geometry.data_width().ceil().clamp(1.0, 8192.0) as usize,
            state.lod.get(),
            view,
        ))
    });
    let prepare_effect = RenderEffect::new(move |_| {
        let request = plan.get();
        generation.update(|value| *value += 1);
        if request.is_none() {
            cached_plan.set_value(None);
            history.update_value(Viewports::clear);
            prepared.set(vec![]);
        }
        if request.as_ref().is_some_and(restore) {
            return;
        }
        if running.get_untracked() {
            return;
        }
        running.set(true);
        spawn_local(async move {
            loop {
                let Some(version) = generation.try_get_untracked() else {
                    return;
                };
                let Some(request) = plan.get_untracked() else {
                    cached_plan.set_value(None);
                    history.update_value(Viewports::clear);
                    prepared.set(vec![]);
                    running.set(false);
                    return;
                };
                if restore(&request) {
                    running.set(false);
                    return;
                }
                let sampled = detail_plan(&request);
                let (keys, min, max, columns, lod, view) = sampled.clone();
                let old = cached_plan.get_value();
                let cached = prepared.get_untracked();
                let jobs = series.with_untracked(|set| {
                    keys.iter()
                        .filter_map(|(id, pointer, mode)| {
                            let reuse = old
                                .as_ref()
                                .is_some_and(|old| {
                                    old.1 == min
                                        && old.2 == max
                                        && old.3 == columns
                                        && old.4 == lod
                                        && old.5 == view
                                        && old.0.contains(&(id.clone(), *pointer, *mode))
                                })
                                .then(|| cached.iter().find(|line| &line.0 == id).cloned())
                                .flatten();
                            set.series()
                                .iter()
                                .find(|line| line.id() == id)
                                .map(|line| (id.clone(), line.data().clone(), *mode, reuse))
                        })
                        .collect::<Vec<_>>()
                });
                let x = crate::Range::new(f64::from_bits(min), f64::from_bits(max));
                let data: Prepared = crate::preparation::prepare("time-series", move || {
                    jobs.into_iter()
                        .map(|(id, data, mode, reuse)| {
                            if let Some(reuse) = reuse {
                                return reuse;
                            }
                            let samples = pan_viewport(
                                &data,
                                x,
                                columns,
                                if lod == Lod::Exact {
                                    Accuracy::Exact
                                } else {
                                    Accuracy::Columns
                                },
                            );
                            let (connection, pattern, width) = mode;
                            let steps = if pattern == LinePattern::None {
                                SeriesData::new(vec![]).unwrap()
                            } else {
                                prepare(&samples, connection)
                            };
                            let patterned =
                                if matches!(pattern, LinePattern::Solid | LinePattern::None) {
                                    steps
                                } else {
                                    crate::line_pattern::prepare(
                                        &steps,
                                        crate::Ranges::new(
                                            x,
                                            (f64::from_bits(view.0), f64::from_bits(view.1)),
                                        ),
                                        (columns as f64, view.2 as f64),
                                        pattern,
                                        f64::from_bits(width),
                                    )
                                };
                            (id, samples, patterned, connection)
                        })
                        .collect()
                })
                .await;
                let Some(current) = generation.try_get_untracked() else {
                    return;
                };
                if let Some(latest) = plan.get_untracked()
                    && compatible(&sampled, &latest)
                    && !restore(&latest)
                {
                    history.update_value(|history| history.remember(sampled.clone(), data.clone()));
                    cached_plan.set_value(Some(sampled));
                    prepared.set(data);
                }
                if current == version {
                    running.set(false);
                    return;
                }
            }
        });
    });
    on_cleanup_local(move || drop(prepare_effect));
    let draw = handle.clone();
    redraw_on_change(move || {
        let mut data = if visible.get() {
            prepared.get()
        } else {
            vec![]
        };
        let hidden = state.hidden.get();
        let present = series.with(|set| {
            set.series()
                .iter()
                .map(|line| line.id().to_owned())
                .collect::<Vec<_>>()
        });
        let settings = drawing.get();
        let emphasized = state.emphasized.get();
        // Paint hovered series last, including temporary previews of hidden lines.
        data.sort_by_key(|(id, ..)| emphasized.as_deref() == Some(&id.to_string()));
        let lod = state.lod.get();
        draw.draw(|scene| {
            scene.clear();
            for (id, original, line, _) in &data {
                let key = id.to_string();
                if (hidden.contains(&key) && emphasized.as_deref() != Some(&key))
                    || !present.contains(id)
                {
                    continue;
                }
                let mut style = state.style(&key);
                let alpha = if emphasized.as_ref().is_some_and(|hover| hover != &key) {
                    0.3
                } else {
                    1.0
                };
                if emphasized.as_deref() == Some(&key) {
                    style.width += 1.5;
                }
                let [r, g, b, a] = style.color;
                let color = Color::srgb_u8(r, g, b, (f64::from(a) * alpha) as u8);
                if settings.get(id).copied().unwrap_or_default().pattern != LinePattern::None
                    && line.samples().len() >= 2
                {
                    push_prepared_line(
                        scene,
                        line,
                        color,
                        style.width.max(0.5),
                        if settings.get(id).copied().unwrap_or_default().pattern
                            == LinePattern::Solid
                        {
                            lod
                        } else {
                            Lod::Exact
                        },
                        kurbo::Affine::IDENTITY,
                    );
                }
                if settings.get(id).copied().unwrap_or_default().markers
                    || original.samples().len() == 1
                {
                    let radius = style.radius.clamp(0.5, 20.0);
                    let marker = match style.marker {
                        MarkerShape::Circle => Marker::Circle { radius },
                        MarkerShape::Square => Marker::Square { half: radius },
                        MarkerShape::Triangle => {
                            Marker::Path(std::sync::Arc::new(crate::triangle_marker(radius)))
                        }
                        MarkerShape::Diamond => {
                            Marker::Path(std::sync::Arc::new(crate::diamond_marker(radius)))
                        }
                        MarkerShape::Cross => Marker::Path(std::sync::Arc::new(
                            crate::cross_marker(radius, radius / 3.0),
                        )),
                    };
                    let origin = original.origin();
                    push_points_transformed(
                        scene,
                        original.vertices(),
                        marker,
                        color,
                        kurbo::Affine::translate((origin.x, origin.y)),
                    );
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_draw::Connection;
    #[test]
    fn pan_keeps_retained_geometry_and_hidden_plots_rebuild_when_shown() {
        use std::{
            cell::RefCell,
            rc::Rc,
            time::{Duration, Instant},
        };
        use zgui::render::Renderer;
        let built = Rc::new(RefCell::new(None));
        let model = Rc::clone(&built);
        let handler = app()
            .with_size(900., 600.)
            .with_renderer(Box::new(|_, target| {
                let mut renderer = zgui_testkit_scene::CaptureRenderer::new();
                renderer.configure(target);
                Ok(Box::new(renderer))
            }))
            .into_handler(move || {
                let state = PlotState::default();
                state
                    .ranges
                    .set(crate::Ranges::new((100., 200.), (-1., 1.)));
                let visible = RwSignal::new_local(true);
                let geometry =
                    Memo::new(move |_| PlotGeometry::with_bounds(0., 0., 800., 500., 800., 500.));
                let data = SeriesData::new(
                    (0..1_000_000)
                        .map(|i| crate::PlotPoint {
                            x: i as f64 / 1000.,
                            y: (i as f64 / 50.).sin(),
                        })
                        .collect::<Vec<_>>(),
                )
                .unwrap();
                let set =
                    TimeSeriesSet::new(vec![crate::time_series::TimeSeries::new("a", "a", data)])
                        .unwrap();
                let handle = CanvasHandle::new();
                let drawing = RwSignal::new_local(BTreeMap::new());
                time_lines(
                    Signal::stored_local(set),
                    state,
                    Signal::derive_local(move || drawing.get()),
                    geometry,
                    visible.into(),
                    handle.clone(),
                );
                let view_handle = handle.clone();
                redraw_on_change(move || {
                    show_ranges(&view_handle, geometry.get().unwrap(), state.ranges.get())
                });
                let initial = handle.scene().revision();
                *model.borrow_mut() = Some((state, visible, handle.clone(), initial, drawing));
                zgui::elements::canvas().scene(&handle).into_view()
            })
            .unwrap();
        let mut harness = zgui_platform_headless::Harness::new(handler.into_runtime());
        harness.settle(16);
        let (state, visible, handle, initial, drawing) = built.borrow().as_ref().unwrap().clone();
        let wait = |harness: &mut zgui_platform_headless::Harness<zgui::runtime::Runtime>,
                    revision| {
            let start = Instant::now();
            while handle.scene().revision() <= revision {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "Viewport preparation timed out"
                );
                harness.settle(16);
                std::thread::sleep(Duration::from_millis(2));
            }
            harness.settle(16);
        };
        wait(&mut harness, initial);
        let wait_series =
            |harness: &mut zgui_platform_headless::Harness<zgui::runtime::Runtime>| {
                let deadline = Instant::now() + Duration::from_secs(10);
                while zgui::canvas::resolve(handle.scene().token())
                    .unwrap()
                    .lock()
                    .unwrap()
                    .series()
                    .is_empty()
                {
                    assert!(
                        Instant::now() < deadline,
                        "Prepared series did not become visible"
                    );
                    harness.settle(32);
                    std::thread::sleep(Duration::from_millis(2));
                }
            };
        wait_series(&mut harness);
        let revision = handle.scene().revision();
        let view = handle.scene().view();
        let started = Instant::now();
        for step in 1..=50 {
            let offset = step as f64;
            state.ranges.set(crate::Ranges::new(
                (100. + offset, 200. + offset),
                (-1., 1.),
            ));
            harness.settle(16);
            std::thread::sleep(Duration::from_millis(2));
        }
        harness.settle(32);
        eprintln!(
            "50 dense pans: {:?}, geometry rebuilds {}",
            started.elapsed(),
            handle.scene().revision() - revision
        );
        assert_eq!(
            handle.scene().revision(),
            revision,
            "Nearby pans must retain prepared buffers"
        );
        assert!(
            handle.scene().view() > view,
            "Pan still updates viewport transform"
        );
        let vertices = || {
            let scene = zgui::canvas::resolve(handle.scene().token()).unwrap();
            let scene = scene.lock().unwrap();
            match &scene.series()[0].series {
                zgui::canvas::Series::Line { data, .. } => data.clone(),
                _ => panic!("Expected retained line"),
            }
        };
        let original_vertices = vertices();
        state
            .ranges
            .set(crate::Ranges::new((125., 175.), (-1., 1.)));
        harness.settle(32);
        assert_eq!(
            handle.scene().revision(),
            revision,
            "Small zoom-in retains full requested density"
        );
        state
            .ranges
            .set(crate::Ranges::new((140., 160.), (-1., 1.)));
        wait(&mut harness, revision);
        let zoomed = handle.scene().revision();
        assert!(zoomed > revision, "Zoom in rebuilds at higher density");
        state
            .ranges
            .set(crate::Ranges::new((100., 200.), (-1., 1.)));
        harness.settle(32);
        assert!(
            std::sync::Arc::ptr_eq(&original_vertices, &vertices()),
            "Returning to recent zoom level restores original buffer"
        );
        visible.set(false);
        harness.settle(32);
        let hidden = handle.scene().revision();
        visible.set(true);
        wait(&mut harness, hidden);
        wait_series(&mut harness);
        assert!(
            handle.scene().revision() > hidden,
            "Showing a hidden plot restores cleared geometry"
        );
        for pattern in [
            LinePattern::Dashed,
            LinePattern::Dotted,
            LinePattern::DashDot,
        ] {
            state
                .ranges
                .set(crate::Ranges::new((100., 200.), (-1., 1.)));
            let old = vertices();
            drawing.set(BTreeMap::from([(
                "a".into(),
                LineDraw {
                    pattern,
                    ..Default::default()
                },
            )]));
            let deadline = Instant::now() + Duration::from_secs(10);
            while std::sync::Arc::ptr_eq(&old, &vertices()) {
                harness.settle(32);
                assert!(Instant::now() < deadline, "Pattern preparation timed out");
                std::thread::sleep(Duration::from_millis(2));
            }
            let retained = vertices();
            let revision = handle.scene().revision();
            let view = handle.scene().view();
            for step in 1..=30 {
                let offset = step as f64 / 30.;
                state.ranges.set(crate::Ranges::new(
                    (100. + offset * 20., 200. + offset * 20.),
                    (-1. + offset * 0.5, 1. + offset * 0.5),
                ));
                harness.settle(16);
            }
            assert_eq!(
                handle.scene().revision(),
                revision,
                "{pattern:?} pan must update transforms without rebuilding geometry"
            );
            assert!(
                std::sync::Arc::ptr_eq(&retained, &vertices()),
                "{pattern:?} must retain the uploaded vertex allocation"
            );
            assert!(handle.scene().view() > view);
        }
    }
    #[test]
    fn padded_pan_cache_reuses_coverage_without_losing_zoom_detail() {
        let original = (
            vec![(
                "sample".into(),
                123,
                (Connection::Linear, LinePattern::Solid, 0),
            )],
            100.0_f64.to_bits(),
            200.0_f64.to_bits(),
            1000,
            Lod::Columns,
            (0, 0, 0),
        );
        let mut next = original.clone();
        for step in 0..=100 {
            next.1 = (100.0 + step as f64).to_bits();
            next.2 = (200.0 + step as f64).to_bits();
            assert!(reusable(&original, &next));
        }
        next.1 = 201.0_f64.to_bits();
        next.2 = 301.0_f64.to_bits();
        assert!(
            !reusable(&original, &next),
            "Never slide cache anchor without new samples"
        );
        next = original.clone();
        next.2 = 150.0_f64.to_bits();
        assert!(!reusable(&original, &next), "Zoom in needs denser samples");
        next = original.clone();
        next.1 = 50.0_f64.to_bits();
        next.2 = 250.0_f64.to_bits();
        assert!(
            reusable(&original, &next),
            "Zoom out can use denser cached samples"
        );
        next = original.clone();
        next.3 = 2000;
        assert!(
            !reusable(&original, &next),
            "Wider plot needs denser samples"
        );
        next = original.clone();
        next.0[0].1 = 456;
        assert!(
            !reusable(&original, &next),
            "New source data invalidates cache"
        );
        next = original.clone();
        next.0[0].2.0 = Connection::Post;
        assert!(
            !reusable(&original, &next),
            "Connection changes invalidate cache"
        );
        let mut patterned = original.clone();
        patterned.0[0].2.1 = LinePattern::Dashed;
        patterned.5 = ((-1_f64).to_bits(), 1_f64.to_bits(), 500);
        next = patterned.clone();
        next.1 = 101.0_f64.to_bits();
        next.2 = 201.0_f64.to_bits();
        assert!(
            reusable(&patterned, &next),
            "Translation retains pixel spacing and the prepared buffers"
        );
        next = original.clone();
        next.4 = Lod::Exact;
        assert!(!reusable(&original, &next));
    }
    #[test]
    fn continuous_pan_keeps_completed_work_but_source_changes_reject_it() {
        let completed = (
            vec![(
                "sample".into(),
                123,
                (Connection::Pre, LinePattern::Solid, 0),
            )],
            0_f64.to_bits(),
            100_f64.to_bits(),
            800,
            Lod::Columns,
            (0, 0, 0),
        );
        let completed = detail_plan(&completed);
        let mut latest = completed.clone();
        latest.1 = 10_f64.to_bits();
        latest.2 = 110_f64.to_bits();
        latest.3 = 1200;
        assert!(compatible(&completed, &latest));
        let mut distant = latest.clone();
        distant.1 = 1000_f64.to_bits();
        distant.2 = 1100_f64.to_bits();
        assert!(
            !compatible(&completed, &distant),
            "Distant stale preparation must not replace the current viewport"
        );
        latest.0[0].1 = 456;
        assert!(!compatible(&completed, &latest));
        latest.0[0].1 = 123;
        latest.0[0].2.0 = Connection::Post;
        assert!(!compatible(&completed, &latest));
        latest.0.clear();
        assert!(!compatible(&completed, &latest));
    }
}
