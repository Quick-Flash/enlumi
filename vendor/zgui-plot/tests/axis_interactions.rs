#![cfg(feature = "workspace")]

use std::{cell::RefCell, rc::Rc};
use zgui::{
    geom::{CssPx, DevicePx, Point, Size},
    platform::SurfaceEvent,
    prelude::*,
    render::Renderer,
    vocab::{Modifiers, PointerAction, PointerButton, PointerEvent, Timestamp},
};
use zgui_platform_headless::Harness;
use zgui_plot::{
    PlotPoint, Ranges,
    time_series::{SeriesData, TimeSeries, TimeSeriesSet},
    zgui_plot::PlotState,
    zgui_time_series::TimeSeriesPlotProps,
};
use zgui_testkit_scene::CaptureRenderer;
use zgui_ui_tokens::prelude::*;

fn pointer(action: PointerAction, x: f32, y: f32, button: PointerButton) -> SurfaceEvent {
    let mut event = PointerEvent::mouse(Point::new(CssPx(x), CssPx(y)));
    event.button =
        matches!(action, PointerAction::Pressed | PointerAction::Released).then_some(button);
    SurfaceEvent::Pointer {
        action,
        event,
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    }
}
fn click(harness: &mut Harness<zgui::runtime::Runtime>, x: f32, y: f32, button: PointerButton) {
    for action in [PointerAction::Pressed, PointerAction::Released] {
        harness.deliver_to_first(pointer(action, x, y, button));
    }
    harness.settle(128);
}
fn command(harness: &mut Harness<zgui::runtime::Runtime>, class: &str) {
    let rect = harness.app().windows()[0].box_with_class(class).unwrap();
    click(
        harness,
        rect.origin.x.0 + rect.size.width.0 / 2.0,
        rect.origin.y.0 + rect.size.height.0 / 2.0,
        PointerButton::Primary,
    );
}

