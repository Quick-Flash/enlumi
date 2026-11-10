use super::telemetry::Coloring;
use std::sync::Arc;
use zgui::prelude::*;
#[component]
pub fn GradientLegendRow(colors: Signal<Arc<Coloring>, LocalStorage>, id: u64) -> impl IntoView {
    view! {column(class="map-gradient-key") {
        text(class="map-color-label"){{move ||colors.with(|colors|colors.scales.get(&id).map(|scale|scale.label.clone()).unwrap_or_default())}}
        row(class="map-color-ramp") {for bucket in move ||0_u8..32,key=|bucket|*bucket {
            box(style:background=move ||colors.with(|colors|{let [r,g,b,_]=colors.palette_for(id,(u16::from(bucket)*63/31) as u8);Some(format!("#{r:02x}{g:02x}{b:02x}"))})){}
        }}
        row(class="map-color-limits") {
            text{{move ||colors.with(|colors|colors.scales.get(&id).map(|scale|plot::format_number(scale.gradient.range[0])).unwrap_or_default())}}
            text{{move ||colors.with(|colors|colors.scales.get(&id).map(|scale|plot::format_number(scale.gradient.range[1])).unwrap_or_default())}}
        }
    }}
}
