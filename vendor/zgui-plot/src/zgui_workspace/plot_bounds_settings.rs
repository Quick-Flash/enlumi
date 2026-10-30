use crate::Ranges;
use crate::plot_data::PlotData;
use crate::zgui_plot::PlotState;
use zgui::{canvas::Lod, prelude::*};
use zgui_ui::prelude::*;

/// Shared time-plot settings: fit, LOD, and validated numeric bounds.
#[component]
pub fn PlotBoundsSettings(
    state: PlotState,
    series: Signal<PlotData, LocalStorage>,
) -> impl IntoView {
    let editing = RwSignal::new_local(false);
    let fields: [RwSignal<String, LocalStorage>; 4] =
        std::array::from_fn(|_| RwSignal::new_local(String::new()));
    let error = RwSignal::new_local(String::new());
    view! {
        column(class = "time-series-settings") {
            row(class = "plot-toolbar") {
                Toggle(size = ToggleSize::Sm, pressed = state.auto_fit) { "Auto fit" }
                Toggle(size = ToggleSize::Sm,
                    pressed = Binding::controlled(Signal::derive_local(move || state.lod.get() == Lod::Columns),
                        move |columns| state.lod.set(if columns { Lod::Columns } else { Lod::Exact }))) { "Per column" }
                Button(size = ButtonSize::Sm, on:click = move |_| state.fit_data(&series.get_untracked())) { "Fit data" }
                Button(size = ButtonSize::Sm, on:click = move |_| {
                    let ranges = state.ranges.get_untracked();
                    for (field, value) in fields.into_iter().zip([ranges.x.min, ranges.x.max, ranges.y.min, ranges.y.max]) { field.set(value.to_string()); }
                    error.set(String::new()); editing.set(true);
                }) { "Edit bounds" }
            }
            if move || editing.get() {
                for field in move || [(0, "X minimum"), (1, "X maximum"), (2, "Y minimum"), (3, "Y maximum")], key = |field| field.0 {
                    row(class = "bounds-field") {
                        text { {field.1} }
                        Input(value = fields[field.0], a11y:label = field.1)
                    }
                }
                text(class = "settings-error") {{move || error.get()}}
                row(class = "plot-toolbar") {
                    Button(size = ButtonSize::Sm, on:click = move |_| {
                        let values = fields.map(|field| field.get_untracked().trim().parse::<f64>()).into_iter().collect::<Result<Vec<_>, _>>();
                        let result = values.map_err(|_| "Bounds must be numbers".to_owned())
                            .and_then(|values| state.set_view(Ranges::new((values[0], values[1]), (values[2], values[3]))));
                        match result { Ok(()) => editing.set(false), Err(message) => error.set(message) }
                    }) { "Apply bounds" }
                    Button(size = ButtonSize::Sm, on:click = move |_| editing.set(false)) { "Cancel" }
                }
            }
        }
    }
}
