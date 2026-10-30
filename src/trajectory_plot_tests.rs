use super::*;
use crate::{state::Line, workspace::PlotWorkspaceProps};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};
use zgui::{
    geom::{DevicePx, Size},
    platform::SurfaceEvent,
    render::{RenderTarget, Renderer},
};
use zgui_platform_headless::Harness;
use zgui_testkit_scene::CaptureRenderer;

fn settle_until(harness: &mut Harness<zgui::runtime::Runtime>, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        harness.settle(64);
        assert!(Instant::now() < deadline, "Preparation did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn replacements_keep_displayed_tracks_and_only_prepare_the_latest_pending_inputs() {
    let (started, starts) = mpsc::channel();
    let (resume, permission) = mpsc::channel();
    let permission = Mutex::new(permission);
    let ui_thread = std::thread::current().id();
    let hook = PreparationHook(Arc::new(move |lines, spec| {
        assert_ne!(
            std::thread::current().id(),
            ui_thread,
            "Alignment must run off the UI thread"
        );
        let _ = started.send(spec.y.clone());
        // A controlled worker boundary makes retention and stale-result checks
        // independent of machine speed. Dropping the sender releases it on panic.
        let _ = permission
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(10));
        Tracks::prepare(lines, spec)
    }));
    let saved = Rc::new(RefCell::new(None));
    let captured = saved.clone();
    let handler = app().with_size(1000., 700.)
        .with_renderer(Box::new(|_, target: RenderTarget| {
            let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(renderer))
        }))
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(),
            plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .into_handler(move || {
            provide_local_context(hook.clone());
            let workbench = Workbench::new();
            let pane = workbench.pane();
            let lines = ["x", "y", "z", "w"].into_iter().enumerate().map(|(i, source)| Line {
                id: workbench.id(), source: source.into(), name: source.into(), revision: 0,
                data: plot::time_series::SeriesData::new((0..3).map(|t| plot::PlotPoint {
                    x: t as f64, y: (t * (i + 1)) as f64,
                }).collect::<Vec<_>>()).unwrap(),
            }).collect::<Vec<_>>();
            let spec = Spec { x: "x".into(), y: "y".into(), equal: false, ..Default::default() };
            let displayed = Arc::new(Tracks::prepare(&lines, &spec).unwrap());
            pane.lines.set(lines);
            pane.trajectory.set(Some(spec));
            pane.trajectory_data.set(Some(displayed.clone()));
            *captured.borrow_mut() = Some((workbench, pane, displayed));
            view! { column(class="page workbench") { row(class="body") { PlotWorkspace(workbench=workbench) } } }
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1000.),
        DevicePx(700.),
    )));
    harness.settle(128);
    let (_workbench, pane, displayed) = saved.borrow().as_ref().unwrap().clone();
    let set_y = |source: &str| {
        pane.trajectory
            .update(|spec| spec.as_mut().unwrap().y = source.into())
    };
    let retained = || {
        pane.trajectory_data.with_untracked(|data| {
            data.as_ref()
                .is_some_and(|data| Arc::ptr_eq(data, &displayed))
        })
    };
    set_y("z");
    settle_until(&mut harness, || {
        starts.try_recv().is_ok_and(|source| source == "z")
    });
    assert!(
        retained(),
        "Starting replacement must keep the displayed track"
    );
    for source in ["w", "y", "z", "w"] {
        set_y(source);
        harness.settle(64);
        assert!(retained());
    }
    assert!(starts.try_recv().is_err(), "Only one job per chart may run");
    resume.send(()).unwrap();
    settle_until(&mut harness, || {
        starts.try_recv().is_ok_and(|source| source == "w")
    });
    assert!(
        retained(),
        "Superseded completion must not replace displayed geometry"
    );
    resume.send(()).unwrap();
    settle_until(&mut harness, || {
        pane.trajectory_data.with_untracked(|data| {
            data.as_ref().is_some_and(|data| {
                data.matches(
                    &pane.lines.get_untracked(),
                    &pane.trajectory.get_untracked().unwrap(),
                )
            })
        })
    });
    assert!(
        starts.try_recv().is_err(),
        "Intermediate requests must not prepare"
    );
    set_y("missing");
    settle_until(&mut harness, || {
        starts.try_recv().is_ok_and(|source| source == "missing")
    });
    assert!(pane.trajectory_data.with_untracked(Option::is_some));
    resume.send(()).unwrap();
    settle_until(&mut harness, || {
        pane.trajectory_data.with_untracked(Option::is_none)
    });
}

