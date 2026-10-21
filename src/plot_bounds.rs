use crate::state::Pane;
use plot::interaction::Axes;
use zgui::prelude::*;

impl Pane {
    pub fn fit_axes(self, axes: Axes) {
        if let Some(map) = self
            .analysis
            .with_untracked(|result| result.as_ref().and_then(|result| result.heatmap.clone()))
        {
            let current = self.plot.ranges.get_untracked();
            let _ = self.plot.set_view(plot::Ranges {
                x: if axes.x {
                    (map.x_low, map.x_high).into()
                } else {
                    current.x
                },
                y: if axes.y {
                    (map.y_low, map.y_high).into()
                } else {
                    current.y
                },
            });
            return;
        }

        self.plot.fit_axes(
            &plot::plot_data::PlotData::Time(self.series.get_untracked()),
            axes,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Line, Workbench};

    #[test]
    fn incoming_line_fits_destination_and_keeps_source_and_reorders_fixed() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let target = workbench.pane();
        let source = window
            .scope
            .with(|| workbench.add_plot(target.id, false, false));
        let line = |id, x, y| Line {
            id,
            source: id.to_string(),
            name: id.to_string(),
            revision: 0,
            data: plot::time_series::SeriesData::new(vec![
                plot::PlotPoint { x, y },
                plot::PlotPoint {
                    x: x + 1.0,
                    y: y + 1.0,
                },
            ])
            .unwrap(),
        };
        target.lines.set(vec![line(100, 0.0, 1.0)]);
        source.lines.set(vec![line(200, 100.0, 200.0)]);
        let zoom = plot::Ranges::new((10.0, 20.0), (30.0, 40.0));
        target.plot.set_view(zoom).unwrap();
        source.plot.set_view(zoom).unwrap();
        workbench.place_line(source.id, 200, target.id, None);
        let fitted = target.plot.ranges.get_untracked();
        assert!(fitted.x.min <= 0.0 && fitted.x.max >= 101.0);
        assert!(fitted.y.min <= 1.0 && fitted.y.max >= 201.0);
        assert_eq!(source.plot.ranges.get_untracked(), zoom);
        assert!(!target.plot.auto_fit.get_untracked());
        target.plot.set_view(zoom).unwrap();
        workbench.place_line(target.id, 200, target.id, Some(100));
        assert_eq!(target.plot.ranges.get_untracked(), zoom);
    }
}
