use super::telemetry::Coloring;
use std::sync::Arc;
use zgui::prelude::*;
#[component]
pub fn GradientLegendRow(
    colors: Signal<Arc<Coloring>, LocalStorage>,
    id: u64,
    #[prop(default = false)] compact: bool,
) -> impl IntoView {
    view! {column(class="map-gradient-key",class:map-gradient-compact=compact) {
        text(class="map-color-label"){{move ||colors.with(|colors|colors.scales.get(&id).map(|scale|scale.label.clone()).unwrap_or_default())}}
        row(class="map-color-ramp") {for bucket in move ||0_u8..32,key=|bucket|*bucket {
            box(style:background=move ||colors.with(|colors|{let [r,g,b,_]=colors.palette_for(id,(u16::from(bucket)*63/31) as u8);Some(format!("#{r:02x}{g:02x}{b:02x}"))})){}
        }}
        row(class="map-color-limits") {
            text{{move ||colors.with(|colors|colors.scales.get(&id).map(|scale|plot::format_number(scale.gradient.range[0])).unwrap_or_default())}}
            if move ||compact {text{{move ||colors.with(|colors|colors.scales.get(&id).map(|scale|plot::format_number((scale.gradient.range[0]+scale.gradient.range[1])/2.)).unwrap_or_default())}}}
            text{{move ||colors.with(|colors|colors.scales.get(&id).map(|scale|format!("{}{}",plot::format_number(scale.gradient.range[1]),if compact{match scale.mode{crate::trace_color::Mode::Speed=>" m/s",crate::trace_color::Mode::Height=>" m",_=>""}}else{""})).unwrap_or_default())}}
        }
    }}
}
