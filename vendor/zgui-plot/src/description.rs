use crate::labels::{Axis, TickLabel};

/// Horizontal alignment of text relative to its `x` position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAnchor {
    #[default]
    Start,
    Middle,
    End,
}

impl TextAnchor {
    /// SVG `text-anchor` value.
    pub fn as_svg(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Middle => "middle",
            Self::End => "end",
        }
    }
}

/// Text label included by a vector renderer.
#[derive(Clone, Debug)]
pub struct SvgText {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub color: String,
    pub opacity: f64,
    pub font_size: f64,
    pub font_weight: u16,
    pub anchor: TextAnchor,
    pub stroke: Option<String>,
    pub stroke_width: f64,
}

/// Rectangle included by a vector renderer.
#[derive(Clone, Debug)]
pub struct SvgRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: String,
    pub opacity: f64,
    pub stroke: Option<String>,
    pub stroke_opacity: f64,
    pub stroke_width: f64,
    pub radius: f64,
}

/// Legend entry measured by the plotting application and rendered with plot labels.
#[derive(Clone, Debug)]
pub struct SvgLegendEntry {
    pub text: String,
    pub color: String,
    pub opacity: f64,
    pub font_size: f64,
    pub font_weight: u16,
    /// Height of the entry's row, as CSS `line-height`.
    pub line_height: f64,
}

/// Glyph drawn before a legend label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LegendMarker {
    /// Point series: `●`.
    #[default]
    Dot,
    /// Line series: `—`.
    Line,
}

impl LegendMarker {
    /// `label` prefixed with this marker's glyph.
    pub fn label(self, label: &str) -> String {
        let glyph = match self {
            Self::Dot => "●",
            Self::Line => "—",
        };
        format!("{glyph} {label}")
    }
}

/// Entry text styles for each marker, shared by on-screen legends and exports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegendTheme {
    pub dot: LegendEntryStyle,
    pub line: LegendEntryStyle,
}

impl LegendTheme {
    /// Text style for entries drawn with `marker`.
    pub fn style(&self, marker: LegendMarker) -> &LegendEntryStyle {
        match marker {
            LegendMarker::Dot => &self.dot,
            LegendMarker::Line => &self.line,
        }
    }

    /// Export entry for a series, labelled and styled as its on-screen entry.
    pub fn entry(
        &self,
        marker: LegendMarker,
        label: &str,
        color: impl Into<String>,
        visible: bool,
    ) -> SvgLegendEntry {
        self.style(marker)
            .entry(marker.label(label), color, visible)
    }
}

/// Text of one kind of legend entry, shared by on-screen entries and exports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegendEntryStyle {
    pub font_size: f64,
    pub font_weight: u16,
    pub line_height: f64,
    /// Opacity of entries whose series is hidden.
    pub hidden_opacity: f64,
}

impl LegendEntryStyle {
    /// Export entry for a series that is `visible` or hidden.
    pub fn entry(
        &self,
        text: impl Into<String>,
        color: impl Into<String>,
        visible: bool,
    ) -> SvgLegendEntry {
        SvgLegendEntry {
            text: text.into(),
            color: color.into(),
            opacity: if visible { 1.0 } else { self.hidden_opacity },
            font_size: self.font_size,
            font_weight: self.font_weight,
            line_height: self.line_height,
        }
    }

    /// CSS declarations for the entry text (without the hidden opacity).
    pub fn css(&self) -> String {
        format!(
            "font-size: {}px; font-weight: {}; line-height: {}px;",
            self.font_size, self.font_weight, self.line_height
        )
    }
}

