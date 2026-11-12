//! Opt-in per-frame section timing. With `ENLUMI_PROFILE_LOG` set, the frame
//! probe writes each UI frame's time per section; the rest is "other".
use std::{
    cell::{Cell, RefCell},
    time::Instant,
};

pub const SECTIONS: [&str; 8] = [
    "map.select",
    "map.raster",
    "map.demand",
    "map.scene",
    "map.render",
    "map.hover",
    "map.save_view",
    "map.plot_view",
];

thread_local! {
    static ENABLED: Cell<Option<bool>> = const { Cell::new(None) };
    static TOTALS: RefCell<[f64; SECTIONS.len()]> = const { RefCell::new([0.; SECTIONS.len()]) };
}

fn enabled() -> bool {
    ENABLED.with(|enabled| {
        *enabled
            .get()
            .get_or_insert_with(|| std::env::var_os("ENLUMI_PROFILE_LOG").is_some())
    })
}

/// Adds the time until drop to `name`. Free when profiling is off.
pub struct Section(Option<(usize, Instant)>);
impl Drop for Section {
    fn drop(&mut self) {
        if let Some((index, started)) = self.0 {
            let ms = started.elapsed().as_secs_f64() * 1000.;
            TOTALS.with(|totals| totals.borrow_mut()[index] += ms);
        }
    }
}
pub fn section(name: &'static str) -> Section {
    Section(enabled().then(|| {
        let index = SECTIONS
            .iter()
            .position(|known| *known == name)
            .expect("profile section must be listed in SECTIONS");
        (index, Instant::now())
    }))
}

/// Milliseconds per section since the last call.
pub fn take() -> [f64; SECTIONS.len()] {
    TOTALS.with(|totals| std::mem::take(&mut *totals.borrow_mut()))
}
