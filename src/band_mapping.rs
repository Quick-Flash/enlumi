use crate::band_mapping_add::BandMappingAddProps;
use crate::copyable_text::CopyableTextProps;
use crate::{
    action_button::ActionButtonProps, band_rule_row::BandRuleRowProps, geometry_inspector::change,
    state::Workbench,
};
use zgui::prelude::*;
#[component]
pub fn BandMapping(workbench: Workbench, id: u64) -> impl IntoView {
    let item = move || {
        workbench.geometry.items.track();
        workbench.geometry_item(id)
    };
    view! {column(class="band-mapping") {
        text(class="band-source-name") {{move ||item().and_then(|item|item.states.as_ref().map(|states|if states.source.is_empty(){"No source: build mapping, then export".into()}else{format!("Source: {}",workbench.field_name(&states.source))})).unwrap_or_default()}}
        row {text(class="muted") {{move || item().and_then(|item|item.states.as_ref().map(|states|format!("{} intervals",states.intervals.len()))).unwrap_or_default()}} ActionButton(class="refresh-state-bands",disabled=Signal::derive_local(move ||item().is_none_or(|item|item.states.as_ref().is_none_or(|states|states.source.is_empty()))),on:click=move |_| {if let Some(item)=workbench.geometry_item(id) && let Some(states)=item.states {workbench.generate_bands(states.source.clone(),Some(id));}}) {"Refresh"}}
        row {
            ActionButton(class="import-state-mapping",on:click=move |_|workbench.mapping_file(id,false)) {"Import mapping"}
            ActionButton(class="export-state-mapping",disabled=Signal::derive_local(move ||item().is_none_or(|item|item.states.as_ref().is_none_or(|states|states.rules.is_empty()))),on:click=move |_|workbench.mapping_file(id,true)) {"Export mapping"}
        }
        text(class="muted") {"Value / name / color"}
        text(class="geometry-help muted") {"Edit value, then press Enter or Set. X removes mapping and shading."}
        column(class="band-rule-list") {
            for value in move ||item().and_then(|item|item.states.as_ref().map(|states|states.rules.iter().map(|rule|rule.value).collect::<Vec<_>>())).unwrap_or_default(),key=|value|*value {BandRuleRow(workbench=workbench,id=id,value=value)}
        }
        BandMappingAdd(workbench=workbench,id=id)
        ActionButton(class="band-show-labels",class:setting-active=move ||item().is_some_and(|item|item.labels),on:click=move |_|change(workbench,id,|item|item.labels = !item.labels)) {{move ||if item().is_some_and(|item|item.labels){"Labels: on"}else{"Labels: off"}}}
        if move || item().is_some_and(|item|item.states.as_ref().is_some_and(|states|states.error.is_some())) {CopyableText(class="warnings",label="State band mapping error",value=Signal::derive_local(move ||item().and_then(|item|item.states.as_ref().and_then(|states|states.error.clone())).unwrap_or_default()))}
        text(class="geometry-help muted") {"Source controls time bounds. Derived sources refresh automatically. Missing values stay unshaded."}
    }}
}
