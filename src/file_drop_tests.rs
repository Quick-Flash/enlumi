use crate::{app::WorkbenchAppProps, preferences::LastLogDirectory};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use zgui::{
    geom::{CssPx, DevicePx, Point, Size},
    platform::{DragEvent, SurfaceEvent},
    prelude::*,
    render::Renderer,
};
use zgui_platform_headless::Harness;
use zgui_testkit_scene::CaptureRenderer;

fn renderer() -> zgui::runtime::RendererFactory {
    Box::new(|_, target| {
        let mut renderer = CaptureRenderer::new();
        renderer.configure(target);
        Ok(Box::new(renderer))
    })
}

fn drop_files(harness: &mut Harness<zgui::runtime::Runtime>, paths: Vec<PathBuf>, x: f32, y: f32) {
    harness.deliver_to_first(SurfaceEvent::Drag(DragEvent::Dropped {
        paths,
        position: Point::new(CssPx(x), CssPx(y)),
    }));
    harness.settle(128);
}

fn text(harness: &Harness<zgui::runtime::Runtime>, class: &str) -> String {
    let window = &harness.app().windows()[0];
    window.dom().text_content(window.nodes_with_class(class)[0])
}

fn wait_for(harness: &mut Harness<zgui::runtime::Runtime>, class: &str, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        harness.settle(128);
        if text(harness, class) == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Expected {class}={expected:?}; status={}",
            text(harness, "status")
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn file_drop_opens_replaces_and_validates_logs() {
    let temp = tempfile::tempdir().unwrap();
    let directory = LastLogDirectory::at(temp.path().join("config/last-log-directory"));
    let store = directory.clone();
    let logs = temp.path().join("logs");
    std::fs::create_dir(&logs).unwrap();
    let first = logs.join("flight ü 1.ULG");
    let second = logs.join("second flight.ulg");
    let fixture = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/esc_status_log.ulg"
    ));
    for path in [&first, &second] {
        std::fs::copy(&fixture, path).unwrap();
    }
    let handler = app().with_size(1500.0, 850.0)
        .with_renderer(renderer())
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .into_handler(move || view! { WorkbenchApp(initial_path=None, client_frame=true, preferences=crate::preferences::ThemePreference::disabled(), log_directory=directory.clone()) })
        .unwrap();
    let mut harness = Harness::new(handler.into_runtime());
    harness.deliver_to_first(SurfaceEvent::Resized(Size::new(
        DevicePx(1500.0),
        DevicePx(850.0),
    )));
    harness.settle(128);
    let plot = harness.app().windows()[0]
        .box_with_class("time-series-plot")
        .unwrap();
    let (x, y) = (
        plot.origin.x.0 + plot.size.width.0 / 2.0,
        plot.origin.y.0 + plot.size.height.0 / 2.0,
    );

    harness.deliver_to_first(SurfaceEvent::Drag(DragEvent::Entered {
        paths: vec![first.clone()],
        position: Point::new(CssPx(x), CssPx(y)),
    }));
    harness.deliver_to_first(SurfaceEvent::Drag(DragEvent::Left));
    harness.settle(128);
    assert_eq!(text(&harness, "file-name"), "Enlumi");
    assert_eq!(store.load(), None);

    // No control has focus: the event must reach the app through the drop position.
    drop_files(&mut harness, vec![first.clone()], x, y);
    wait_for(&mut harness, "file-name", "flight ü 1.ULG");
    assert!(
        !harness.app().windows()[0]
            .nodes_with_class("raw-section")
            .is_empty()
    );
    assert_eq!(store.load(), Some(logs.clone()));

    drop_files(&mut harness, vec![first.clone(), second.clone()], x, y);
    assert_eq!(text(&harness, "status"), "Drop one log file at a time");
    assert_eq!(text(&harness, "file-name"), "flight ü 1.ULG");
    drop_files(&mut harness, vec![], x, y);
    assert_eq!(text(&harness, "status"), "Drop one log file at a time");

    let title = harness.app().windows()[0]
        .box_with_class("window-title")
        .unwrap();
    drop_files(
        &mut harness,
        vec![second],
        title.origin.x.0 + 20.0,
        title.origin.y.0 + 10.0,
    );
    wait_for(&mut harness, "file-name", "second flight.ulg");

    let invalid = logs.join("unsupported.data");
    std::fs::write(&invalid, b"not a log").unwrap();
    let error = crate::log::LogStore::read(&invalid).err().unwrap();
    drop_files(&mut harness, vec![invalid], x, y);
    wait_for(&mut harness, "status", &error);
    assert_eq!(text(&harness, "file-name"), "second flight.ulg");
}

#[test]
fn file_drop_targets_position_with_and_without_focus_at_each_scale() {
    use std::{cell::RefCell, rc::Rc};
    use zgui::vocab::{Modifiers, PointerAction, PointerEvent, Timestamp};
    for scale in [1.0, 2.0] {
        let received = Rc::new(RefCell::new(Vec::new()));
        let left = received.clone();
        let right = received.clone();
        let handler = app().with_size(400.0, 200.0)
            .with_renderer(renderer())
            .with_stylesheet(".targets {width:400px;height:200px;} .target {width:200px;height:200px;flex:none;}")
            .into_handler(move || {
                let left = left.clone();
                let right = right.clone();
                view! {
                row(class="targets") {
                    control(class="target",tabindex=Focus::Sequential,on:drop=move |_|left.borrow_mut().push("left")) {"Focus here"}
                    column(class="target",on:drop=move |_|right.borrow_mut().push("right")) {"Drop here"}
                }
                }
            }).unwrap();
        let mut harness = Harness::new(handler.into_runtime());
        harness.deliver_to_first(SurfaceEvent::ScaleFactorChanged {
            scale_factor: scale,
            size: Size::new(
                DevicePx(400.0 * scale as f32),
                DevicePx(200.0 * scale as f32),
            ),
        });
        harness.settle(128);
        assert!(
            harness.app().windows()[0]
                .host()
                .focused()
                .get_untracked()
                .is_none()
        );
        drop_files(&mut harness, vec!["test.ulg".into()], 300.0, 100.0);
        assert_eq!(*received.borrow(), ["right"]);
        let mut event = PointerEvent::mouse(Point::new(CssPx(100.0), CssPx(100.0)));
        event.button = Some(zgui::vocab::PointerButton::Primary);
        for action in [PointerAction::Pressed, PointerAction::Released] {
            harness.deliver_to_first(SurfaceEvent::Pointer {
                action,
                event,
                modifiers: Modifiers::NONE,
                timestamp: Timestamp::ORIGIN,
            });
        }
        harness.settle(128);
        let window = &harness.app().windows()[0];
        assert_eq!(
            window.host().focused().get_untracked(),
            Some(window.nodes_with_class("target")[0])
        );
        drop_files(&mut harness, vec!["test.ulg".into()], 300.0, 100.0);
        drop_files(&mut harness, vec!["test.ulg".into()], -10.0, -10.0);
        assert_eq!(*received.borrow(), ["right", "right"]);
    }
}
