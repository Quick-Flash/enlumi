use super::{gradient_legend_row::GradientLegendRowProps, telemetry::Coloring};
use std::sync::Arc;
use zgui::prelude::*;
#[component]
pub fn ColorLegend(colors: Signal<Arc<Coloring>, LocalStorage>) -> impl IntoView {
    view! {if move ||colors.with(|colors|!colors.scales.is_empty()) {
        column(class="map-color-legend") {for id in move ||colors.with(|colors|colors.scales.keys().copied().collect::<Vec<_>>()),key=|id|*id {GradientLegendRow(colors=colors,id=id)}}
    }}
}
pub fn svg(colors: &Coloring, w: f64, h: f64) -> String {
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let mut svg = String::new();
    let rows = colors.scales.len().min(4);
    for (index, (id, scale)) in colors.scales.iter().take(rows).enumerate() {
        let x = (w - 208.).max(8.);
        let y = (h - 8. - rows as f64 * 70.).max(8.) + index as f64 * 70.;
        let label = if scale.label.chars().count() > 29 {
            format!("{}…", scale.label.chars().take(28).collect::<String>())
        } else {
            scale.label.clone()
        };
        svg.push_str(&format!("<g><title>{}</title><rect x=\"{x}\" y=\"{y}\" width=\"200\" height=\"70\" rx=\"4\" fill=\"#151b26\" fill-opacity=\"0.92\"/><text x=\"{}\" y=\"{}\" fill=\"white\" font-family=\"sans-serif\" font-size=\"11\">{}</text>",escape(&scale.label),x+8.,y+17.,escape(&label)));
        for bucket in 0..64 {
            let [r, g, b, _] = colors.palette_for(*id, bucket);
            svg.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"2.875\" height=\"10\" fill=\"#{r:02x}{g:02x}{b:02x}\"/>",x+8.+f64::from(bucket)*2.875,y+27.));
        }
        svg.push_str(&format!("<text x=\"{}\" y=\"{}\" fill=\"white\" font-family=\"sans-serif\" font-size=\"11\">{}</text><text x=\"{}\" y=\"{}\" text-anchor=\"end\" fill=\"white\" font-family=\"sans-serif\" font-size=\"11\">{}</text></g>",x+8.,y+55.,plot::format_number(scale.gradient.range[0]),x+192.,y+55.,plot::format_number(scale.gradient.range[1])));
    }
    svg
}
