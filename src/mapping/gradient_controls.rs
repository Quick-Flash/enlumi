use super::{gradient_stop::GradientStopProps, settings::AltitudeOptionsProps};
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trace_color::{self, Mode, TraceColor},
};
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn GradientControls(
    workbench: Workbench,
    pane: Pane,
    selected: Signal<Option<u64>, LocalStorage>,
    appearance: Memo<TraceColor>,
) -> impl IntoView {
    let low = RwSignal::new_local(String::new());
    let high = RwSignal::new_local(String::new());
    let error = RwSignal::new_local(String::new());
    let sync = zgui::reactive::RenderEffect::new(move |_| {
        if let Some(gradient) = appearance.get().gradient() {
            low.set(gradient.range[0].to_string());
            high.set(gradient.range[1].to_string());
        }
        error.set(String::new());
    });
    on_cleanup_local(move || drop(sync));
    let apply = move || {
        let result = low
            .get_untracked()
            .trim()
            .parse::<f64>()
            .ok()
            .zip(high.get_untracked().trim().parse::<f64>().ok())
            .ok_or_else(|| "Enter numeric gradient limits".to_owned())
            .and_then(|(low, high)| {
                trace_color::change(pane, selected.get_untracked(), |color| {
                    if let Some(gradient) = color.gradient_mut() {
                        gradient.range = [low, high];
                    }
                })
            });
        error.set(result.err().unwrap_or_default());
    };
    let height = Memo::new(move |_| appearance.get().mode == Mode::Height);
    view! {column(class="trace-gradient-controls") {
        text(class="muted"){{move ||if height.get(){"Height in metres, from bound Z. Values outside limits use endpoint colors."}else{"Horizontal speed in m/s. Values outside limits use endpoint colors."}}}
        row(class="trace-gradient-limits") {
            column{ text{"Lower limit"} Input(class="trace-gradient-min",value=low,a11y:label="Gradient lower limit",on:key_down=move |event|if event.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){apply();}) }
            column{ text{"Upper limit"} Input(class="trace-gradient-max",value=high,a11y:label="Gradient upper limit",on:key_down=move |event|if event.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){apply();}) }
            ActionButton(class="trace-gradient-apply",on:click=move |_|apply()){"Apply limits"}
        }
        row(class="map-coordinate-units") {GradientStop(pane=pane,selected=selected,appearance=appearance,high=false) GradientStop(pane=pane,selected=selected,appearance=appearance,high=true)}
        if move ||!error.get().is_empty(){text(class="trace-gradient-error",a11y:role=Role::Alert){{move ||error.get()}}}
        if move ||height.get() { {move ||{
            let source=selected.get().and_then(|id|pane.lines.with(|lines|lines.iter().find(|line|line.id==id).map(|line|line.source.clone())));
            let index=pane.trajectory.with(|s|s.as_ref().and_then(|s|s.pairs().iter().position(|pair|Some(&pair.y)==source.as_ref()))).unwrap_or(0);
            AnyView::new(view!{AltitudeOptions(workbench=workbench,pane=pane,index=index,force=true)})
        }} }
    }}
}
