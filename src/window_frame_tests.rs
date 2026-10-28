#[path = "window_frame_test_platform.rs"]
mod platform;
use crate::window_frame::WindowFrameProps;
use platform::{RecordingApp, State};
use std::sync::{Arc, Mutex};
use zgui::{geom::*, platform::*, prelude::*, render::Renderer, vocab::*};
use zgui_platform_headless::Harness;
use zgui_testkit_scene::CaptureRenderer;

fn frame(enabled: bool, scale: f64) -> Harness<RecordingApp> {
    let handler = app().with_size(800.0, 500.0)
        .with_renderer(Box::new(|_, target| {
            let mut renderer = CaptureRenderer::new(); renderer.configure(target); Ok(Box::new(renderer))
        }))
        .with_stylesheet(crate::theme::stylesheet())
        .into_handler(move || view! {
            WindowFrame(enabled=enabled) {
                column(class="workbench") { control(class="frame-content", tabindex=Focus::Sequential) {"Content"} }
            }
        }).unwrap();
    let mut harness = Harness::new(RecordingApp {
        runtime: handler.into_runtime(),
        state: Arc::new(Mutex::new(State::default())),
    });
    harness.deliver_to_first(SurfaceEvent::ScaleFactorChanged {
        scale_factor: scale,
        size: Size::new(
            DevicePx(800.0 * scale as f32),
            DevicePx(500.0 * scale as f32),
        ),
    });
    harness.settle(128);
    harness
}

