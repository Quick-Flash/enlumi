use std::cell::RefCell;
use zgui::{prelude::*, vocab::Timestamp};

#[derive(Default)]
struct Meter {
    start: Option<Timestamp>,
    previous: Option<Timestamp>,
    frames: u32,
}
impl Meter {
    fn frame(&mut self, at: Timestamp) -> Option<f64> {
        if self.previous.is_some_and(|previous| at <= previous) {
            return None;
        }
        self.previous = Some(at);
        let start = *self.start.get_or_insert(at);
        if at == start {
            return None;
        }
        self.frames += 1;
        let elapsed = at.saturating_since(start).as_secs_f64();
        if elapsed < 0.25 {
            return None;
        }
        let fps = f64::from(self.frames) / elapsed;
        self.start = Some(at);
        self.frames = 0;
        Some(fps)
    }
}

#[component]
pub fn Fps() -> impl IntoView {
    let fps = RwSignal::new_local(None::<f64>);
    let meter = StoredValue::new_local(RefCell::new(Meter::default()));
    let frame = StoredValue::new_local(None::<zgui::view::FrameHandle>);
    let clock = Timers::current().expect("FPS window clock");
    sample(clock, meter, frame, fps);
    on_cleanup_local(move || frame.set_value(None));
    view! { text(class = "fps-readout", a11y:label = "Frames per second") {{move || fps.get().map_or_else(|| "--".into(), |fps| format!("{:.0}", fps.clamp(0., 999.)))}} }
}
fn sample(
    clock: Timers,
    meter: StoredValue<RefCell<Meter>, LocalStorage>,
    frame: StoredValue<Option<zgui::view::FrameHandle>, LocalStorage>,
    fps: RwSignal<Option<f64>, LocalStorage>,
) {
    let next = clock.clone();
    frame.set_value(Some(clock.request_frame(move |at| {
        frame.set_value(None);
        if let Some(value) = meter.with_value(|meter| meter.borrow_mut().frame(at)) {
            fps.set(Some(value));
        }
        sample(next, meter, frame, fps);
    })));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_frame_intervals_and_ignores_duplicate_timestamps() {
        let mut meter = Meter::default();
        assert_eq!(meter.frame(Timestamp::ORIGIN), None);
        for index in 1..25 {
            let at = Timestamp::from_origin(std::time::Duration::from_millis(index * 10));
            assert_eq!(meter.frame(at), None);
            assert_eq!(meter.frame(at), None);
        }
        assert_eq!(
            meter.frame(Timestamp::from_origin(std::time::Duration::from_millis(
                250
            ))),
            Some(100.0)
        );
        for index in 26..=100 {
            let at = Timestamp::from_origin(std::time::Duration::from_millis(index * 10));
            assert_eq!(
                meter.frame(at),
                if index % 25 == 0 { Some(100.0) } else { None }
            );
            assert_eq!(meter.frame(at), None);
        }
    }
}