#[test]
fn xy_pan_updates_canvas_view_without_reencoding_retained_lines() {
    use plot::line_draw::{LineDraw, LinePattern};
    let probe = OverlayProbe(Rc::new(RefCell::new(None)));
    let observed = probe.clone();
    let saved = Rc::new(RefCell::new(None));
    let captured = saved.clone();
    let handler=app().with_size(400.,300.)
        .with_renderer(Box::new(|_,target| {let mut renderer=CaptureRenderer::new();renderer.configure(target);Ok(Box::new(renderer))}))
        .with_stylesheet("column.xy-test {position:relative;width:400px;height:300px;} .trajectory-overlay,.trajectory-canvas,.trajectory-marker-canvas {position:absolute;inset:0;width:100%;height:100%;}")
        .into_handler(move || {
            provide_local_context(probe.clone());
            let workbench=Workbench::new(); let pane=workbench.pane();
            let lines=["x","y"].into_iter().enumerate().map(|(axis,source)|Line {
                id:workbench.id(), source:source.into(), name:source.into(), revision:0,
                data:plot::time_series::SeriesData::new((0..20_000).map(|i| {
                    let angle=i as f64*std::f64::consts::TAU/19_999.;
                    plot::PlotPoint{x:i as f64,y:5.+3.*if axis==0 {angle.sin()} else {angle.cos()}}
                }).collect::<Vec<_>>()).unwrap(),
            }).collect::<Vec<_>>();
            let spec=Spec{x:"x".into(),y:"y".into(),equal:false,..Default::default()};
            pane.trajectory_data.set(Some(Arc::new(Tracks::prepare(&lines,&spec).unwrap())));
            pane.lines.set(lines); pane.trajectory.set(Some(spec));
            pane.plot.auto_fit.set(false); pane.plot.set_view(plot::Ranges::new((0.,10.),(0.,10.))).unwrap();
            *captured.borrow_mut()=Some(pane);
            view! {column(class="xy-test") {TrajectoryOverlay(workbench=workbench,pane=pane,visible=Signal::stored_local(true))}}
        }).unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(400.),
        DevicePx(300.),
    )));
    harness.settle(64);
    let pane = saved.borrow().unwrap();
    let y = pane.lines.with_untracked(|lines| lines[1].id);
    let (handle, paths) = {
        let probe = observed.0.borrow();
        let probe = probe.as_ref().unwrap();
        (probe.canvas.clone(), probe.paths)
    };
    settle_until(&mut harness, || {
        paths.with_untracked(|paths| paths.contains_key(&y))
    });
    harness.settle(64);
    let vertices = || {
        let scene = zgui::canvas::resolve(handle.scene().token()).unwrap();
        let scene = scene.lock().unwrap();
        match &scene.series()[0].series {
            zgui::canvas::Series::Line { data, .. } => data.clone(),
            _ => panic!("Expected a retained XY line"),
        }
    };
    for pattern in [
        LinePattern::Solid,
        LinePattern::Dashed,
        LinePattern::Dotted,
        LinePattern::DashDot,
    ] {
        pane.plot
            .set_view(plot::Ranges::new((0., 10.), (0., 10.)))
            .unwrap();
        let previous = vertices();
        pane.drawing.set(std::collections::BTreeMap::from([(
            y,
            LineDraw {
                pattern,
                ..Default::default()
            },
        )]));
        if pattern != LinePattern::Solid {
            settle_until(&mut harness, || !Arc::ptr_eq(&previous, &vertices()));
        }
        harness.settle(64);
        let retained = vertices();
        let revision = handle.scene().revision();
        let view = handle.scene().view();
        for step in 1..=20 {
            let offset = step as f64 * 0.1;
            pane.plot
                .set_view(plot::Ranges::new(
                    (offset, 10. + offset),
                    (offset * 0.5, 10. + offset * 0.5),
                ))
                .unwrap();
            harness.settle(16);
        }
        assert_eq!(
            handle.scene().revision(),
            revision,
            "{pattern:?} XY pan must retain canvas content and GPU chunks"
        );
        assert!(Arc::ptr_eq(&retained, &vertices()));
        assert!(handle.scene().view() > view);
    }
}
