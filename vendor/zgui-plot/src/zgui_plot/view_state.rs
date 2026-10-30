//! Persistent per-kind view options; data identity supplied by application.
use crate::{Range, Ranges};
use std::rc::Rc;
use zgui::prelude::*;

pub struct ViewState<T: 'static> {
    pub range: RwSignal<Range, LocalStorage>,
    pub viewport: RwSignal<Option<Ranges>, LocalStorage>,
    pub locked_y: RwSignal<Option<Range>, LocalStorage>,
    pub bins: RwSignal<usize, LocalStorage>,
    pub limits: RwSignal<bool, LocalStorage>,
    pub fail_regions: RwSignal<bool, LocalStorage>,
    pub cdf: RwSignal<bool, LocalStorage>,
    pub ccdf: RwSignal<bool, LocalStorage>,
    pub fit: RwSignal<bool, LocalStorage>,
    pub source: StoredValue<Vec<Rc<Vec<T>>>, LocalStorage>,
}
impl<T: 'static> Copy for ViewState<T> {}
impl<T: 'static> Clone for ViewState<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: 'static> Default for ViewState<T> {
    fn default() -> Self {
        Self {
            range: RwSignal::new_local(Range::new(0.0, 1.0)),
            viewport: RwSignal::new_local(None),
            locked_y: RwSignal::new_local(None),
            bins: RwSignal::new_local(80),
            limits: RwSignal::new_local(true),
            fail_regions: RwSignal::new_local(true),
            cdf: RwSignal::new_local(true),
            ccdf: RwSignal::new_local(true),
            fit: RwSignal::new_local(true),
            source: StoredValue::new_local(Vec::new()),
        }
    }
}
impl<T: 'static> ViewState<T> {
    pub fn duplicate(self) -> Self {
        Self {
            range: RwSignal::new_local(self.range.get_untracked()),
            viewport: RwSignal::new_local(self.viewport.get_untracked()),
            locked_y: RwSignal::new_local(self.locked_y.get_untracked()),
            bins: RwSignal::new_local(self.bins.get_untracked()),
            limits: RwSignal::new_local(self.limits.get_untracked()),
            fail_regions: RwSignal::new_local(self.fail_regions.get_untracked()),
            cdf: RwSignal::new_local(self.cdf.get_untracked()),
            ccdf: RwSignal::new_local(self.ccdf.get_untracked()),
            fit: RwSignal::new_local(self.fit.get_untracked()),
            source: StoredValue::new_local(self.source.get_value()),
        }
    }
    pub fn use_source(self, source: Vec<Rc<Vec<T>>>, full: Range) {
        if !self.source.with_value(|old| same_sources(old, &source)) {
            self.source.set_value(source);
            self.range.set(full);
            self.viewport.set(None);
            self.locked_y.set(None);
        }
    }
}
pub fn same_sources<T>(old: &[Rc<Vec<T>>], new: &[Rc<Vec<T>>]) -> bool {
    old.len() == new.len() && old.iter().zip(new).all(|(a, b)| Rc::ptr_eq(a, b))
}
