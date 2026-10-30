//! Plot chrome styling: the single source for the CSS that styles tick labels
//! and legends on screen and for the matching SVG export styles.

use crate::labels::{TickLabelLayout, XLabelRow};
use crate::{LegendEntryStyle, LegendStyle, LegendTheme, TextStyle};

fn plot_background_css() -> String {
    "#12181c".into()
}
fn selected_css() -> String {
    "#ffe0a0".into()
}

/// Scatter and trend tick label boxes; X labels sit inside the bottom edge.
pub const TREND_LABELS: TickLabelLayout = TickLabelLayout {
    x_label: (64.0, 16.0),
    y_label: (56.0, 16.0),
    inset: 4.0,
    x_row: XLabelRow::Inside,
};

/// Histogram tick label boxes; values sit in the axis strip under the bars.
pub const HISTOGRAM_LABELS: TickLabelLayout = TickLabelLayout {
    x_label: (72.0, 16.0),
    y_label: (56.0, 16.0),
    inset: 4.0,
    x_row: XLabelRow::Below { gap: 2.0 },
};

/// Legend box offset from the plot's top-right corner.
pub const LEGEND_TOP: f64 = 4.0;
pub const LEGEND_RIGHT: f64 = 6.0;
/// The histogram legend clears the percent axis on its right edge.
pub const HISTOGRAM_LEGEND_RIGHT: f64 = 48.0;

/// Firmware (dot) and line legend entries.
pub const LEGEND: LegendTheme = LegendTheme {
    dot: LegendEntryStyle {
        font_size: 14.0,
        font_weight: 400,
        line_height: 18.0,
        hidden_opacity: 0.3,
    },
    line: LegendEntryStyle {
        font_size: 15.0,
        font_weight: 600,
        line_height: 18.0,
        hidden_opacity: 0.5,
    },
};

/// Tick label text, outlined in the plot background to stay legible over lines.
pub fn tick_text() -> TextStyle {
    TextStyle {
        color: "#f4f7f5".to_owned(),
        font_size: 13.0,
        font_weight: 600,
        line_height: TREND_LABELS.x_label.1,
        baseline: 13.0,
        outline: Some(plot_background_css()),
        outline_width: 2.0,
    }
}

pub fn legend_box() -> LegendStyle {
    LegendStyle {
        fill: "#0e171b".to_owned(),
        fill_opacity: 0.79,
        stroke: Some("#46565b".to_owned()),
        stroke_opacity: 0.8,
        radius: 3.0,
        padding: (6.0, 4.0),
        char_width: 0.62,
    }
}

/// Plot crosshair cursor: a selected-colour cross outlined in the background.
fn crosshair_cursor() -> String {
    let (outline, cross) = (plot_background_css(), selected_css());
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='17' height='17' viewBox='0 0 17 17'>\
         <path d='M8.5 1v15M1 8.5h15' stroke='{outline}' stroke-width='3'/>\
         <path d='M8.5 1v15M1 8.5h15' stroke='{cross}' stroke-width='1'/></svg>"
    );
    let encoded = svg
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace('#', "%23");
    format!("url(\"data:image/svg+xml,{encoded}\") 8 8, crosshair")
}

/// CSS rules for tick labels and legends, appended to the app stylesheet.
/// Layout-only properties stay in surface/workspace stylesheets.
pub fn stylesheet() -> String {
    [
        format!(".plot-tick {{ {} }}", tick_text().css()),
        format!(".plot-tick-y {{ width: {}px; }}", TREND_LABELS.y_label.0),
        format!(".plot-tick-x {{ width: {}px; }}", TREND_LABELS.x_label.0),
        format!(
            ".histogram-plot .plot-tick-y {{ width: {}px; }}",
            HISTOGRAM_LABELS.y_label.0
        ),
        format!(
            ".histogram-plot .plot-tick-x {{ width: {}px; }}",
            HISTOGRAM_LABELS.x_label.0
        ),
        format!(
            ".plot-legend {{ top: {LEGEND_TOP}px; right: {LEGEND_RIGHT}px; {} }}",
            legend_box().css()
        ),
        format!(".histogram-plot > .plot-legend {{ right: {HISTOGRAM_LEGEND_RIGHT}px; }}"),
        crate::zgui_legend::stylesheet(&LEGEND),
        format!(".plot-input {{ cursor: {}; }}", crosshair_cursor()),
        // After `.legend-hidden`, so hovering a hidden entry still lights it up.
        ".plot-legend-item:hover { opacity: 1; filter: brightness(1.6); }".to_owned(),
    ]
    .join("\n")
}

use crate::labels::TickLabel;
use crate::{PlotDescription, SvgLegendEntry, tick_label_text};
pub fn description(
    width: f64,
    height: f64,
    title: &str,
    ticks: &[TickLabel],
    legend_entries: Vec<SvgLegendEntry>,
    legend_right: f64,
) -> PlotDescription {
    let mut description = PlotDescription::new(width, height, plot_background_css());
    description.text = tick_label_text(ticks, &tick_text());
    description.push_title(title);
    description.push_legend(
        legend_entries,
        (
            width - legend_right,
            if title.is_empty() {
                LEGEND_TOP
            } else {
                crate::TITLE_LEGEND_TOP
            },
        ),
        &legend_box(),
    );
    description
}
