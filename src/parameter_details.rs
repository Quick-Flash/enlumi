use crate::{action_button::ActionButtonProps, state::Workbench};
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn ParameterDetails(workbench: Workbench) -> impl IntoView {
    let parameter = Memo::new(move |_| {
        let id = workbench.selected_field.get()?;
        workbench.log.with(|log| {
            let log = log.as_ref()?;
            Some((
                id.clone(),
                log.setting(&id)?.clone(),
                log.setting_number(&id),
            ))
        })
    });
    let unavailable = Signal::derive_local(move || {
        workbench.loading.get() || workbench.profiles.busy.get() || workbench.derived_busy.get()
    });
    view! {
        if move || parameter.get().is_some() {
            column(class = "parameter-details") {
                text(class = "heading") {{move || parameter.get().map_or_else(String::new, |(_, setting, _)| setting.name)}}
                text(class = "muted") {{move || parameter.get().map_or_else(String::new, |(id, setting, number)| {
                    let element = id.split('/').nth(3).map_or_else(String::new, |i| format!(" · element {i}"));
                    format!("{}{element} · {}", setting.kind, if number.is_some() { "numeric constant" } else { "recorded text" })
                })}}
                text(class="muted") {{move ||parameter.get().map(|(_,s,_)|s.timing()).unwrap_or_default()}}
                Textarea(class = "parameter-value", read_only = true, label = "Recorded parameter value",
                    value = Binding::controlled(Signal::derive_local(move || parameter.get().map_or_else(String::new, |(id, setting, number)| {
                        if id.split('/').nth(3).is_some() { number.map_or(setting.value, |value| value.to_string()) } else { setting.value }
                    })), |_| {}))
                if move || parameter.get().is_some_and(|(_, _, number)| number.is_some()) {
                    row(class = "parameter-actions") {
                        ActionButton(class = "parameter-plot", disabled = unavailable, on:click = move |_| {
                            if let Some((id, _, _)) = parameter.get_untracked() { workbench.add(id); }
                        }) { "Add to plot" }
                        ActionButton(class = "parameter-filter", disabled = unavailable, on:click = move |_| {
                            if let Some((id, _, _)) = parameter.get_untracked() {
                                match workbench.accept_filter_sources(vec![id]) {
                                    Ok(()) => workbench.filters_open.set(true),
                                    Err(error) => { workbench.filters_open.set(true); workbench.filter_error.set(error); }
                                }
                            }
                        }) { "Use in filter" }
                    }
                }
            }
        }
    }
}
