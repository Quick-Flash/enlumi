use crate::{
    state::Pane,
    trace_color::{self, TraceColor},
};
use plot::zgui_series::ColorPickerProps;
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn GradientStop(
    pane: Pane,
    selected: Signal<Option<u64>, LocalStorage>,
    appearance: Memo<TraceColor>,
    high: bool,
) -> impl IntoView {
    let color = Signal::derive_local(move || {
        appearance
            .get()
            .gradient()
            .map(|gradient| gradient.colors[usize::from(high)])
            .unwrap_or([0; 3])
    });
    view! {Popover {
        PopoverTrigger(class=if high{"trace-gradient-high-color"}else{"trace-gradient-low-color"},size=ButtonSize::Xs,variant=ButtonVariant::Outline,a11y:label=if high{"Upper limit color"}else{"Lower limit color"}) {
            box(class="state-color-key-swatch",style:background=move ||Some(crate::plot_appearance::css(color.get()))){}
            text{{if high{"High color"}else{"Low color"}}}
        }
        PopoverContent(class="trace-gradient-color-popup") {
            ColorPicker(color=Signal::derive_local(move ||{let [r,g,b]=color.get();[r,g,b,255]}),on_change=UnsyncCallback::new(move |[r,g,b,_]:[u8;4]|{let _=trace_color::change(pane,selected.get_untracked(),|style|if let Some(gradient)=style.gradient_mut(){gradient.colors[usize::from(high)]=[r,g,b];});}))
        }
    }}
}
