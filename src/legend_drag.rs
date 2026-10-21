use crate::state::{Pane, Workbench};
use zgui::prelude::*;

pub use plot::zgui_series::LegendDrag;
impl Workbench {
    // Field IDs grow from zero; complemented geometry IDs occupy upper half.
    pub fn legend_keys(self, pane: Pane) -> Vec<u64> {
        let mut keys = pane
            .lines
            .with(|lines| lines.iter().map(|line| line.id).collect::<Vec<_>>());
        keys.extend(
            pane.geometry
                .with(|ids| ids.iter().map(|id| !id).collect::<Vec<_>>()),
        );
        let order = pane.legend_order.get();
        keys.sort_by_key(|key| order.iter().position(|id| id == key).unwrap_or(usize::MAX));
        keys
    }
    pub fn ordered_geometry(self, pane: Pane) -> Vec<crate::geometry::Geometry> {
        let keys = self.legend_keys(pane);
        self.geometry.items.with(|items| {
            keys.into_iter()
                .filter(|key| *key >= (1_u64 << 63))
                .filter_map(|key| items.iter().find(|item| item.id == !key).cloned())
                .collect()
        })
    }
    fn order_legend(self, pane: Pane, id: u64, before: Option<u64>) {
        if before == Some(id) {
            return;
        }
        let mut keys = self.legend_keys(pane);
        keys.retain(|key| *key != id);
        let index = before
            .and_then(|key| keys.iter().position(|id| *id == key))
            .unwrap_or(keys.len());
        keys.insert(index, id);
        pane.legend_order.set(keys.clone());
        pane.lines.update(|lines| {
            lines.sort_by_key(|line| {
                keys.iter()
                    .position(|key| *key == line.id)
                    .unwrap_or(usize::MAX)
            })
        });
    }