/// `#rrggbb` colour with `opacity` folded in as `#rrggbbaa`; other colour
/// syntaxes are returned unchanged.
pub fn css_color(color: &str, opacity: f64) -> String {
    let is_hex = color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !is_hex || opacity >= 1.0 {
        return color.to_owned();
    }
    format!(
        "{color}{:02x}",
        (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
    )
}

/// Vector-renderer metadata surrounding ordered plot geometry.
pub struct PlotDescription {
    pub width: f64,
    pub height: f64,
    pub background: String,
    pub rects: Vec<SvgRect>,
    pub text: Vec<SvgText>,
}

impl PlotDescription {
    /// Empty description of a `width` x `height` canvas filled with `background`.
    pub fn new(width: f64, height: f64, background: impl Into<String>) -> Self {
        Self {
            width,
            height,
            background: background.into(),
            rects: Vec::new(),
            text: Vec::new(),
        }
    }

    /// Add centered title inside plot, with same outlined text used on screen.
    pub fn push_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if title.is_empty() {
            return;
        }
        let style = title_text_style();
        self.text.push(SvgText {
            x: self.width / 2.0,
            y: TITLE_TOP + style.baseline,
            text: title,
            color: style.color,
            opacity: 1.0,
            font_size: style.font_size,
            font_weight: style.font_weight,
            anchor: TextAnchor::Middle,
            stroke: style.outline,
            stroke_width: style.outline_width,
        });
    }

    /// Lay out a legend with [`legend`] and add its box and rows.
    pub fn push_legend(
        &mut self,
        entries: Vec<SvgLegendEntry>,
        top_right: (f64, f64),
        style: &LegendStyle,
    ) {
        if let Some((rect, text)) = legend(entries, top_right, style) {
            self.rects.push(rect);
            self.text.extend(text);
        }
    }
}

/// Font and outline of exported text.
#[derive(Clone, Debug)]
pub struct TextStyle {
    pub color: String,
    pub font_size: f64,
    pub font_weight: u16,
    /// Height of the label box, as CSS `line-height`.
    pub line_height: f64,
    /// Baseline position below the top of the label box.
    pub baseline: f64,
    /// Halo drawn behind glyphs to keep them legible over plot lines.
    pub outline: Option<String>,
    pub outline_width: f64,
}

impl TextStyle {
    /// CSS declarations producing the same text; the outline becomes a
    /// four-way `text-shadow` of half the outline width.
    pub fn css(&self) -> String {
        let mut css = format!(
            "color: {}; font-size: {}px; font-weight: {}; line-height: {}px;",
            self.color, self.font_size, self.font_weight, self.line_height
        );
        if let Some(outline) = &self.outline {
            let d = self.outline_width / 2.0;
            css.push_str(&format!(
                " text-shadow: -{d}px 0 {outline}, {d}px 0 {outline}, 0 -{d}px {outline}, 0 {d}px {outline};"
            ));
        }
        css
    }
}

/// Export text for positioned tick labels: Y labels start at the left of their
/// box, X labels are centred in it, matching how [`crate::labels`] places them.
pub fn tick_label_text<'a>(
    labels: impl IntoIterator<Item = &'a TickLabel>,
    style: &TextStyle,
) -> Vec<SvgText> {
    labels
        .into_iter()
        .map(|label| {
            let (x, anchor) = match label.axis {
                Axis::X => (label.left + label.width / 2.0, TextAnchor::Middle),
                Axis::Y => (label.left, TextAnchor::Start),
            };
            SvgText {
                x,
                y: label.top + style.baseline,
                text: label.text.clone(),
                color: style.color.clone(),
                opacity: label.opacity,
                font_size: style.font_size,
                font_weight: style.font_weight,
                anchor,
                stroke: style.outline.clone(),
                stroke_width: style.outline_width,
            }
        })
        .collect()
}

/// Box appearance for [`legend`].
#[derive(Clone, Debug)]
pub struct LegendStyle {
    /// `#rrggbb` fill colour.
    pub fill: String,
    pub fill_opacity: f64,
    /// `#rrggbb` 1px border colour.
    pub stroke: Option<String>,
    pub stroke_opacity: f64,
    pub radius: f64,
    /// Horizontal and vertical space between the border and the rows.
    pub padding: (f64, f64),
    /// Estimated glyph width as a fraction of font size, used to size the box.
    pub char_width: f64,
}

