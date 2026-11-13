//! Compact color controls shared by trace cards and the on-map legend.
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    trace_color::{self, Mode},
};
use zgui::prelude::*;
use zgui_ui::prelude::*;

pub(crate) fn appearance(
    pane: Pane,
    id: Option<u64>,
    source: Option<&str>,
) -> trace_color::TraceColor {
    source
        .and_then(|source| {
            pane.draft_trace_colors
                .with(|colors| colors.get(source).copied())
        })
        .unwrap_or_else(|| {
            id.map(|id| trace_color::current(pane, Some(id)))
                .unwrap_or_default()
        })
}

fn change(
    pane: Pane,
    id: Option<u64>,
    source: Option<Signal<String, LocalStorage>>,
    edit: impl FnOnce(&mut trace_color::TraceColor),
) -> Result<(), String> {
    if let Some(id) = id {
        if let Some(source) = source.map(|source| source.get_untracked())
            && let Some(color) = pane
                .draft_trace_colors
                .with_untracked(|colors| colors.get(&source).copied())
        {
            pane.trace_colors.update(|colors| {
                colors.insert(id, color);
            });
            pane.draft_trace_colors.update(|colors| {
                colors.remove(&source);
            });
        }
        return trace_color::change(pane, Some(id), edit);
    }
    let source = source
        .map(|source| source.get_untracked())
        .filter(|source| !source.is_empty())
        .ok_or("Choose a trace")?;
    let mut color = reactive_graph::graph::untrack(|| appearance(pane, None, Some(&source)));
    edit(&mut color);
    if !color.valid() {
        return Err("Limits must be finite; upper limit must exceed lower limit".into());
    }
    pane.draft_trace_colors.update(|colors| {
        colors.insert(source, color);
    });
    Ok(())
}

#[component]
pub fn TraceColorControls(
    workbench: Workbench,
    pane: Pane,
    selected: Signal<Option<u64>, LocalStorage>,
    #[prop(optional)] source: Option<Signal<String, LocalStorage>>,
) -> impl IntoView {
    let style = Memo::new(move |_| {
        appearance(
            pane,
            selected.get(),
            source.map(|source| source.get()).as_deref(),
        )
    });
    let low = RwSignal::new_local(String::new());
    let high = RwSignal::new_local(String::new());
    let error = RwSignal::new_local(String::new());
    let sync = zgui::reactive::RenderEffect::new(move |_| {
        if let Some(gradient) = style.get().gradient() {
            low.set(gradient.range[0].to_string());
            high.set(gradient.range[1].to_string());
        }
    });
    on_cleanup_local(move || drop(sync));
    let apply = move || {
        let result = low
            .get_untracked()
            .parse::<f64>()
            .ok()
            .zip(high.get_untracked().parse::<f64>().ok())
            .ok_or_else(|| "Enter numeric color limits".to_owned())
            .and_then(|(low, high)| {
                change(pane, selected.get_untracked(), source, |color| {
                    if let Some(gradient) = color.gradient_mut() {
                        gradient.range = [low, high];
                    }
                })
            });
        error.set(result.err().unwrap_or_default());
    };
    let range = Memo::new(move |_| {
        selected.get().and_then(|id| {
            pane.map_colors.with(|colors| {
                colors
                    .scales
                    .get(&id)
                    .filter(|scale| scale.mode == style.get().mode)
                    .and_then(|scale| scale.data_range)
            })
        })
    });
    view! {column(class="map-trace-colors") {
        row(class="map-trace-color-modes",a11y:role=Role::Group,a11y:label="Color by") {
            for choice in move ||[(Mode::Solid,"Solid"),(Mode::Speed,"Speed"),(Mode::Height,"Altitude"),(Mode::Field,"Field")],key=|(_,label)|*label {
                ActionButton(class="map-trace-color-mode",class:setting-active=move ||style.get().mode==choice.0,on:click=move |_|{
                    if let Err(message)=change(pane,selected.get_untracked(),source,|color|color.mode=choice.0){workbench.status.set(message);}
                }) {{choice.1}}
            }
        }
        if move ||style.get().mode!=Mode::Solid {
            row(class="map-trace-color-range") {
                Input(class="map-trace-color-low",value=low,a11y:label="Range minimum",on:focus_out=move |_|apply(),on:key_down=move |e|if e.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){apply();})
                text {"to"}
                Input(class="map-trace-color-high",value=high,a11y:label="Range maximum",on:focus_out=move |_|apply(),on:key_down=move |e|if e.key==zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter){apply();})
                text {{move ||if style.get().mode==Mode::Speed {"m/s"}else if style.get().mode==Mode::Height {"m"}else{""}}}
                ActionButton(class="map-trace-color-auto",disabled=Signal::derive_local(move ||style.get().mode!=Mode::Field && range.get().is_none_or(|[low,high]|high<=low)),on:click=move |_|{
                    let range=range.get_untracked();
                    let _=change(pane,selected.get_untracked(),source,|color|{
                        let field=color.mode==Mode::Field;
                        if let Some(gradient)=color.gradient_mut(){if field{gradient.range=[0.,0.];}else if let Some([low,high])=range && high>low {gradient.range=[low,high];}}
                    });
                }) {"Auto"}
            }
            row(class="map-trace-color-ramp") {for bucket in move ||0..32,key=|bucket|*bucket {
                box(style:background=move ||style.get().gradient().map(|gradient|{let [r,g,b]=gradient.color(f64::from(bucket)/31.);format!("#{r:02x}{g:02x}{b:02x}")})) {}
            }}
        }
        if move ||!error.get().is_empty() {text(class="trace-gradient-error",a11y:role=Role::Alert){{move ||error.get()}}}
    }}
}
