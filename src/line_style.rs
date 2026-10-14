use crate::{color_picker::ColorPickerProps, line_draw::Connection, state::Pane};
use plot::plot_data::{MarkerShape, SeriesStyle};
use zgui::{prelude::*, reactive::UnsyncCallback};
use zgui_ui::prelude::*;

#[component]
pub fn LineStyle(id: u64, pane: Pane) -> impl IntoView {
    let state = pane.plot;
    let key = StoredValue::new_local(id.to_string());
    let current = move || key.with_value(|key| state.style(key));
    view! {
        column(class = "line-style") {
            row(class = "appearance-row") {
                ColorPicker(color = Signal::derive_local(move || current().color), on_change = UnsyncCallback::new(move |color| key.with_value(|key| state.styles.update(|styles| styles.entry(key.clone()).or_insert_with(|| SeriesStyle::for_id(key)).color = color))))
                column(class = "appearance-controls") {
                    text(class = "heading") {{move || format!("Line width: {:.1} px", current().width)}}
                    Slider(class = "line-width", min = 0.5, max = 10.0, step = 0.5, label = "Line width", value = Binding::controlled(Signal::derive_local(move || current().width), move |width| key.with_value(|key| state.styles.update(|styles| styles.entry(key.clone()).or_insert_with(|| SeriesStyle::for_id(key)).width = width))))
                    text(class = "heading") { "Sample markers" }
                    row(class = "plot-toolbar") {
                        for choice in move || [("None", None), ("Dots", Some(MarkerShape::Circle)), ("Square", Some(MarkerShape::Square)), ("Triangle", Some(MarkerShape::Triangle)), ("Diamond", Some(MarkerShape::Diamond)), ("Cross", Some(MarkerShape::Cross))], key = |choice| choice.0 {
                            Button(size = ButtonSize::Xs, class = "line-marker", class:chosen = move || {
                                let shown = pane.drawing.with(|drawing| drawing.get(&id).copied().unwrap_or_default().markers);
                                match choice.1 { None => !shown, Some(marker) => shown && current().marker == marker }
                            }, on:click = move |_| {
                                pane.drawing.update(|drawing| drawing.entry(id).or_default().markers = choice.1.is_some());
                                if let Some(marker) = choice.1 { key.with_value(|key| state.styles.update(|styles| styles.entry(key.clone()).or_insert_with(|| SeriesStyle::for_id(key)).marker = marker)); }
                            }) {{choice.0}}
                        }
                    }
                    if move || pane.drawing.with(|drawing| drawing.get(&id).is_some_and(|drawing| drawing.markers)) {
                        text(class = "heading") {{move || format!("Marker radius: {:.1} px", current().radius)}}
                        Slider(min = 1.0, max = 10.0, step = 0.5, label = "Marker radius", value = Binding::controlled(Signal::derive_local(move || current().radius), move |radius| key.with_value(|key| state.styles.update(|styles| styles.entry(key.clone()).or_insert_with(|| SeriesStyle::for_id(key)).radius = radius))))
                    }
                    text(class = "heading") { "Line connection" }
                    row(class = "plot-toolbar") {
                        for choice in move || [("Linear", Connection::Linear), ("Steps pre", Connection::Pre), ("Steps post", Connection::Post)], key = |choice| choice.0 {
                            Button(size = ButtonSize::Xs, class = "line-connection", class:chosen = move || pane.drawing.with(|drawing| drawing.get(&id).copied().unwrap_or_default().connection == choice.1),
                                on:click = move |_| pane.drawing.update(|drawing| drawing.entry(id).or_default().connection = choice.1)) {{choice.0}}
                        }
                    }
                }
            }
        }
    }
}
