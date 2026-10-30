use super::{PlotState, render};
use crate::{
    PlotGeometry, PlotPoint,
    plot_data::{HistogramSeries, HistogramSet, PlotData, ScatterSeries, ScatterSet},
    time_series::{SeriesData, TimeSeries, TimeSeriesSet},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use zgui::{prelude::*, reactive::RenderEffect, render::Renderer};

#[test]
fn hover_preserves_nearest_identity_and_does_not_notify_for_unchanged_hits() {
    let built = Rc::new(RefCell::new(None));
    let model = Rc::clone(&built);
    let runs = Rc::new(Cell::new(0));
    let observed = Rc::clone(&runs);
    let handler = app()
        .with_size(300., 200.)
        .with_renderer(Box::new(|_, target| {
            let mut renderer = zgui_testkit_scene::CaptureRenderer::new();
            renderer.configure(target);
            Ok(Box::new(renderer))
        }))
        .into_handler(move || {
            let state = PlotState::default();
            state
                .set_view(crate::Ranges::new((0., 1.), (0., 1.)))
                .unwrap();
            let observed = Rc::clone(&observed);
            let watcher = RenderEffect::new(move |_| {
                state.hover.track();
                state.bin_hover.track();
                state.ranges.track();
                state.auto_fit.track();
                observed.set(observed.get() + 1);
            });
            on_cleanup_local(move || drop(watcher));
            *model.borrow_mut() = Some(state);
            view! { box {} }
        })
        .unwrap();
    let mut harness = zgui_platform_headless::Harness::new(handler.into_runtime());
    harness.settle(16);
    let state = built.borrow().unwrap();
    let definitions = [
        ("far", vec![PlotPoint { x: 0.5, y: 0.51 }]),
        (
            "winner",
            vec![
                PlotPoint {
                    x: 0.49,
                    y: f64::NAN,
                },
                PlotPoint { x: 0.5, y: 0.5 },
            ],
        ),
        ("tie", vec![PlotPoint { x: 0.5, y: 0.5 }]),
    ];
    let time = PlotData::Time(
        TimeSeriesSet::new(
            definitions
                .iter()
                .map(|(id, samples)| {
                    TimeSeries::new(*id, *id, SeriesData::new(samples.clone()).unwrap())
                })
                .collect(),
        )
        .unwrap(),
    );
    let scatter = PlotData::Scatter(
        ScatterSet::new(
            definitions
                .iter()
                .map(|(id, samples)| ScatterSeries::new(*id, *id, samples.clone()).unwrap())
                .collect(),
        )
        .unwrap(),
    );
    let geometry = PlotGeometry::with_bounds(0., 0., 1000., 600., 1000., 600.).unwrap();
    let target = PlotPoint { x: 0.5, y: 0.5 };
    for data in [time, scatter] {
        state.hidden.set(Default::default());
        let hit = render::hover(&data, state, target, geometry).unwrap();
        assert_eq!(
            hit.series_id, "winner",
            "Equal distances retain first series"
        );
        assert_eq!(hit.sample_index, 1, "Gaps retain original sample indices");
        let before = runs.get();
        state.set_hover(Some(hit.clone()));
        harness.settle(16);
        assert_eq!(runs.get(), before + 1);
        for _ in 0..100 {
            state.set_hover(render::hover(&data, state, target, geometry));
        }
        harness.settle(16);
        assert_eq!(
            runs.get(),
            before + 1,
            "Same sample should not redraw hover subscribers"
        );
        state.hidden.update(|hidden| {
            hidden.insert("winner".into());
        });
        let next = render::hover(&data, state, target, geometry).unwrap();
        assert_eq!(next.series_id, "tie");
        state.set_hover(Some(next));
        harness.settle(16);
        assert_eq!(runs.get(), before + 2);
        state.set_hover(render::hover(
            &data,
            state,
            PlotPoint { x: 2., y: 2. },
            geometry,
        ));
        harness.settle(16);
        assert!(state.hover.get_untracked().is_none());
        assert_eq!(runs.get(), before + 3);
        state.set_hover(None);
        harness.settle(16);
        assert_eq!(runs.get(), before + 3);
    }
    state.hidden.set(Default::default());
    let histogram = PlotData::Histogram(
        HistogramSet::new(vec![HistogramSeries::new("hist", "hist", [0.5])]).unwrap(),
    );
    assert!(render::hover(&histogram, state, target, geometry).is_none());
    harness.settle(16);
    assert!(state.bin_hover.get_untracked().is_some());
    let before = runs.get();
    render::hover(&histogram, state, target, geometry);
    harness.settle(16);
    assert_eq!(
        runs.get(),
        before,
        "Same histogram bin should not redraw hover subscribers"
    );
    let ranges = state.ranges.get_untracked();
    state.set_view(ranges).unwrap();
    harness.settle(16);
    let before = runs.get();
    for _ in 0..100 {
        state.set_view(ranges).unwrap();
    }
    harness.settle(16);
    assert_eq!(
        runs.get(),
        before,
        "Repeated view settings must not invalidate unchanged state"
    );
}
