//! Ground-distance rulers. Perspective scale is measured at the focus plane.
use zgui::prelude::*;

#[derive(Clone, Debug, PartialEq)]
struct Ruler {
    label: String,
    pixels: f64,
}
impl Ruler {
    fn new(metres_per_pixel: f64, unit_metres: f64, unit: &str) -> Self {
        let target = 120. * metres_per_pixel / unit_metres;
        let power = 10_f64.powf(target.log10().floor());
        let fraction = target / power;
        let value = power
            * if fraction >= 5. {
                5.
            } else if fraction >= 2. {
                2.
            } else {
                1.
            };
        Self {
            label: format!("{value} {unit}"),
            pixels: value * unit_metres / metres_per_pixel,
        }
    }
}

pub fn scale_bar(
    read: impl Fn() -> Option<f64> + Copy + Send + Sync + 'static,
    terrain: bool,
) -> impl IntoView {
    let rulers = Memo::new(move |_| {
        let metres = read().filter(|m| m.is_finite() && *m > 0.)?;
        let metric = if metres * 120. >= 1000. {
            Ruler::new(metres, 1000., "km")
        } else {
            Ruler::new(metres, 1., "m")
        };
        let imperial = if metres * 120. >= 1609.344 {
            Ruler::new(metres, 1609.344, "mi")
        } else {
            Ruler::new(metres, 0.3048, "ft")
        };
        Some([metric, imperial])
    });
    view! {
        if move ||rulers.get().is_some() {
            column(class="map-distance-scale",a11y:label="Map distance scale") {
                if move ||terrain {text(class="map-scale-reference") {"At map centre"}}
                for index in move ||0..2,key=|index|*index {
                    column(class="map-scale-row") {
                        text(class="map-scale-label") {{move ||rulers.get().map(|r|r[index].label.clone()).unwrap_or_default()}}
                        box(class="map-scale-rule",style:width=move ||rulers.get().map(|r|format!("{}px",r[index].pixels))) {}
                    }
                }
            }
        }
    }
}

pub(super) fn svg_rulers(metres: f64, height: f64) -> String {
    if !metres.is_finite() || metres <= 0. {
        return String::new();
    }
    let metric = if metres * 120. >= 1000. {
        Ruler::new(metres, 1000., "km")
    } else {
        Ruler::new(metres, 1., "m")
    };
    let imperial = if metres * 120. >= 1609.344 {
        Ruler::new(metres, 1609.344, "mi")
    } else {
        Ruler::new(metres, 0.3048, "ft")
    };
    let mut svg = format!(
        "<g transform=\"translate(8 {})\"><rect width=\"144\" height=\"64\" fill=\"#151b26\" fill-opacity=\"0.87\"/>",
        height - 72.
    );
    use std::fmt::Write;
    for (i, ruler) in [metric, imperial].iter().enumerate() {
        let y = 18. + i as f64 * 28.;
        let bottom = y + 8.;
        let right = 8. + ruler.pixels;
        let _ = write!(
            svg,
            "<text x=\"8\" y=\"{y}\" font-family=\"sans-serif\" font-size=\"11\" fill=\"white\">{}</text><path d=\"M 8 {} V {bottom} H {right} V {}\" stroke=\"white\" stroke-width=\"2\" fill=\"none\"/>",
            ruler.label,
            bottom - 4.,
            bottom - 4.
        );
    }
    svg.push_str("</g>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rulers_round_down_without_exceeding_available_width() {
        for metres in [0.01, 0.5, 10., 100., 1000.] {
            for (factor, unit) in [(1., "m"), (1000., "km"), (1609.344, "mi")] {
                let r = Ruler::new(metres, factor, unit);
                assert!(r.pixels > 0. && r.pixels <= 120. + 1e-8);
                let value = r
                    .label
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap();
                assert!((r.pixels * metres - value * factor).abs() < 1e-6);
            }
        }
        assert_eq!(Ruler::new(10., 1000., "km").label, "1 km");
        assert_eq!(Ruler::new(20., 1609.344, "mi").label, "1 mi");
    }
}