#[test]
fn stock_plot_owns_context_fit_axis_double_click_and_vertical_drag() {
    let model = Rc::new(RefCell::new(None));
    let built = Rc::clone(&model);
    let handler = app()
        .with_size(900.0, 600.0)
        .with_renderer(Box::new(|_, target| {
            let mut renderer = CaptureRenderer::new();
            renderer.configure(target);
            Ok(Box::new(renderer))
        }))
        .with_stylesheet(format!(
            "{}\n{}\n.test-root {{width:100vw;height:100vh;}}",
            zgui_plot::zgui_surface::stylesheet(),
            zgui_plot::zgui_time_series::stylesheet()
        ))
        .into_handler(move || {
            let state = PlotState::default();
            let data = SeriesData::new(vec![
                PlotPoint { x: 0.0, y: 0.0 },
                PlotPoint { x: 100.0, y: 200.0 },
            ])
            .unwrap();
            let series = TimeSeriesSet::new(vec![TimeSeries::new("a", "a", data)]).unwrap();
            *built.borrow_mut() = Some(state);
            view! { ThemeProvider(scheme = ColorScheme::Dark) { column(class = "test-root") {
                TimeSeriesPlot(series = Signal::stored_local(series), state = state)
            } } }
        })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(900.0),
        DevicePx(600.0),
    )));
    harness.settle(256);
    let state = model.borrow().unwrap();
    let fitted = state.ranges.get_untracked();
    let graph = harness.app().windows()[0]
        .box_with_class("time-series-plot")
        .unwrap();
    let center = (
        graph.origin.x.0 + graph.size.width.0 / 2.0,
        graph.origin.y.0 + graph.size.height.0 / 2.0,
    );
    let zoom = Ranges::new((10.0, 20.0), (30.0, 40.0));
    for (class, expected) in [
        (
            "plot-fit-x",
            Ranges {
                x: fitted.x,
                y: zoom.y,
            },
        ),
        (
            "plot-fit-y",
            Ranges {
                x: zoom.x,
                y: (19.0, 41.0).into(),
            },
        ),
        ("plot-fit-both", fitted),
    ] {
        state.set_view(zoom).unwrap();
        harness.settle(128);
        click(&mut harness, center.0, center.1, PointerButton::Secondary);
        command(&mut harness, class);
        assert_eq!(state.ranges.get_untracked(), expected);
        assert!(!state.auto_fit.get_untracked());
    }
    // Secondary legend press opens menu without changing visibility.
    let legend = harness.app().windows()[0]
        .box_with_class("plot-legend")
        .unwrap();
    click(
        &mut harness,
        legend.origin.x.0 + 12.0,
        legend.origin.y.0 + 10.0,
        PointerButton::Secondary,
    );
    assert!(state.hidden.get_untracked().is_empty());
    command(&mut harness, "plot-fit-both");
    for (point, expected) in [
        (
            (center.0, graph.origin.y.0 + graph.size.height.0 - 8.0),
            Ranges {
                x: fitted.x,
                y: zoom.y,
            },
        ),
        (
            (
                graph.origin.x.0 + 10.0,
                graph.origin.y.0 + graph.size.height.0 * 0.4,
            ),
            Ranges {
                x: zoom.x,
                y: (19.0, 41.0).into(),
            },
        ),
    ] {
        state.set_view(zoom).unwrap();
        harness.settle(128);
        click(&mut harness, point.0, point.1, PointerButton::Primary);
        click(&mut harness, point.0, point.1, PointerButton::Primary);
        assert_eq!(state.ranges.get_untracked(), expected);
    }
    state.set_view(zoom).unwrap();
    harness.settle(128);
    let (x, y) = (
        graph.origin.x.0 + 10.0,
        graph.origin.y.0 + graph.size.height.0 * 0.4,
    );
    harness.deliver_to_first(pointer(
        PointerAction::Pressed,
        x,
        y,
        PointerButton::Primary,
    ));
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        x,
        y - 30.0,
        PointerButton::Primary,
    ));
    harness.deliver_to_first(pointer(
        PointerAction::Released,
        x,
        y - 30.0,
        PointerButton::Primary,
    ));
    harness.settle(128);
    let dragged = state.ranges.get_untracked();
    assert_eq!(dragged.x, zoom.x);
    assert_eq!(dragged.y.min, zoom.y.min);
    assert!(dragged.y.max > zoom.y.max);
    state.set_view(zoom).unwrap();
    harness.settle(128);
    harness.deliver_to_first(pointer(
        PointerAction::Pressed,
        x,
        y,
        PointerButton::Primary,
    ));
    harness.deliver_to_first(pointer(
        PointerAction::Moved,
        x,
        y - 30.0,
        PointerButton::Primary,
    ));
    harness.deliver_to_first(pointer(
        PointerAction::Cancelled,
        x,
        y - 30.0,
        PointerButton::Primary,
    ));
    harness.settle(128);
    assert_eq!(state.ranges.get_untracked(), zoom);
    assert!(!state.auto_fit.get_untracked());
    // Wheel over either axis zooms around pointer while preserving other axis.
    for (fx, fy, x_axis) in [
        (0.25, 0.99, true),
        (0.75, 0.99, true),
        (0.01, 0.25, false),
        (0.01, 0.75, false),
    ] {
        for delta in [-1.0, 1.0] {
            state.set_view(zoom).unwrap();
            harness.settle(128);
            harness.deliver_to_first(SurfaceEvent::Wheel {
                event: zgui::vocab::WheelEvent {
                    delta: zgui::vocab::ScrollDelta::Lines { x: 0.0, y: delta },
                    phase: zgui::vocab::ScrollPhase::Moved,
                    position: Point::new(
                        CssPx(graph.origin.x.0 + graph.size.width.0 * fx),
                        CssPx(graph.origin.y.0 + graph.size.height.0 * fy),
                    ),
                    id: zgui::vocab::PointerId::MOUSE,
                    kind: zgui::vocab::PointerKind::Mouse,
                },
                modifiers: Modifiers::NONE,
                timestamp: Timestamp::ORIGIN,
            });
            harness.settle(128);
            let after = state.ranges.get_untracked();
            let (before, after, fraction) = if x_axis {
                assert_eq!(after.y, zoom.y);
                (zoom.x, after.x, f64::from(fx))
            } else {
                assert_eq!(after.x, zoom.x);
                (zoom.y, after.y, 1.0 - f64::from(fy))
            };
            assert_ne!(before, after);
            assert!(
                (before.min + before.span() * fraction - after.min - after.span() * fraction).abs()
                    < 1e-5
            );
            assert!(!state.auto_fit.get_untracked());
        }
    }
}
