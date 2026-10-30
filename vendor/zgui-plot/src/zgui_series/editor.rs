use super::{SeriesActions, SeriesPane as Pane, line_style::LineStyleProps};
use std::collections::BTreeSet;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn SeriesEditor(
    actions: SeriesActions,
    #[prop(optional)] fixed_connection: Option<crate::line_draw::Connection>,
    pane: Pane,
    #[prop(optional)] on_extra: Option<UnsyncCallback<()>>,
    #[prop(attrs)] attrs: Attrs,
) -> impl IntoView {
    super::install();
    let selection = RwSignal::new_local(
        pane.selected
            .get_untracked()
            .filter(|id| {
                pane.lines
                    .with_untracked(|lines| lines.iter().any(|line| line.id == *id))
            })
            .or_else(|| {
                pane.lines
                    .with_untracked(|lines| lines.first().map(|line| line.id))
            })
            .into_iter()
            .collect::<BTreeSet<_>>(),
    );
    let anchor = RwSignal::new_local(pane.selected.get_untracked());
    let ids = Signal::derive_local(move || {
        pane.lines.with(|lines| {
            let mut ids = lines
                .iter()
                .filter(|line| selection.with(|selected| selected.contains(&line.id)))
                .map(|line| line.id)
                .collect::<Vec<_>>();
            if ids.is_empty()
                && let Some(line) = lines.first()
            {
                ids.push(line.id);
            }
            ids
        })
    });
    view! {
        column(class = "editor", {..attrs}) {
            row(class = "line-selection-heading") {
                text(class = "heading") {{move || { let count = ids.with(Vec::len); format!("{count} {} selected", if count == 1 { "line" } else { "lines" }) }}}
                Tooltip {
                    TooltipTrigger { Button(size = ButtonSize::IconXs, variant = ButtonVariant::Ghost, class = "line-selection-info", a11y:label = "How to select and edit lines") {
                        vector(class = "info-icon", prop:viewBox = "0 0 20 20", prop:d = "M10 1 A9 9 0 1 1 10 19 A9 9 0 1 1 10 1 M10 9 V15 M10 5 V6", a11y:hidden = true)
                    } }
                    TooltipContent(class = "line-selection-help") { "Click selects one line. Ctrl-click toggles lines; one line always stays selected. Shift-click selects a range. Width, line type, markers, and connection apply to every selected line; mixed values change together. Color controls stay visible and disabled until one line is selected. " text { {actions.extra_help} } }
                }
            }
            column(class = "line-list") {
                for line in move || pane.lines.get(), key = |line| line.id {
                    control(class = "line-button", tabindex = Focus::Sequential, a11y:role = Role::Button,
                        style:color = move || Some(pane.plot.style(&line.id.to_string()).css_color()),
                        style:background-color = move || { let [r,g,b,_] = pane.plot.style(&line.id.to_string()).color; Some(format!("rgba({r}, {g}, {b}, {})", if ids.with(|ids| ids.contains(&line.id)) { 0.14 } else { 0.02 })) },
                        class:selected = move || ids.with(|ids| ids.contains(&line.id)),
                        a11y:toggled_on = move || ids.with(|ids| ids.contains(&line.id)),
                        on:click = move |event| {
                            let order = pane.lines.with_untracked(|lines| lines.iter().map(|line| line.id).collect::<Vec<_>>());
                            let current = ids.get_untracked();
                            selection.update(|selected| {
                                *selected = current.iter().copied().collect();
                                select_lines(selected, anchor.get_untracked(), line.id,
                                    event.modifiers.contains(zgui::vocab::Modifiers::CONTROL), event.modifiers.shift(), &order);
                            });
                            if !event.modifiers.shift() { anchor.set(Some(line.id)); }
                            pane.selected.set(Some(line.id)); actions.select.run((pane.id, line.id));
                        }) {
                        text(class = "line-selection-mark", a11y:hidden = true) {{move || if ids.with(|ids| ids.contains(&line.id)) { "\u{2713}" } else { "" }}}
                        box(class = "line-color-swatch", style:background-color = move || Some(pane.plot.style(&line.id.to_string()).css_color())) {}
                        text(class = "line-selection-name") {{move || pane.lines.with(|lines| lines.iter().find(|current| current.id == line.id).map_or_else(String::new, |current| current.label()))}}
                    }
                }
            }
            if move || !ids.with(Vec::is_empty) {
                LineStyle(ids = ids, pane = pane, show_connection = fixed_connection.is_none())
            } else {
                text(class = "editor-empty muted") { "Select lines to edit their appearance." }
            }
            if move || actions.extra.is_some() {
            Button(size = ButtonSize::Sm, class = actions.extra_class, disabled = Signal::derive_local(move || pane.lines.with(Vec::is_empty)), on:click = move |_| {
                if let Some(action) = actions.extra { action.run(pane.id); }
                if let Some(on_extra) = on_extra { on_extra.run(()); }
            }) {{actions.extra_label}}
            }
        }
    }
}

fn select_lines(
    selected: &mut BTreeSet<u64>,
    anchor: Option<u64>,
    id: u64,
    ctrl: bool,
    shift: bool,
    order: &[u64],
) {
    if shift
        && let Some((start, end)) = anchor.and_then(|anchor| {
            Some((
                order.iter().position(|line| *line == anchor)?,
                order.iter().position(|line| *line == id)?,
            ))
        })
    {
        if !ctrl {
            selected.clear();
        }
        selected.extend(&order[start.min(end)..=start.max(end)]);
    } else if ctrl {
        if selected.contains(&id) {
            if selected.len() > 1 {
                selected.remove(&id);
            }
        } else {
            selected.insert(id);
        }
    } else {
        *selected = BTreeSet::from([id]);
    }
}
