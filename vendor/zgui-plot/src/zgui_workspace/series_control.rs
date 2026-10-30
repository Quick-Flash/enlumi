use crate::plot_data::{MarkerShape, SeriesStyle};
use crate::zgui_plot::PlotState;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn SeriesControl(
    id: String,
    label: String,
    state: PlotState,
    #[prop(default = true)] line: bool,
    #[prop(default = true)] markers: bool,
) -> impl IntoView {
    let id = StoredValue::new_local(id);
    let selected = move || id.with_value(|id| state.style(id));
    view! {
        column(class = "series-control") {
            Toggle(size = ToggleSize::Sm,
                pressed = Binding::controlled(Signal::derive_local(move || id.with_value(|id| !state.hidden.with(|hidden| hidden.contains(id)))),
                    move |shown| id.with_value(|id| state.hidden.update(|hidden| { if shown { hidden.remove(id); } else { hidden.insert(id.clone()); } })))) { {label} }
            row(class = "plot-toolbar") {
                for choice in move || [("Green", [117,197,139,255]), ("Blue", [119,185,232,255]), ("Yellow", [238,190,96,255]), ("Red", [237,145,134,255])], key = |choice| choice.0 {
                    Button(size = ButtonSize::Xs, class:setting-active = move || selected().color == choice.1,
                        on:click = move |_| id.with_value(|id| state.styles.update(|styles| { styles.entry(id.clone()).or_insert_with(|| SeriesStyle::for_id(id)).color = choice.1; }))) { {choice.0} }
                }
            }
            row(class = "plot-toolbar") {
                if move || line {
                text { "Line width" }
                for width in move || [1.0_f64,1.5,2.0,3.0,5.0], key = |width| width.to_bits() {
                    Button(size = ButtonSize::Xs, class:setting-active = move || selected().width == width,
                        on:click = move |_| id.with_value(|id| state.styles.update(|styles| { styles.entry(id.clone()).or_insert_with(|| SeriesStyle::for_id(id)).width = width; }))) { {width.to_string()} }
                }
                }
                if move || markers {
                text { "Radius" }
                for radius in move || [2_u8,3,5,8], key = |radius| *radius {
                    Button(size = ButtonSize::Xs, class:setting-active = move || selected().radius == f64::from(radius),
                        on:click = move |_| id.with_value(|id| state.styles.update(|styles| { styles.entry(id.clone()).or_insert_with(|| SeriesStyle::for_id(id)).radius = f64::from(radius); }))) { {radius.to_string()} }
                }
                }
            }
            if move || markers {
            row(class = "plot-toolbar") {
                for shape in move || [MarkerShape::Circle,MarkerShape::Square,MarkerShape::Triangle,MarkerShape::Diamond,MarkerShape::Cross], key = |shape| *shape {
                    Button(size = ButtonSize::Xs, class:setting-active = move || selected().marker == shape,
                        on:click = move |_| id.with_value(|id| state.styles.update(|styles| { styles.entry(id.clone()).or_insert_with(|| SeriesStyle::for_id(id)).marker = shape; }))) { {format!("{shape:?}")} }
                }
            }
            }
        }
    }
}