impl LegendStyle {
    /// CSS declarations for the legend box.
    pub fn css(&self) -> String {
        let mut css = format!(
            "background: {}; border-radius: {}px; padding: {}px {}px;",
            css_color(&self.fill, self.fill_opacity),
            self.radius,
            self.padding.1,
            self.padding.0
        );
        if let Some(stroke) = &self.stroke {
            css.push_str(&format!(
                " border: 1px solid {};",
                css_color(stroke, self.stroke_opacity)
            ));
        }
        css
    }
}

/// Lay out a legend box whose top-right corner is at `(right, top)`, clamped to
/// the canvas' left edge. Rows of each entry's line height stack inside the
/// style's padding, as CSS lays them out. Returns nothing for no entries.
pub fn legend(
    entries: Vec<SvgLegendEntry>,
    (right, top): (f64, f64),
    style: &LegendStyle,
) -> Option<(SvgRect, Vec<SvgText>)> {
    if entries.is_empty() {
        return None;
    }
    let (pad_x, pad_y) = style.padding;
    let width = entries
        .iter()
        .map(|entry| entry.text.chars().count() as f64 * entry.font_size * style.char_width)
        .fold(0.0, f64::max)
        + 2.0 * pad_x;
    let height = 2.0 * pad_y + entries.iter().map(|entry| entry.line_height).sum::<f64>();
    let x = (right - width).max(0.0);
    let rect = SvgRect {
        x,
        y: top,
        width,
        height,
        fill: style.fill.clone(),
        opacity: style.fill_opacity,
        stroke: style.stroke.clone(),
        stroke_opacity: style.stroke_opacity,
        stroke_width: 1.0,
        radius: style.radius,
    };
    let mut row_top = top + pad_y;
    let text = entries
        .into_iter()
        .map(|entry| {
            // Glyphs sit centred in the row; the baseline is ~0.8em below the em-box top.
            let baseline =
                row_top + (entry.line_height - entry.font_size) / 2.0 + entry.font_size * 0.8;
            row_top += entry.line_height;
            SvgText {
                x: x + pad_x,
                y: baseline,
                text: entry.text,
                color: entry.color,
                opacity: entry.opacity,
                font_size: entry.font_size,
                font_weight: entry.font_weight,
                anchor: TextAnchor::Start,
                stroke: None,
                stroke_width: 0.0,
            }
        })
        .collect();
    Some((rect, text))
}

/// In-plot title inset and legend clearance, shared by screen and SVG.
pub const TITLE_TOP: f64 = 4.0;
pub const TITLE_LEGEND_TOP: f64 = 24.0;

