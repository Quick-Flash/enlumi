use super::{SeriesPane as Pane, color_picker::ColorPickerProps};
use crate::line_draw::{Connection, LineDraw, LinePattern};
use crate::plot_data::{MarkerShape, SeriesStyle};
use zgui::prelude::*;
use zgui_ui::prelude::*;

/// Selected stable line IDs for shared appearance controls.
pub type LineIds = Signal<Vec<u64>, LocalStorage>;

fn styles(pane: Pane, ids: LineIds, edit: impl Fn(&mut SeriesStyle)) {
    let ids = ids.get_untracked();
    pane.plot.styles.update(|styles| {
        for id in ids {
            let key = id.to_string();
            edit(
                styles
                    .entry(key.clone())
                    .or_insert_with(|| SeriesStyle::for_id(&key)),
            );
        }
    });
}
fn drawing(pane: Pane, ids: LineIds, edit: impl Fn(&mut LineDraw)) {
    let ids = ids.get_untracked();
    pane.drawing.update(|drawing| {
        for id in ids {
            edit(drawing.entry(id).or_default());
        }
    });
}

#[component]
pub fn LineStyle(
    ids: LineIds,
    pane: Pane,
    #[prop(default = true)] show_connection: bool,
    #[prop(attrs)] attrs: Attrs,
) -> impl IntoView {
    super::install();
    let current = move || {
        pane.plot.style(
            &ids.with(|ids| ids.first().copied().unwrap_or(0))
                .to_string(),
        )
    };
    let all = move |test: &dyn Fn(u64) -> bool| {
        ids.with(|ids| !ids.is_empty() && ids.iter().all(|id| test(*id)))
    };
    let width_mixed = move || !all(&|id| pane.plot.style(&id.to_string()).width == current().width);
    let radius_mixed =
        move || !all(&|id| pane.plot.style(&id.to_string()).radius == current().radius);
    let markers_disabled = Signal::derive_local(move || {
        !ids.with(|ids| {
            ids.iter().any(|id| {
                pane.drawing
                    .with(|drawing| drawing.get(id).is_some_and(|drawing| drawing.markers))
            })
        })
    });
    view! {
        column(class = "line-style", {..attrs}) {
            row(class = "appearance-row") {
                ColorPicker(disabled = Signal::derive_local(move || ids.with(Vec::len) != 1), color = Signal::derive_local(move || current().color), on_change = UnsyncCallback::new(move |color| {
                        if ids.with_untracked(Vec::len) == 1 { styles(pane, ids, |style| style.color = color); }
                    }))
                column(class = "appearance-controls") {
                    text(class = "heading") {{move || if width_mixed() { "Line width: mixed".into() } else { format!("Line width: {:.1} px", current().width) }}}
                    Slider(class = "line-width", min = 0.5, max = 10.0, step = 0.5, label = "Line width", value = Binding::controlled(Signal::derive_local(move || current().width), move |width| styles(pane, ids, |style| style.width = width)))
                    text(class = "heading") { "Line type" }
                    row(class = "plot-toolbar") {
                        for choice in move || [("Solid", LinePattern::Solid), ("Dashed", LinePattern::Dashed), ("Dotted", LinePattern::Dotted), ("Dash-dot", LinePattern::DashDot), ("None", LinePattern::None)], key = |choice| choice.0 {
                            Button(size = ButtonSize::Xs, class = "line-pattern", class:chosen = move || all(&|id| pane.drawing.with(|drawing| drawing.get(&id).copied().unwrap_or_default().pattern == choice.1)),
                                on:click = move |_| drawing(pane, ids, |drawing| {
                                    drawing.pattern = choice.1;
                                    if choice.1 == LinePattern::None { drawing.markers = true; }
                                })) {{choice.0}}
                        }
                    }
                    text(class = "heading") { "Sample markers" }
                    row(class = "plot-toolbar") {
                        for choice in move || [("None", None), ("Dots", Some(MarkerShape::Circle)), ("Square", Some(MarkerShape::Square)), ("Triangle", Some(MarkerShape::Triangle)), ("Diamond", Some(MarkerShape::Diamond)), ("Cross", Some(MarkerShape::Cross))], key = |choice| choice.0 {
                            Button(size = ButtonSize::Xs, class = "line-marker", class:chosen = move || all(&|id| {
                                let shown = pane.drawing.with(|drawing| drawing.get(&id).copied().unwrap_or_default().markers);
                                match choice.1 { None => !shown, Some(marker) => shown && pane.plot.style(&id.to_string()).marker == marker }
                            }), on:click = move |_| {
                                drawing(pane, ids, |drawing| {
                                    drawing.markers = choice.1.is_some();
                                    if !drawing.markers && drawing.pattern == LinePattern::None {
                                        drawing.pattern = LinePattern::Solid;
                                    }
                                });
                                if let Some(marker) = choice.1 { styles(pane, ids, |style| style.marker = marker); }
                            }) {{choice.0}}
                        }
                    }
                    column(class = "marker-radius-controls", class:marker-radius-disabled = move || markers_disabled.get()) {
                        text(class = "heading") {{move || if radius_mixed() { "Marker radius: mixed".into() } else { format!("Marker radius: {:.1} px", current().radius) }}}
                        Slider(class = "line-radius", disabled = markers_disabled, min = 1.0, max = 10.0, step = 0.5, label = "Marker radius", value = Binding::controlled(Signal::derive_local(move || current().radius), move |radius| styles(pane, ids, |style| style.radius = radius)))
                    }
                    if move || show_connection {
                    text(class = "heading") { "Line connection" }
                    row(class = "plot-toolbar") {
                        for choice in move || [("Linear", Connection::Linear), ("Steps pre", Connection::Pre), ("Steps post", Connection::Post)], key = |choice| choice.0 {
                            Button(size = ButtonSize::Xs, class = "line-connection", class:chosen = move || all(&|id| pane.drawing.with(|drawing| drawing.get(&id).copied().unwrap_or_default().connection == choice.1)),
                                on:click = move |_| drawing(pane, ids, |drawing| drawing.connection = choice.1)) {{choice.0}}
                        }
                    }
                    }
                }
            }
        }
    }
}
