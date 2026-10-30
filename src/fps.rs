//! Continuously sampled UI-frame cadence and opt-in frame-cost recording.
use std::{
    cell::RefCell,
    io::{BufWriter, Write},
    rc::Rc,
    time::Instant,
};
use zgui::{
    prelude::*,
    render::FrameOutcome,
    runtime::{FrameProbe, Window, window::frame::FrameReport},
    vocab::Timestamp,
};

struct Probe {
    log: RefCell<Option<BufWriter<std::fs::File>>>,
    started: Instant,
}
impl FrameProbe for Probe {
    fn frame_ended(&self, _window: &Window) {}
    fn frame_reported(&self, _window: &Window, report: &FrameReport) {
        let stats = match report.outcome {
            FrameOutcome::Presented(stats) => Some(stats),
            _ => None,
        };
        let mut output = self.log.borrow_mut();
        if let Some(log) = output.as_mut() {
            let stats_value = stats.unwrap_or_default();
            let result = writeln!(
                log,
                "{:.3},{},{:.3},{:.3},{},{},{},{}",
                self.started.elapsed().as_secs_f64() * 1000.,
                u8::from(stats.is_some()),
                report.elapsed.as_secs_f64() * 1000.,
                report.paint_elapsed.as_secs_f64() * 1000.,
                report.restyled,
                stats_value.draw_calls,
                stats_value.bytes_uploaded,
                stats_value.damage_px
            )
            .and_then(|()| log.flush());
            if result.is_err() {
                *output = None;
            }
        }
    }
}

pub fn probe() -> Rc<dyn FrameProbe> {
    let log = std::env::var_os("ENLUMI_PERF_LOG").and_then(|path| {
        match std::fs::File::create(&path).and_then(|file| {
            let mut log=BufWriter::new(file);
            writeln!(log,"at_ms,presented,frame_ms,paint_submit_ms,restyled,draw_calls,uploaded_bytes,damage_px")?;
            Ok(log)
        }) {
            Ok(log)=>Some(log),
            Err(error)=>{eprintln!("Cannot create performance log {}: {error}",std::path::Path::new(&path).display());None}
        }
    });
    Rc::new(Probe {
        log: RefCell::new(log),
        started: Instant::now(),
    })
}

#[derive(Default)]
struct Meter {
    start: Option<Timestamp>,
    previous: Option<Timestamp>,
    frames: u32,
}
impl Meter {
    fn frame(&mut self, at: Timestamp) -> Option<u32> {
        if self.previous.is_some_and(|previous| at <= previous) {
            return None;
        }
        self.previous = Some(at);
        let start = *self.start.get_or_insert(at);
        if at <= start {
            return None;
        }
        self.frames += 1;
        let elapsed = at.saturating_since(start).as_secs_f64();
        if elapsed < 0.25 {
            return None;
        }
        let fps = (f64::from(self.frames) / elapsed).round().clamp(0., 999.) as u32;
        self.start = Some(at);
        self.frames = 0;
        Some(fps)
    }
}

#[component]
pub fn Fps() -> impl IntoView {
    let fps = RwSignal::new_local(None::<u32>);
    let meter = StoredValue::new_local(RefCell::new(Meter::default()));
    let frame = StoredValue::new_local(None::<zgui::view::FrameHandle>);
    sample(
        Timers::current().expect("FPS window clock"),
        meter,
        frame,
        fps,
    );
    on_cleanup_local(move || frame.set_value(None));
    view! {
        text(class="fps-readout",a11y:label="UI frames per second") {{move ||fps.get().map_or_else(||"--".into(),|value|value.to_string())}}
        text(class="fps-label",a11y:hidden=true) {"FPS"}
    }
}
fn sample(
    clock: Timers,
    meter: StoredValue<RefCell<Meter>, LocalStorage>,
    frame: StoredValue<Option<zgui::view::FrameHandle>, LocalStorage>,
    fps: RwSignal<Option<u32>, LocalStorage>,
) {
    let next = clock.clone();
    frame.set_value(Some(clock.request_frame(move |at| {
        frame.set_value(None);
        if let Some(value) = meter.with_value(|meter| meter.borrow_mut().frame(at))
            && fps.get_untracked() != Some(value)
        {
            fps.set(Some(value));
        }
        sample(next, meter, frame, fps);
    })));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn measures_continuous_ui_frame_cadence() {
        let mut meter = Meter::default();
        assert_eq!(meter.frame(Timestamp::ORIGIN), None);
        for index in 1..25 {
            let at = Timestamp::from_origin(Duration::from_millis(index * 10));
            assert_eq!(meter.frame(at), None);
            assert_eq!(meter.frame(at), None);
        }
        assert_eq!(
            meter.frame(Timestamp::from_origin(Duration::from_millis(250))),
            Some(100)
        );
        assert_eq!(
            meter.frame(Timestamp::from_origin(Duration::from_millis(500))),
            Some(4)
        );
    }
}
