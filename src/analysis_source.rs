use crate::{action_button::ActionButtonProps, state::Workbench};
use plot::zgui_workspace::{SeriesSourceActions, SeriesSourceProps};
use zgui::prelude::*;

#[component]
pub fn AnalysisSource(workbench: Workbench, source: String) -> impl IntoView {
    let source = StoredValue::new_local(source);
    let plain_click = RwSignal::new_local(true);
    let id = move || format!("@analysis/{}", source.get_value());
    let actions = SeriesSourceActions {
        on_move: UnsyncCallback::new(move |(x, y)| workbench.field_drag_move(x, y)),
        on_drop: UnsyncCallback::new(move |(source, x, y)| workbench.drop_fields(source, x, y)),
        on_cancel: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
        on_finish: UnsyncCallback::new(move |()| workbench.finish_field_drag()),
    };
    view! {
        row(class="analysis-input") {
            row(class="field-leaf", on:pointer_down=move |event| {
                if event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                plain_click.set(!event.modifiers.contains(zgui::vocab::Modifiers::CONTROL) && !event.modifiers.contains(zgui::vocab::Modifiers::SHIFT));
                let visible = workbench.analysis().sources.with_untracked(|sources| sources.iter().map(|source| format!("@analysis/{source}")).collect::<Vec<_>>());
                workbench.select_field(id(), event.modifiers.contains(zgui::vocab::Modifiers::CONTROL), event.modifiers.contains(zgui::vocab::Modifiers::SHIFT), &visible);
            }, on:pointer_move=move |_| {
                if workbench.drag.state.get_untracked().is_some_and(|drag| drag.3) { plain_click.set(false); }
            }, on:pointer_cancel=move |_| plain_click.set(true), on:click=move |_| {
                if plain_click.get_untracked() {
                    workbench.selected_fields.set(std::collections::BTreeSet::from([id()]));
                    workbench.selected_field.set(Some(id()));
                    workbench.selection_anchor.set(Some(id()));
                }
                plain_click.set(true);
            }, on:key_down=move |event| {
                if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter) {
                    workbench.drop_analysis_sources(vec![source.get_value()], Some(workbench.active().get_untracked()));
                    event.prevent_default(); event.stop_propagation();
                }
            }) {
                SeriesSource(id=Signal::derive_local(move || Some(id())),
                    label=Signal::derive_local(move || workbench.field_name(&source.get_value())),
                    count=Signal::derive_local(move || {
                        let source = source.get_value();
                        let count = workbench.derived.with(|fields| fields.iter().find(|field| field.key() == source).and_then(|field| field.data.as_ref().map(|data| data.samples().len())))
                            .or_else(|| workbench.log.with(|log| log.as_ref().and_then(|log| log.catalogue.iter().find(|field| field.id == source).map(|field| field.count)))).unwrap_or(0);
                        count.to_string()
                    }),
                    selected=Signal::derive_local(move || workbench.selected_fields.with(|fields| fields.contains(&id()))),
                    drag=workbench.drag, actions=actions)
            }
            ActionButton(icon=true,a11y:label="Remove analysis input",on:click=move |_| {
                let source = source.get_value();
                workbench.analysis().cancel();
                workbench.analysis().sources.update(|all| all.retain(|id| id != &source));
                workbench.analysis().disabled.update(|all| { all.remove(&source); });
                workbench.selected_fields.update(|all| { all.remove(&id()); });
                if workbench.selected_field.get_untracked() == Some(id()) { workbench.selected_field.set(None); }
            }) {"×"}
        }
    }
}