fn center(h: &Harness<RecordingApp>, class: &str) -> (f32, f32) {
    let rect = h.app().runtime.windows()[0]
        .box_with_class(class)
        .unwrap_or_else(|| panic!("Missing {class}"));
    let scale = h.app().runtime.windows()[0]
        .handle()
        .scale()
        .get_untracked();
    (
        (rect.origin.x.0 + rect.size.width.0 / 2.0) / scale,
        (rect.origin.y.0 + rect.size.height.0 / 2.0) / scale,
    )
}
fn pointer(h: &mut Harness<RecordingApp>, action: PointerAction, point: (f32, f32)) {
    let mut event = PointerEvent::mouse(Point::new(CssPx(point.0), CssPx(point.1)));
    event.button = matches!(action, PointerAction::Pressed | PointerAction::Released)
        .then_some(PointerButton::Primary);
    h.deliver_to_first(SurfaceEvent::Pointer {
        action,
        event,
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    h.settle(128);
}
fn click(h: &mut Harness<RecordingApp>, class: &str) {
    let point = center(h, class);
    pointer(h, PointerAction::Moved, point);
    pointer(h, PointerAction::Pressed, point);
    pointer(h, PointerAction::Released, point);
}
fn key(h: &mut Harness<RecordingApp>, key: NamedKey) {
    h.deliver_to_first(SurfaceEvent::Key {
        state: KeyState::Pressed,
        event: KeyEvent::named(key, PhysicalKey::Unidentified(0)),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    h.settle(128);
}

#[test]
fn native_frame_routes_window_buttons_and_tracks_state_without_resizing() {
    let mut h = frame(true, 1.0);
    click(&mut h, "window-minimize");
    assert!(h.app().state.lock().unwrap().minimized);
    click(&mut h, "window-maximize");
    assert!(h.app().state.lock().unwrap().maximized);
    assert!(
        h.app().runtime.windows()[0]
            .nodes_with_class("window-resize")
            .is_empty()
    );
    click(&mut h, "window-maximize");
    assert!(!h.app().state.lock().unwrap().maximized);
    assert_eq!(
        h.app().runtime.windows()[0]
            .nodes_with_class("window-resize")
            .len(),
        8
    );
    click(&mut h, "window-fullscreen-button");
    assert_eq!(
        h.app().state.lock().unwrap().fullscreen,
        Some(FullscreenMode::Borderless)
    );
    assert!(
        h.app().runtime.windows()[0]
            .nodes_with_class("window-titlebar")
            .is_empty()
    );
    // The focused fullscreen button was removed; Escape must still reach the window.
    key(&mut h, NamedKey::Escape);
    assert_eq!(h.app().state.lock().unwrap().fullscreen, None);
    key(&mut h, NamedKey::F11);
    assert_eq!(
        h.app().state.lock().unwrap().fullscreen,
        Some(FullscreenMode::Borderless)
    );
    key(&mut h, NamedKey::F11);
    assert_eq!(h.app().state.lock().unwrap().fullscreen, None);
    assert_eq!(
        h.app().state.lock().unwrap().moves,
        0,
        "Buttons must never initiate a compositor move"
    );
    click(&mut h, "window-close");
    assert!(h.app().runtime.windows().is_empty());
    assert!(h.platform().is_exiting());
}

#[test]
fn native_frame_drags_and_all_resize_edges_work_at_fractional_and_integer_scales() {
    for scale in [1.0, 1.5, 2.0] {
        let mut h = frame(true, scale);
        let rect = h.app().runtime.windows()[0]
            .box_with_class("workbench")
            .unwrap();
        assert!(rect.origin.y.0 >= 34.0 * scale as f32);
        assert!(
            rect.origin.y.0 + rect.size.height.0 <= 500.01 * scale as f32,
            "Content must fit below title bar at {scale}x: {rect:?}"
        );
        let title = center(&h, "window-title");
        pointer(&mut h, PointerAction::Pressed, title);
        assert_eq!(h.app().state.lock().unwrap().moves, 1);
        // The compositor owns the release: the next press can still double-click to maximize.
        pointer(&mut h, PointerAction::Pressed, title);
        assert!(h.app().state.lock().unwrap().maximized);
        click(&mut h, "window-maximize");
        for (class, edge, cursor) in [
            (
                "window-resize-n",
                ResizeEdge::North,
                CursorStyle::ResizeNorthSouth,
            ),
            (
                "window-resize-s",
                ResizeEdge::South,
                CursorStyle::ResizeNorthSouth,
            ),
            (
                "window-resize-e",
                ResizeEdge::East,
                CursorStyle::ResizeEastWest,
            ),
            (
                "window-resize-w",
                ResizeEdge::West,
                CursorStyle::ResizeEastWest,
            ),
            (
                "window-resize-ne",
                ResizeEdge::NorthEast,
                CursorStyle::ResizeNorthEastSouthWest,
            ),
            (
                "window-resize-nw",
                ResizeEdge::NorthWest,
                CursorStyle::ResizeNorthWestSouthEast,
            ),
            (
                "window-resize-se",
                ResizeEdge::SouthEast,
                CursorStyle::ResizeNorthWestSouthEast,
            ),
            (
                "window-resize-sw",
                ResizeEdge::SouthWest,
                CursorStyle::ResizeNorthEastSouthWest,
            ),
        ] {
            let point = center(&h, class);
            pointer(&mut h, PointerAction::Moved, point);
            assert_eq!(h.platform().offscreens()[0].last_cursor(), Some(cursor));
            pointer(&mut h, PointerAction::Pressed, point);
            assert_eq!(h.app().state.lock().unwrap().resizes.last(), Some(&edge));
        }
    }
}

#[test]
fn native_controls_have_accessible_names_and_keyboard_activation() {
    let mut h = frame(true, 1.0);
    let updates = h.platform().offscreens()[0].take_a11y_log();
    let labels: Vec<_> = updates
        .iter()
        .flat_map(|update| &update.nodes)
        .filter_map(|(_, node)| node.label())
        .collect();
    for label in [
        "Minimize window",
        "Maximize window",
        "Close window",
        "Enter fullscreen (F11)",
    ] {
        assert!(
            labels.contains(&label),
            "Missing accessible name {label}: {labels:?}"
        );
    }
    click(&mut h, "window-maximize");
    key(&mut h, NamedKey::Enter);
    assert!(
        !h.app().state.lock().unwrap().maximized,
        "Focused window button must activate from keyboard"
    );
}

#[test]
fn other_backends_keep_the_full_content_area_and_platform_decorations() {
    let h = frame(false, 1.0);
    assert!(
        h.app().runtime.windows()[0]
            .nodes_with_class("window-titlebar")
            .is_empty()
    );
    let rect = h.app().runtime.windows()[0]
        .box_with_class("workbench")
        .unwrap();
    assert_eq!(rect.origin.y.0, 0.0);
    assert_eq!(rect.size.height.0, 500.0);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a running Wayland compositor and GPU renderer"]
fn native_wayland_compositor_configures_maximize_restore_and_fullscreen() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    let completed = Arc::new(AtomicUsize::new(0));
    let report = completed.clone();
    app().with_size(800.0, 500.0)
        .with_decorations(Decorations::None)
        .with_stylesheet(crate::theme::stylesheet())
        .run_on(zgui_platform_wayland::run, move || {
            let window = zgui::runtime::use_window();
            let stage = completed.clone();
            let attempts = std::cell::Cell::new(0);
            let stable_since = std::cell::Cell::new(None::<std::time::Instant>);
            let timer = set_interval(Duration::from_millis(100), move || {
                attempts.set(attempts.get() + 1);
                if attempts.get() > 150 { window.close(); return; }
                let current = stage.load(Ordering::Relaxed);
                let ready = match current {
                    0 => true,
                    1 => window.maximized().get_untracked(),
                    2 => !window.maximized().get_untracked(),
                    3 => window.fullscreen().get_untracked().is_some(),
                    4 => window.fullscreen().get_untracked().is_none(),
                    _ => false,
                };
                if !ready { stable_since.set(None); return; }
                // Let the configured frame reach the compositor before requesting another
                // state. Weston applies these states when that buffer is committed.
                let since = stable_since.get().unwrap_or_else(|| {
                    let now = std::time::Instant::now(); stable_since.set(Some(now)); now
                });
                if since.elapsed() < Duration::from_millis(500) { return; }
                stable_since.set(None);
                let next = match stage.load(Ordering::Relaxed) {
                    0 => { window.set_maximized(true); 1 }
                    1 if window.maximized().get_untracked() => { window.set_maximized(false); 2 }
                    2 if !window.maximized().get_untracked() => { window.set_fullscreen(Some(FullscreenMode::Borderless)); 3 }
                    3 if window.fullscreen().get_untracked().is_some() => { window.set_fullscreen(None); 4 }
                    4 if window.fullscreen().get_untracked().is_none() => { window.close(); 5 }
                    current => current,
                };
                stage.store(next, Ordering::Relaxed);
            });
            on_cleanup_local(move || drop(timer));
            view! { WindowFrame(enabled=true) { column(class="workbench") { "Native Wayland window smoke test" } } }
        }).unwrap();
    assert_eq!(
        report.load(Ordering::Relaxed),
        5,
        "Compositor did not acknowledge the requested window states within 15 seconds"
    );
}
