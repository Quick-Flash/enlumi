//! Latest hover position runs once per frame; busy/leave always clear immediately.
use super::PlotHover;
use zgui::prelude::*;

pub(super) fn coalesced(callback: UnsyncCallback<PlotHover>) -> UnsyncCallback<PlotHover> {
    let pending = StoredValue::new_local(None::<(f64, f64)>);
    let frame = StoredValue::new_local(None::<zgui::view::FrameHandle>);
    let clock = Timers::current().expect("Plot hover window clock");
    on_cleanup_local(move || {
        frame.set_value(None);
        pending.set_value(None);
    });
    UnsyncCallback::new(move |event| match event {
        PlotHover::At(position) => {
            pending.set_value(Some(position));
            if frame.with_value(Option::is_none) {
                let next = clock.request_frame(move |_| {
                    frame.set_value(None);
                    let position = pending.get_value();
                    pending.set_value(None);
                    if let Some(position) = position {
                        callback.run(PlotHover::At(position));
                    }
                });
                frame.set_value(Some(next));
            }
        }
        PlotHover::Busy | PlotHover::Left => {
            frame.set_value(None);
            pending.set_value(None);
            callback.run(event);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc, time::Duration};
    use zgui::render::Renderer;
    #[test]
    fn hover_bursts_keep_latest_position_and_cancel_on_busy_or_leave() {
        let built = Rc::new(RefCell::new(None));
        let model = Rc::clone(&built);
        let events = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&events);
        let handler = app()
            .with_size(300., 200.)
            .with_renderer(Box::new(|_, target| {
                let mut renderer = zgui_testkit_scene::CaptureRenderer::new();
                renderer.configure(target);
                Ok(Box::new(renderer))
            }))
            .into_handler(move || {
                let recorded = Rc::clone(&recorded);
                *model.borrow_mut() = Some(coalesced(UnsyncCallback::new(move |event| {
                    recorded.borrow_mut().push(event)
                })));
                view! { box {} }
            })
            .unwrap();
        let mut harness = zgui_platform_headless::Harness::new(handler.into_runtime());
        harness.settle(16);
        let hover = built.borrow().unwrap();
        for index in 0..100 {
            hover.run(PlotHover::At((index as f64, 2.)));
        }
        assert!(events.borrow().is_empty());
        harness.advance(Duration::from_millis(16));
        harness.pump();
        harness.settle(16);
        assert_eq!(*events.borrow(), vec![PlotHover::At((99., 2.))]);
        hover.run(PlotHover::At((200., 3.)));
        hover.run(PlotHover::Busy);
        assert_eq!(events.borrow().last(), Some(&PlotHover::Busy));
        harness.advance(Duration::from_millis(16));
        harness.pump();
        assert_eq!(
            events.borrow().len(),
            2,
            "Cancelled hover cannot restore readout during drag"
        );
        hover.run(PlotHover::At((300., 4.)));
        hover.run(PlotHover::Left);
        harness.advance(Duration::from_millis(16));
        harness.pump();
        assert_eq!(events.borrow().len(), 3);
        assert_eq!(events.borrow().last(), Some(&PlotHover::Left));
        hover.run(PlotHover::At((400., 5.)));
        harness.advance(Duration::from_millis(16));
        harness.pump();
        harness.settle(16);
        assert_eq!(events.borrow().last(), Some(&PlotHover::At((400., 5.))));
    }
}