/// White text with thin black halo; no title background box.
pub fn title_text_style() -> TextStyle {
    TextStyle {
        color: "#ffffff".into(),
        font_size: 13.0,
        font_weight: 600,
        line_height: 18.0,
        baseline: 14.0,
        outline: Some("#000000".into()),
        outline_width: 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str) -> SvgLegendEntry {
        SvgLegendEntry {
            text: text.to_owned(),
            color: "#fff".to_owned(),
            opacity: 1.0,
            font_size: 10.0,
            font_weight: 400,
            line_height: 14.0,
        }
    }

    fn style() -> LegendStyle {
        LegendStyle {
            fill: "#000".to_owned(),
            fill_opacity: 1.0,
            stroke: None,
            stroke_opacity: 0.0,
            radius: 0.0,
            padding: (6.0, 4.0),
            char_width: 0.5,
        }
    }

    #[test]
    fn legend_sizes_to_widest_entry_and_stacks_rows() {
        let (rect, text) =
            legend(vec![entry("ab"), entry("abcd")], (100.0, 4.0), &style()).unwrap();
        // 4 chars * 10px * 0.5 + 2 * 6 padding; 2 * 4 padding + 2 rows * 14.
        assert_eq!((rect.x, rect.width, rect.height), (68.0, 32.0, 36.0));
        // row top 8 + (14 - 10) / 2 + 8, then one 14px row lower.
        assert_eq!(
            text.iter().map(|text| text.y).collect::<Vec<_>>(),
            [18.0, 32.0]
        );
        assert!(text.iter().all(|text| text.x == 74.0));
    }

    #[test]
    fn css_matches_export_styles() {
        assert_eq!(css_color("#0e171b", 0.79), "#0e171bc9");
        assert_eq!(css_color("#0e171b", 1.0), "#0e171b");
        assert_eq!(css_color("red", 0.5), "red");
        let legend_css = LegendStyle {
            stroke: Some("#46565b".to_owned()),
            stroke_opacity: 0.8,
            ..style()
        }
        .css();
        assert!(legend_css.contains("border: 1px solid #46565bcc;"));
        assert!(legend_css.contains("padding: 4px 6px;"));
        let text = TextStyle {
            color: "#fff".to_owned(),
            font_size: 13.0,
            font_weight: 600,
            line_height: 16.0,
            baseline: 13.0,
            outline: Some("#000".to_owned()),
            outline_width: 2.0,
        };
        assert!(
            text.css()
                .contains("text-shadow: -1px 0 #000, 1px 0 #000, 0 -1px #000, 0 1px #000;")
        );
        let entry_style = LegendEntryStyle {
            font_size: 12.0,
            font_weight: 400,
            line_height: 16.0,
            hidden_opacity: 0.3,
        };
        assert_eq!(entry_style.entry("x", "#fff", false).opacity, 0.3);
        let theme = LegendTheme {
            dot: LegendEntryStyle {
                font_size: 14.0,
                ..entry_style
            },
            line: entry_style,
        };
        let line = theme.entry(LegendMarker::Line, "CDF", "#fff", true);
        assert_eq!((line.text.as_str(), line.font_size), ("— CDF", 12.0));
        assert_eq!(
            entry_style.css(),
            "font-size: 12px; font-weight: 400; line-height: 16px;"
        );
    }

    #[test]
    fn tick_text_centres_x_labels_and_left_aligns_y_labels() {
        let label = |axis, left| TickLabel {
            axis,
            value: 0.0,
            text: "0".to_owned(),
            opacity: 0.5,
            left,
            top: 10.0,
            width: 64.0,
            height: 16.0,
        };
        let style = TextStyle {
            color: "#fff".to_owned(),
            font_size: 13.0,
            font_weight: 600,
            line_height: 16.0,
            baseline: 13.0,
            outline: Some("#000".to_owned()),
            outline_width: 2.0,
        };
        let text = tick_label_text(&[label(Axis::X, 100.0), label(Axis::Y, 4.0)], &style);
        assert_eq!((text[0].x, text[0].anchor), (132.0, TextAnchor::Middle));
        assert_eq!((text[1].x, text[1].anchor), (4.0, TextAnchor::Start));
        assert!(
            text.iter()
                .all(|text| text.y == 23.0 && text.opacity == 0.5)
        );
    }

    #[test]
    fn description_collects_legend() {
        let mut description = PlotDescription::new(100.0, 50.0, "#000");
        description.push_legend(vec![entry("ab")], (100.0, 4.0), &style());
        description.push_legend(Vec::new(), (100.0, 4.0), &style());
        assert_eq!((description.rects.len(), description.text.len()), (1, 1));
    }

    #[test]
    fn legend_clamps_to_canvas_and_skips_empty() {
        let (rect, _) = legend(vec![entry("wide label")], (10.0, 0.0), &style()).unwrap();
        assert_eq!(rect.x, 0.0);
        assert!(legend(Vec::new(), (10.0, 0.0), &style()).is_none());
    }
}
