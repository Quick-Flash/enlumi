use super::{color_settings::ColorSettingsProps, trace_markers::TraceMarkersProps};
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
    state_tint_options::StateTintOptionsProps,
    trace_color,
};
use plot::zgui_series::LineStyleProps;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn MapLineSettings(
    workbench: Workbench,
    pane: Pane,
    open: RwSignal<bool, LocalStorage>,
) -> impl IntoView {
    let selected = Signal::derive_local(move || trace_color::selected(pane));
    let selection = RwSignal::new_local(
        selected
            .get_untracked()
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
    );
    let anchor = RwSignal::new_local(selected.get_untracked());
    let ids = Signal::derive_local(move || {
        let mut ids = pane.view_lines.with(|lines| {
            lines
                .iter()
                .filter(|line| selection.with(|set| set.contains(&line.id)))
                .map(|line| line.id)
                .collect::<Vec<_>>()
        });
        if ids.is_empty() {
            ids.extend(selected.get());
        }
        ids
    });
    view! {column(class="map-line-settings") {
        column(class="map-line-selection") {
            row(class="line-selection-heading") {
                text(class="heading") {{move ||format!("{} {} selected",ids.with(Vec::len),if ids.with(Vec::len)==1{"line"}else{"lines"})}}
                Tooltip {
                    TooltipTrigger {Button(size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,class="line-selection-info",a11y:label="How to select and edit lines") {vector(class="info-icon",prop:viewBox="0 0 20 20",prop:d="M10 1 A9 9 0 1 1 10 19 A9 9 0 1 1 10 1 M10 9 V15 M10 5 V6",a11y:hidden=true)}}
                    TooltipContent(class="line-selection-help") {"Click selects one line. Ctrl-click toggles lines; one line always stays selected. Shift-click selects a range. Appearance, state markers and direction arrows apply to every selected line."}
                }
            }
            column(class="line-list") {
                for line in move ||pane.view_lines.get(),key=|line|line.id {
                    control(class="line-button map-line-choice",tabindex=Focus::Sequential,a11y:role=Role::Button,class:selected=move ||ids.with(|ids|ids.contains(&line.id)),a11y:toggled_on=move ||ids.with(|ids|ids.contains(&line.id)),
                        style:color=move ||Some(pane.plot.style(&line.id.to_string()).css_color()),style:background_color=move ||{let [r,g,b,_]=pane.plot.style(&line.id.to_string()).color;Some(format!("rgba({r},{g},{b},{})",if ids.with(|ids|ids.contains(&line.id)){0.14}else{0.02}))},
                        on:click=move |event|{
                            let current=ids.get_untracked();let order=pane.view_lines.with_untracked(|lines|lines.iter().map(|line|line.id).collect::<Vec<_>>());
                            selection.update(|set|{*set=current.into_iter().collect();
                                if event.modifiers.shift() {if let Some((a,b))=anchor.get_untracked().and_then(|id|Some((order.iter().position(|key|*key==id)?,order.iter().position(|key|*key==line.id)?))){if !event.modifiers.control(){set.clear();}set.extend(&order[a.min(b)..=a.max(b)]);}}
                                else if event.modifiers.control(){if !set.contains(&line.id){set.insert(line.id);}else if set.len()>1{set.remove(&line.id);}}
                                else{set.clear();set.insert(line.id);}
                            });if !event.modifiers.shift(){anchor.set(Some(line.id));}workbench.select(pane.id,line.id);
                        }) {
                        text(class="line-selection-mark",a11y:hidden=true) {{move ||if ids.with(|ids|ids.contains(&line.id)){"\u{2713}"}else{""}}}
                        box(class="line-color-swatch",style:background=move ||Some(pane.plot.style(&line.id.to_string()).css_color())){}
                        text(class="line-selection-name") {{line.name}}
                    }
                }
            }
        }
        if move ||selected.get().is_some() {
            TraceMarkers(workbench=workbench,pane=pane,selected=selected,ids=Some(ids))
            if move ||ids.with(Vec::len)==1 {ColorSettings(workbench=workbench,pane=pane)}
            LineStyle(ids=ids,pane=pane.display(),show_connection=false)
            StateTintOptions(workbench=workbench,pane=pane)
            ActionButton(class="map-edit-coordinates",on:click=move |_|{workbench.edit_trajectory(pane);open.set(false);}) {"Edit coordinates"}
        }
    }}
}
