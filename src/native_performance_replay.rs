//! Opt-in comparison through real desktop backends, with identical raw plots and input.
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use zgui::{
    geom::{CssPx, Point},
    platform::{AppHandler, IdlePolicy, PlatformCx, SurfaceEvent, SurfaceId, WakeReason},
    prelude::*,
    vocab::{Modifiers, PointerAction, PointerButton, PointerEvent},
};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    use crate::{
        browser::BrowserProps, fps::FpsProps, theme::AppThemeProps, workspace::PlotWorkspaceProps,
    };
    let path = PathBuf::from(std::env::var_os("ENLUMI_REPLAY_LOG").expect("ENLUMI_REPLAY_LOG"));
    let log = Arc::new(crate::log::LogStore::read(&path).unwrap());
    let fields = log
        .catalogue
        .iter()
        .filter(|field| {
            let name = field.id.to_ascii_lowercase();
            name.contains("gyro") || name.contains("rpm")
        })
        .take(12)
        .map(|field| (field.id.clone(), log.prepare(&field.id).unwrap()))
        .collect::<Vec<_>>();
    assert!(fields.len() >= 6, "need at least six gyro/RPM fields");
    eprintln!(
        "Replay raw fields: {:?}",
        fields.iter().map(|(id, _)| id).collect::<Vec<_>>()
    );
    app().with_title("Enlumi backend replay — move to 165 Hz display")
        .with_size(1500., 850.).with_probe(crate::fps::probe())
        .with_stylesheet(format!("{}\n{}\n{}\n{}", plot::zgui_surface::stylesheet(), plot::zgui_time_series::stylesheet(), plot::zgui_workspace::stylesheet(), crate::theme::stylesheet()))
        .run_on(|inner| zgui::app::desktop()(Box::new(Replay { inner, start: Instant::now(), next: Instant::now(), surface: None, dragging: false, minimized: false, restore_requested: false, max_late: Duration::ZERO, stress_step: 0 })), move || {
            let workbench = crate::state::Workbench::new();
            workbench.log.set(Some(log.clone()));
            let mut pane = workbench.pane();
            for (index, group) in fields.chunks(3).enumerate() {
                if index > 0 { pane = workbench.add_plot(pane.id, true, false); }
                pane.lines.set(group.iter().map(|(id, data)| crate::state::Line { id: workbench.id(), source: id.clone(), name: id.clone(), data: data.clone(), revision: 0 }).collect());
                pane.links.update(|links| links.x = true);
            }
            view! { AppTheme(palette = crate::theme::Palette::Forest) { column(class = "page workbench") { row(class = "toolbar") { Fps() } row(class = "body") { Browser(workbench = workbench) PlotWorkspace(workbench = workbench) } } } }
        })?;
    Ok(())
}

struct Replay {
    inner: Box<dyn AppHandler>,
    start: Instant,
    next: Instant,
    surface: Option<SurfaceId>,
    dragging: bool,
    minimized: bool,
    restore_requested: bool,
    max_late: Duration,
    stress_step: usize,
}
impl Replay {
    fn input(&mut self, cx: &dyn PlatformCx) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.start).as_secs_f32();
        if elapsed >= 27. {
            self.max_late = self.max_late.max(now.saturating_duration_since(self.next));
        }
        if elapsed >= 35. {
            cx.request_exit();
            return;
        }
        self.next = now + Duration::from_millis(1);
        // First 15 seconds allow moving the window to the intended monitor and preparation.
        if elapsed < 15. {
            return;
        }
        let Some(id) = self.surface else {
            return;
        };
        let surface = cx.surface(id).unwrap();
        if std::env::var_os("ENLUMI_REPLAY_STRESS").is_some() {
            let step = if elapsed >= 27. {
                42
            } else if elapsed >= 25. {
                41
            } else if (16. ..24.).contains(&elapsed) {
                1 + ((elapsed - 16.) * 5.) as usize
            } else {
                0
            };
            if step > self.stress_step {
                self.stress_step = step;
                match step {
                    41 => surface.set_fullscreen(Some(zgui::platform::FullscreenMode::Borderless)),
                    42 => surface.set_fullscreen(None),
                    _ => {
                        let (width, height) =
                            [(1000., 650.), (1500., 850.), (900., 700.), (1250., 800.)][step % 4];
                        if let Some(size) =
                            surface.request_size(zgui::geom::Size::new(CssPx(width), CssPx(height)))
                        {
                            self.inner
                                .surface_event(cx, id, SurfaceEvent::Resized(size));
                        }
                    }
                }
            }
        }
        if self.minimized {
            if elapsed >= 30.
                && !self.restore_requested
                && std::env::var_os("ENLUMI_REPLAY_RESTORE").is_some()
            {
                self.restore_requested = true;
                surface.focus();
            }
            return;
        }
        let width = surface.size().width.0 / surface.scale_factor() as f32;
        let height = surface.size().height.0 / surface.scale_factor() as f32;
        let x = width * (0.65 + 0.15 * ((elapsed - 15.) * 2.).sin());
        let mut event = PointerEvent::mouse(Point::new(CssPx(x), CssPx(height * 0.35)));
        let action = if elapsed >= 25. && !self.dragging {
            self.dragging = true;
            event.button = Some(PointerButton::Primary);
            PointerAction::Pressed
        } else {
            PointerAction::Moved
        };
        self.inner.surface_event(
            cx,
            id,
            SurfaceEvent::Pointer {
                action,
                event,
                modifiers: Modifiers::NONE,
                timestamp: cx.clock().timestamp(),
            },
        );
        if elapsed >= 28.
            && !self.restore_requested
            && std::env::var_os("ENLUMI_REPLAY_MINIMIZE").is_some()
        {
            self.minimized = true;
            surface.set_minimized(true);
        }
    }
}
impl AppHandler for Replay {
    fn surfaces_available(&mut self, cx: &dyn PlatformCx) {
        self.inner.surfaces_available(cx);
    }
    fn surfaces_lost(&mut self, cx: &dyn PlatformCx) {
        self.inner.surfaces_lost(cx);
    }
    fn surface_event(&mut self, cx: &dyn PlatformCx, id: SurfaceId, event: SurfaceEvent) {
        self.surface = Some(id);
        if self.restore_requested
            && matches!(
                event,
                SurfaceEvent::Focused(true) | SurfaceEvent::Occluded(false)
            )
        {
            self.minimized = false;
        }
        self.inner.surface_event(cx, id, event);
    }
    fn wake(&mut self, cx: &dyn PlatformCx, reason: WakeReason) {
        self.inner.wake(cx, reason);
    }
    fn idle(&mut self, cx: &dyn PlatformCx) -> IdlePolicy {
        self.inner.idle(cx).merge(IdlePolicy::BlockUntil(self.next))
    }
    fn deadline_reached(&mut self, cx: &dyn PlatformCx) {
        self.inner.deadline_reached(cx);
        if Instant::now() >= self.next {
            self.input(cx);
        }
    }
    fn shutting_down(&mut self, cx: &dyn PlatformCx) {
        eprintln!(
            "Replay max UI timer lateness after 27s: {:.3} ms; restored: {}",
            self.max_late.as_secs_f64() * 1000.,
            self.restore_requested && !self.minimized
        );
        self.inner.shutting_down(cx);
    }
}
