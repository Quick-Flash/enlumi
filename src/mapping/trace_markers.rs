use super::settings::target;
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trace_color,
};
use zgui::prelude::*;

#[component]
pub fn TraceMarkers(
    workbench: Workbench,
    pane: Pane,
    selected: Signal<Option<u64>, LocalStorage>,
    #[prop(default = true)] show_arrows: bool,
    #[prop(optional)] ids: Option<Signal<Vec<u64>, LocalStorage>>,
) -> impl IntoView {
    let pane = target(workbench, pane);
    let targets = move || {
        ids.map_or_else(
            || selected.get().into_iter().collect::<Vec<_>>(),
            |ids| ids.get(),
        )
    };
    let states = move || {
        targets()
            .iter()
            .all(|id| trace_color::current(pane, Some(*id)).state_markers)
    };
    let mixed_states = move || {
        !states()
            && targets()
                .iter()
                .any(|id| trace_color::current(pane, Some(*id)).state_markers)
    };
    let source = move || {
        super::display_options::state_source(workbench, pane).map_or_else(
            || {
                "Labeled dots for state changes. Attach a state band to this plot to use them."
                    .to_owned()
            },
            |name| format!("Labeled dots when state band {name} changes value."),
        )
    };
    let arrows = move || targets().iter().all(|id| trace_color::arrows(pane, *id));
    let mixed_arrows =
        move || !arrows() && targets().iter().any(|id| trace_color::arrows(pane, *id));
    view! {column(class="map-trace-markers") {
        ActionButton(class="map-trace-states map-setting-toggle",class:setting-active=states,a11y:toggled_on=states,on:click=move |_|{let on=states();for id in targets(){let _=trace_color::change(pane,Some(id),|style|style.state_markers= !on);}}) {{move ||if mixed_states(){"State markers: mixed"}else if states(){"State markers: on"}else{"State markers: off"}}}
        text(class="map-setting-help") {{source}}
        if move ||show_arrows {
        ActionButton(class="map-arrows-editor map-setting-toggle",class:setting-active=arrows,a11y:toggled_on=arrows,on:click=move |_|{let on=arrows();for id in targets(){let _=trace_color::change(pane,Some(id),|s|s.direction_arrows=Some(!on));}}) {{move ||if mixed_arrows(){"Direction arrows: mixed"}else if arrows(){"Direction arrows: on"}else{"Direction arrows: off"}}}

        text(class="map-setting-help") {"Arrows use a darker shade of this line's color."}
        }

    }}
}
