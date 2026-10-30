use crate::{
    line_draw::LineDraw,
    time_series::{SeriesData, TimeSeries, TimeSeriesSet},
    zgui_plot::PlotState,
};
use std::collections::BTreeMap;
use zgui::prelude::*;

/// Plot-facing identity. Consumer retains source metadata separately.
#[derive(Clone)]
pub struct SeriesLine {
    pub id: u64,
    pub name: String,
    pub data: SeriesData,
    pub revision: u64,
}
impl SeriesLine {
    pub fn label(&self) -> String {
        self.name.clone()
    }
}

/// Durable presentation over caller-owned data. Membership changes use callbacks.
#[derive(Clone, Copy)]
pub struct SeriesPane {
    pub id: u64,
    pub lines: Signal<Vec<SeriesLine>, LocalStorage>,
    pub links: RwSignal<crate::zgui_link::LinkOptions, LocalStorage>,
    pub selected: RwSignal<Option<u64>, LocalStorage>,
    pub drawing: RwSignal<BTreeMap<u64, LineDraw>, LocalStorage>,
    pub plot: PlotState,
    pub series: Signal<TimeSeriesSet, LocalStorage>,
}
impl SeriesPane {
    pub fn new(id: u64, lines: Signal<Vec<SeriesLine>, LocalStorage>) -> Self {
        let series = Signal::derive_local(move || {
            TimeSeriesSet::new(
                lines
                    .get()
                    .iter()
                    .map(|line| {
                        TimeSeries::new(line.id.to_string(), line.label(), line.data.clone())
                    })
                    .collect(),
            )
            .expect("unique series IDs")
        });
        Self {
            id,
            lines,
            series,
            links: RwSignal::new_local(crate::zgui_link::LinkOptions {
                x: false,
                y: false,
                pointer: true,
            }),
            selected: RwSignal::new_local(None),
            drawing: RwSignal::new_local(BTreeMap::new()),
            plot: PlotState::default(),
        }
    }
    pub fn colors(self) -> Vec<[u8; 4]> {
        self.lines.with_untracked(|lines| {
            self.plot.styles.with_untracked(|styles| {
                lines
                    .iter()
                    .map(|line| {
                        let id = line.id.to_string();
                        styles
                            .get(&id)
                            .copied()
                            .unwrap_or_else(|| crate::plot_data::SeriesStyle::for_id(&id))
                            .color
                    })
                    .collect()
            })
        })
    }
    pub fn remove_appearance(self, id: u64) {
        self.plot.auto_fit.set(false);
        self.plot.hidden.update(|hidden| {
            hidden.remove(&id.to_string());
        });
        self.plot.styles.update(|styles| {
            styles.remove(&id.to_string());
        });
        self.drawing.update(|drawing| {
            drawing.remove(&id);
        });
        self.plot.hover.set(None);
        if self.plot.emphasized.get_untracked().as_deref() == Some(&id.to_string()) {
            self.plot.emphasized.set(None);
        }
        if self.selected.get_untracked() == Some(id) {
            self.selected.set(
                self.lines
                    .with_untracked(|lines| lines.first().map(|line| line.id)),
            );
        }
    }
    /// Copy presentation before consumer transfers membership. Source cleanup stays explicit.
    pub fn copy_appearance(self, target: Self, id: u64, global_colors: &[[u8; 4]]) {
        let key = id.to_string();
        let mut style = self.plot.styles.with_untracked(|styles| {
            styles
                .get(&key)
                .copied()
                .unwrap_or_else(|| crate::plot_data::SeriesStyle::for_id(&key))
        });
        let local = target.colors();
        if local.iter().any(|color| color[..3] == style.color[..3]) {
            style.color = crate::palette::next_color(&local, global_colors);
        }
        target.plot.styles.update(|styles| {
            styles.insert(key.clone(), style);
        });
        let drawing = self
            .drawing
            .with_untracked(|drawing| drawing.get(&id).copied().unwrap_or_default());
        target.drawing.update(|settings| {
            settings.insert(id, drawing);
        });
        let hidden = self
            .plot
            .hidden
            .with_untracked(|hidden| hidden.contains(&key));
        target.plot.hidden.update(|values| {
            if hidden {
                values.insert(key.clone());
            } else {
                values.remove(&key);
            }
        });
        target.plot.hover.set(None);
    }
}

/// Consumer-owned membership and extra actions. Controls never know ULog fields.
#[derive(Clone, Copy)]
pub struct SeriesActions {
    pub select: UnsyncCallback<(u64, u64)>,
    pub remove: UnsyncCallback<(u64, u64)>,
    pub move_line: UnsyncCallback<(u64, u64, u64, Option<u64>)>,
    pub extra: Option<UnsyncCallback<u64>>,
    pub extra_label: &'static str,
    pub extra_class: &'static str,
    pub extra_help: &'static str,
}
impl Default for SeriesActions {
    fn default() -> Self {
        Self {
            select: UnsyncCallback::new(|_| {}),
            remove: UnsyncCallback::new(|_| {}),
            move_line: UnsyncCallback::new(|_| {}),
            extra: None,
            extra_label: "",
            extra_class: "",
            extra_help: "",
        }
    }
}
