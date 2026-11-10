use crate::state::Workbench;
use zgui::prelude::*;
#[component]
pub fn StateBandKey(workbench: Workbench, id: u64) -> impl IntoView {
    let item = Memo::new(move |_| {
        workbench.geometry.items.track();
        workbench.geometry_item(id)
    });
    view! {column(class="state-color-key-band") {
        text(class="heading"){{move ||item.with(|item|item.as_ref().map(|item|item.name.clone()).unwrap_or_default())}}
        row(class="state-color-key-rules") {for rule in move ||item.with(|item|item.as_ref().and_then(|item|item.states.as_ref()).map(|s|s.rules.clone()).unwrap_or_default()),key=|rule|(rule.value,rule.name.clone(),rule.color) {
            row(class="state-color-key-rule") {
                box(class="state-color-key-swatch",style:background=crate::plot_appearance::css(rule.color)){}
                text{{rule.name}}
            }
        }}
    }}
}
