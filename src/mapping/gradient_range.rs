use super::telemetry::ColorContext;
use crate::{
    action_button::ActionButtonProps,
    state::Pane,
    trace_color::{self, TraceColor},
};
use zgui::prelude::*;

#[component]
pub fn GradientRange(
    pane: Pane,
    selected: Signal<Option<u64>, LocalStorage>,
    appearance: Memo<TraceColor>,
) -> impl IntoView {
    let colors = use_local_context::<ColorContext>();
    let range = Signal::derive_local(move || {
        colors.and_then(|colors| {
            colors.0.with(|colors| {
                colors
                    .scales
                    .get(&selected.get()?)
                    .filter(|scale| scale.mode == appearance.get().mode)
                    .and_then(|scale| scale.data_range)
            })
        })
    });
    view! {column(class="trace-gradient-data") {
        text(class="muted"){{move ||match range.get() {
            Some([lo,hi]) if lo==hi=>format!("Data is constant at {}. One color expected.",plot::format_number(lo)),
            Some([lo,hi])=>format!("Data range: {} to {}",plot::format_number(lo),plot::format_number(hi)),
            None=>if appearance.get().mode==trace_color::Mode::Height {"No valid height values. Bind Z below."}else{"No valid speed samples."}.into(),
        }}}
        if move ||range.get().zip(appearance.get().gradient()).is_some_and(|([lo,hi],gradient)|lo<hi &&(hi<=gradient.range[0]||lo>=gradient.range[1])) {
            text(class="trace-gradient-clipped"){"All values fall outside chosen limits. Fit data or change limits to see gradient."}
        }
        ActionButton(class="trace-gradient-fit",disabled=Signal::derive_local(move ||range.get().is_none_or(|[lo,hi]|lo>=hi)),on:click=move |_|if let Some(range)=range.get_untracked(){let _=trace_color::change(pane,selected.get_untracked(),|style|if let Some(gradient)=style.gradient_mut(){gradient.range=range;});}){"Fit data range"}
    }}
}
