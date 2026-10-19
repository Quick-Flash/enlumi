use crate::state::{Pane, Workbench};
use plot::{PlotGeometry, PlotPoint};
use zgui::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    pub pane: u64,
    pub line: u64,
    pub start: PlotPoint,
    pub end: PlotPoint,
}
impl Measurement {
    pub fn readout(self, frequency_axis: bool) -> String {
        let span = (self.end.x - self.start.x).abs();
        if span == 0. {
            return "Drag to another sample".into();
        }
        if frequency_axis {
            format!(
                "Δf {} Hz · 1/Δf {} s",
                plot::format_number(span),
                plot::format_number(1. / span)
            )
        } else {
            format!(
                "Δt {} s · f {} Hz",
                plot::format_number(span),
                plot::format_number(1. / span)
            )
        }
    }
}

#[derive(Clone, Copy)]
pub struct MeasurementUi {
    pub active: RwSignal<bool, LocalStorage>,
    pub selection: RwSignal<Option<Measurement>, LocalStorage>,
}
impl MeasurementUi {
    pub fn new() -> Self {
        Self {
            active: RwSignal::new_local(false),
            selection: RwSignal::new_local(None),
        }
    }
    pub fn clear(self) {
        self.active.set(false);
        self.selection.set(None);
    }
}

impl Workbench {
    pub fn measurement_shortcut(
        self,
        event: &zgui::vocab::KeyEvent,
        modifiers: zgui::vocab::Modifiers,
    ) -> bool {
        // Leave active measurement and its pointer capture alone until release.
        if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Escape)
            && self.measurement.active.get_untracked()
        {
            return true;
        }
        if event.repeat
            || modifiers.control()
            || modifiers.alt()
            || modifiers.meta()
            || zgui::view::focus_is_text_entry()
        {
            return false;
        }
        if matches!(&event.key_without_modifiers,zgui::vocab::Key::Character(key) if key.as_str().eq_ignore_ascii_case("m"))
        {
            self.finish_geometry_editing();
            self.analysis.picking_range.set(false);
            self.measurement.active.update(|active| *active = !*active);
            self.measurement.selection.set(None);
            true
        } else {
            false
        }
    }
}

/// Pick initial line by screen distance; drag along that line by X alone.
/// Sorted samples bound both searches, including logs with invalid values.
pub fn snap(
    pane: Pane,
    target: PlotPoint,
    geometry: PlotGeometry,
    line: Option<u64>,
) -> Option<(u64, PlotPoint)> {
    let ranges = pane.plot.ranges.get_untracked();
    let pixels = (geometry.data_width(), geometry.data_height());
    let hidden = pane.plot.hidden.get_untracked();
    if let Some(id) = line {
        if hidden.contains(&id.to_string()) {
            return None;
        }
        return pane.lines.with_untracked(|lines| {
            let samples = lines.iter().find(|line| line.id == id)?.data.samples();
            let at = samples.partition_point(|sample| sample.x < target.x);
            let valid = |sample: &&PlotPoint| sample.y.is_finite() && ranges.x.contains(sample.x);
            let left = samples[..at].iter().rev().find(valid);
            let right = samples[at..].iter().find(valid);
            left.into_iter()
                .chain(right)
                .min_by(|a, b| (a.x - target.x).abs().total_cmp(&(b.x - target.x).abs()))
                .map(|point| (id, *point))
        });
    }
    let limit = pixels.0.hypot(pixels.1);
    let mut radius = 12_f64.min(limit);
    loop {
        let closest = pane.lines.with_untracked(|lines| {
            lines
                .iter()
                .filter(|item| {
                    line.is_none_or(|id| item.id == id) && !hidden.contains(&item.id.to_string())
                })
                .filter_map(|item| {
                    let index = item.data.nearest(target, ranges, pixels, radius)?;
                    let point = item.data.samples()[index];
                    let dx = (point.x - target.x) / ranges.x.span() * pixels.0;
                    let dy = (point.y - target.y) / ranges.y.span() * pixels.1;
                    Some((dx * dx + dy * dy, item.id, point))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, id, point)| (id, point))
        });
        if closest.is_some() || radius >= limit {
            return closest;
        }
        radius = (radius * 2.).min(limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measurement_snaps_visible_samples_and_formats_time_and_frequency_domains() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let pane = workbench.pane();
        pane.plot
            .set_view(plot::Ranges::new((0., 4.), (0., 2.)))
            .unwrap();
        pane.lines.set(vec![
            crate::state::Line {
                id: 1,
                source: "wave".into(),
                name: "Wave".into(),
                revision: 0,
                data: plot::time_series::SeriesData::new(vec![
                    PlotPoint { x: 1., y: 1. },
                    PlotPoint { x: 2., y: f64::NAN },
                    PlotPoint { x: 3., y: 1. },
                ])
                .unwrap(),
            },
            crate::state::Line {
                id: 2,
                source: "hidden".into(),
                name: "Hidden".into(),
                revision: 0,
                data: plot::time_series::SeriesData::new(vec![PlotPoint { x: 2., y: 1. }]).unwrap(),
            },
        ]);
        pane.plot
            .hidden
            .set(std::collections::BTreeSet::from(["2".into()]));
        let geometry = PlotGeometry::with_bounds(0., 0., 400., 200., 400., 200.).unwrap();
        let hit = snap(pane, PlotPoint { x: 2., y: 1. }, geometry, None).unwrap();
        assert_eq!(hit.0, 1);
        assert!(hit.1.y.is_finite());
        assert!(snap(pane, PlotPoint { x: 2., y: 1. }, geometry, Some(2)).is_none());
        let measurement = Measurement {
            pane: pane.id,
            line: 1,
            start: PlotPoint { x: 0.75, y: 1. },
            end: PlotPoint { x: 0.25, y: 1. },
        };
        assert!(measurement.readout(false).contains("2 Hz"));
        assert!(measurement.readout(true).contains("2 s"));
        assert_eq!(
            Measurement {
                end: measurement.start,
                ..measurement
            }
            .readout(false),
            "Drag to another sample"
        );
    }
}
