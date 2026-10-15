use crate::state::Workbench;
use zgui::prelude::*;

pub use plot::zgui_series::LegendDrag;
impl Workbench {
    pub fn can_place_line(self, source: u64, id: u64, target: u64) -> bool {
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
            remove: UnsyncCallback::new(move |(pane, id)| {
                if let Some(pane) = self.find(pane) {
                    pane.remove_line(id);
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
            panes: Signal::derive_local(move || {
                self.panes.get().iter().map(|pane| pane.display()).collect()
            }),
            visible: UnsyncCallback::new(move |id| {
                plot::workspace::plot_visible(
                    id,
                    self.maximized.get_untracked(),
                    self.layout.get_untracked(),
                )
            }),
            hit_plot: UnsyncCallback::new(move |(x, y)| self.hit_plot(x, y)),
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
            let global = self.panes.with_untracked(|panes| {
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