    pub fn can_place_line(self, source: u64, id: u64, target: u64) -> bool {
        if source != target
            && self
                .find(source)
                .zip(self.find(target))
                .is_none_or(|(source, target)| {
                    source
                        .analysis
                        .with_untracked(|result| result.as_ref().map(|result| result.mode))
                        != target
                            .analysis
                            .with_untracked(|result| result.as_ref().map(|result| result.mode))
                })
        {
            return false;
        }
        if id >= (1_u64 << 63) {
            return self
                .find(source)
                .zip(self.find(target))
                .is_some_and(|(source, target)| {
                    source.geometry.with_untracked(|ids| ids.contains(&!id))
                        && (source.id == target.id
                            || !target.geometry.with_untracked(|ids| ids.contains(&!id)))
                });
        }
        source == target
            || self
                .find(source)
                .zip(self.find(target))
                .is_some_and(|(source, target)| {
                    source.lines.with_untracked(|lines| {
                        lines.iter().find(|line| line.id == id).is_some_and(|line| {
                            target.lines.with_untracked(|lines| {
                                !lines.iter().any(|other| other.source == line.source)
                            })
                        })
                    })
                })
    }
    pub fn series_actions(self) -> plot::zgui_series::SeriesActions {
        plot::zgui_series::SeriesActions {
            select: UnsyncCallback::new(move |(pane, id)| self.select(pane, id)),
            remove: UnsyncCallback::new(move |(pane, id): (u64, u64)| {
                if let Some(pane) = self.find(pane) {
                    if id >= (1_u64 << 63) {
                        self.remove_geometry_from(pane, !id);
                    } else {
                        pane.remove_line(id);
                    }
                }
            }),
            move_line: UnsyncCallback::new(move |(source, id, target, before)| {
                self.place_line(source, id, target, before)
            }),
            extra: Some(UnsyncCallback::new(move |pane| {
                if let Some(pane) = self.find(pane) {
                    self.filter_legend(pane);
                }
            })),
            extra_label: "Add filters",
            extra_class: "legend-add-filters",
            extra_help: "Add filters opens a fresh batch with every field in this legend.",
        }
    }
    pub fn legend_host(self) -> plot::zgui_series::LegendHost {
        plot::zgui_series::LegendHost {
            drag: self.line_drag,
            order: Some(UnsyncCallback::new(move |id| {
                self.find(id)
                    .map_or_else(Vec::new, |pane| self.legend_keys(pane))
            })),
            panes: Signal::derive_local(move || {
                self.panes()
                    .get()
                    .iter()
                    .map(|pane| pane.display())
                    .collect()
            }),
            visible: UnsyncCallback::new(move |id| {
                plot::workspace::plot_visible(
                    id,
                    self.maximized().get_untracked(),
                    self.layout().get_untracked(),
                )
            }),
            hit_plot: UnsyncCallback::new(move |(x, y)| self.hit_any_plot(x, y)),
            accepts: UnsyncCallback::new(move |(source, id, target)| {
                self.can_place_line(source, id, target)
            }),
            actions: self.series_actions(),
        }
    }
    pub fn place_line(self, source: u64, id: u64, target: u64, before: Option<u64>) {
        let (Some(source), Some(target)) = (self.find(source), self.find(target)) else {
            return;
        };
        if !self.can_place_line(source.id, id, target.id) {
            return;
        }
        if id >= (1_u64 << 63) {
            if source.id != target.id {
                source.geometry.update(|ids| {
                    ids.remove(&!id);
                });
                target.geometry.update(|ids| {
                    ids.insert(!id);
                });
                let hidden = source
                    .hidden_geometry
                    .with_untracked(|ids| ids.contains(&!id));
                source.hidden_geometry.update(|ids| {
                    ids.remove(&!id);
                });
                target.hidden_geometry.update(|ids| {
                    if hidden {
                        ids.insert(!id);
                    } else {
                        ids.remove(&!id);
                    }
                });
            }
            self.order_legend(target, id, before);
            self.activate(target.id);
            return;
        }
        // Snapshot current order before line membership changes.
        source.legend_order.set(self.legend_keys(source));
        target.legend_order.set(self.legend_keys(target));
        let Some(mut line) = source
            .lines
            .with_untracked(|lines| lines.iter().find(|line| line.id == id).cloned())
        else {
            return;
        };
        if source.id != target.id
            && target
                .lines
                .with_untracked(|lines| lines.iter().any(|other| other.source == line.source))
        {
            return;
        }
        source.plot.auto_fit.set(false);
        target.plot.auto_fit.set(false);
        if source.id == target.id {
            if before == Some(id) {
                return;
            }
            source.lines.update(|lines| {
                lines.retain(|line| line.id != id);
                let index = before
                    .and_then(|id| lines.iter().position(|line| line.id == id))
                    .unwrap_or(lines.len());
                lines.insert(index, line);
            });
        } else {
            let global = self.panes().with_untracked(|panes| {
                panes
                    .iter()
                    .flat_map(|pane| pane.display().colors())
                    .collect::<Vec<_>>()
            });
            source
                .display()
                .copy_appearance(target.display(), id, &global);
            line.revision += 1;
            source.remove_line(id);
            target.lines.update(|lines| {
                let index = before
                    .and_then(|id| lines.iter().position(|line| line.id == id))
                    .unwrap_or(lines.len());
                lines.insert(index, line);
            });
            target.fit_axes(plot::interaction::Axes::BOTH);
        }
        self.order_legend(target, id, before);
        self.select(target.id, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Line;
    use plot::plot_data::SeriesStyle;

    #[test]
    fn cross_plot_drop_rejects_duplicate_source_and_recolors_conflicting_line() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let source = workbench.pane();
        let target = window
            .scope
            .with(|| workbench.add_plot(source.id, false, false));
        let data = plot::time_series::SeriesData::new(vec![
            plot::PlotPoint { x: 0.0, y: 1.0 },
            plot::PlotPoint { x: 1.0, y: 2.0 },
        ])
        .unwrap();
        let line = |id| Line {
            id,
            source: "same field".into(),
            name: "same field".into(),
            data: data.clone(),
            revision: 0,
        };
        source.lines.set(vec![line(1)]);
        target.lines.set(vec![line(2)]);
        let ranges = plot::Ranges::new((5.0, 10.0), (-3.0, 7.0));
        target.plot.set_view(ranges).unwrap();
        source.plot.auto_fit.set(true);
        workbench.place_line(source.id, 1, target.id, None);
        assert_eq!(source.lines.get_untracked().len(), 1);
        assert_eq!(target.lines.get_untracked().len(), 1);
        assert_eq!(target.plot.ranges.get_untracked(), ranges);
        assert!(source.plot.auto_fit.get_untracked());
        source
            .lines
            .update(|lines| lines[0].source = "different field".into());
        let mut style = SeriesStyle::for_id("1");
        style.color = [78, 180, 255, 255];
        style.width = 4.0;
        source.plot.styles.update(|styles| {
            styles.insert("1".into(), style);
        });
        target.plot.styles.update(|styles| {
            styles.insert("2".into(), style);
        });
        workbench.place_line(source.id, 1, target.id, None);
        assert!(source.lines.get_untracked().is_empty());
        assert_eq!(
            target
                .lines
                .with_untracked(|lines| lines.iter().map(|line| line.id).collect::<Vec<_>>()),
            [2, 1]
        );
        let moved = target.plot.style("1");
        assert_ne!(moved.color, style.color);
        assert_eq!(moved.width, style.width);
        assert_eq!(target.plot.style("2"), style);
    }
}
