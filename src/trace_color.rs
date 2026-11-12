use serde::{Deserialize, Serialize};
use zgui::prelude::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Solid,
    Speed,
    Height,
    /// The map's color field (`Spec::color_source`), aligned to each sample.
    Field,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gradient {
    pub range: [f64; 2],
    pub colors: [[u8; 3]; 2],
}
impl Gradient {
    pub fn valid(self) -> bool {
        self.range.iter().all(|v| v.is_finite()) && self.range[1] > self.range[0]
    }
    pub fn color(self, fraction: f64) -> [u8; 3] {
        std::array::from_fn(|i| {
            (f64::from(self.colors[0][i])
                + (f64::from(self.colors[1][i]) - f64::from(self.colors[0][i]))
                    * fraction.clamp(0., 1.))
            .round() as u8
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TraceColor {
    pub mode: Mode,
    pub speed: Gradient,
    pub height: Gradient,
    /// Empty range (`[0, 0]`) means the field's own data range.
    pub field: Gradient,
}
impl Default for TraceColor {
    fn default() -> Self {
        Self {
            mode: Mode::Solid,
            speed: Gradient {
                range: [0., 30.],
                colors: [[45, 110, 230], [240, 80, 60]],
            },
            height: Gradient {
                range: [0., 100.],
                colors: [[50, 180, 100], [245, 210, 60]],
            },
            field: Gradient {
                range: [0., 0.],
                colors: [[60, 90, 200], [245, 200, 60]],
            },
        }
    }
}
impl TraceColor {
    pub fn gradient(self) -> Option<Gradient> {
        match self.mode {
            Mode::Solid => None,
            Mode::Speed => Some(self.speed),
            Mode::Height => Some(self.height),
            Mode::Field => Some(self.field),
        }
    }
    pub fn gradient_mut(&mut self) -> Option<&mut Gradient> {
        match self.mode {
            Mode::Solid => None,
            Mode::Speed => Some(&mut self.speed),
            Mode::Height => Some(&mut self.height),
            Mode::Field => Some(&mut self.field),
        }
    }
    pub fn valid(self) -> bool {
        self.speed.valid()
            && self.height.valid()
            && (self.field.valid() || self.field.range == [0., 0.])
    }
}
pub fn selected(pane: crate::state::Pane) -> Option<u64> {
    let selected = pane.selected.get();
    pane.view_lines.with(|lines| {
        lines
            .iter()
            .find(|line| Some(line.id) == selected)
            .or_else(|| lines.first())
            .map(|line| line.id)
    })
}
pub fn current(pane: crate::state::Pane, id: Option<u64>) -> TraceColor {
    use zgui::prelude::*;
    let styles = pane.trace_colors.get();
    pane.trajectory
        .with(|spec| {
            spec.as_ref()
                .zip(id)
                .map(|(spec, id)| crate::mapping::telemetry::style_for(spec, &styles, id))
        })
        .unwrap_or_else(|| {
            id.and_then(|id| styles.get(&id).copied())
                .unwrap_or_default()
        })
}
pub fn change(
    pane: crate::state::Pane,
    id: Option<u64>,
    edit: impl FnOnce(&mut TraceColor),
) -> Result<(), String> {
    use zgui::prelude::*;
    let Some(id) = id else {
        return Err("Choose a trace".into());
    };
    let mut color = reactive_graph::graph::untrack(|| current(pane, Some(id)));
    edit(&mut color);
    if !color.valid() {
        return Err("Limits must be finite; upper limit must exceed lower limit".into());
    }
    pane.trace_colors.update(|colors| {
        colors.insert(id, color);
    });
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chosen_colors_clamp_and_ranges_require_finite_increasing_limits() {
        let mut color = TraceColor::default();
        color.speed.colors = [[0, 20, 40], [200, 120, 240]];
        assert_eq!(color.speed.color(-2.), [0, 20, 40]);
        assert_eq!(color.speed.color(0.5), [100, 70, 140]);
        assert_eq!(color.speed.color(2.), [200, 120, 240]);
        for range in [[1., 1.], [2., 1.], [f64::NAN, 2.], [0., f64::INFINITY]] {
            color.speed.range = range;
            assert!(!color.valid());
        }
    }
    #[test]
    fn mode_switch_preserves_independent_gradients_and_serialization() {
        let mut color = TraceColor {
            mode: Mode::Speed,
            ..Default::default()
        };
        color.gradient_mut().unwrap().range = [5., 50.];
        color.mode = Mode::Height;
        color.gradient_mut().unwrap().range = [100., 500.];
        color.height.colors = [[1, 2, 3], [4, 5, 6]];
        color.mode = Mode::Solid;
        assert_eq!(color.gradient(), None);
        assert_eq!(color.speed.range, [5., 50.]);
        assert_eq!(
            serde_json::from_str::<TraceColor>(&serde_json::to_string(&color).unwrap()).unwrap(),
            color
        );
    }
}
