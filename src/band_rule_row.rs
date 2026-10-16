use crate::{action_button::ActionButtonProps, geometry_inspector::change, state::Workbench};
use plot::zgui_series::ColorPickerProps;
use zgui::{prelude::*, reactive::RenderEffect};
use zgui_ui::prelude::*;
#[component]
pub fn BandRuleRow(workbench: Workbench, id: u64, value: i64) -> impl IntoView {
    let rule = move || {
        workbench.geometry.items.track();
        workbench.geometry_item(id).and_then(|item| {
            item.states.as_ref().and_then(|states| {
                states
                    .rules
                    .iter()
                    .find(|rule| rule.value == value)
                    .cloned()
            })
        })
    };
    let draft_value = RwSignal::new_local(value.to_string());
    let name = RwSignal::new_local(rule().map_or_else(String::new, |rule| rule.name));
    let sync = RenderEffect::new(move |_| {
        if let Some(rule) = rule()
            && name.get_untracked() != rule.name
        {
            name.set(rule.name);
        }
    });
    on_cleanup_local(move || drop(sync));
    view! {row(class="band-rule-row") {
        Input(class="band-rule-value",a11y:label=format!("Value for state {value}"),value=draft_value,on:key_down=move |event| {if event.key == zgui::vocab::Key::Named(zgui::vocab::NamedKey::Enter) {workbench.edit_mapping_rule(id,Some(value),&draft_value.get_untracked(),&name.get_untracked());event.prevent_default();event.stop_propagation();}})
        ActionButton(class="band-set-value",disabled=Signal::derive_local(move ||draft_value.get()==value.to_string()),on:click=move |_|{workbench.edit_mapping_rule(id,Some(value),&draft_value.get_untracked(),&name.get_untracked());}) {"Set"}
        Input(class="band-rule-name",a11y:label=format!("Name for state {value}"),value=Binding::controlled(name,move |text:String| {name.set(text.clone());if text.trim().is_empty() || text.len()>256 {workbench.geometry.error.set("State name needs 1 to 256 bytes of text".into());return;}change(workbench,id,|item|{if let Some(states)=item.states.as_mut() && let Some(rule)=std::sync::Arc::make_mut(states).rules.iter_mut().find(|rule|rule.value==value){rule.name=text;}});workbench.geometry.error.set(String::new());}))
        Popover {
            PopoverTrigger(class="band-color-trigger",size=ButtonSize::IconXs,variant=ButtonVariant::Ghost,a11y:label=format!("Color for state {value}"),style:background=move ||rule().map(|rule|format!("#{:02x}{:02x}{:02x}",rule.color[0],rule.color[1],rule.color[2]))) {""}
            PopoverContent(class="band-color-popover") {
                ColorPicker(color=Signal::derive_local(move ||rule().map_or([235,174,82,255],|rule|[rule.color[0],rule.color[1],rule.color[2],255])),on_change=UnsyncCallback::new(move |color:[u8;4]|change(workbench,id,|item|{if let Some(states)=item.states.as_mut() && let Some(rule)=std::sync::Arc::make_mut(states).rules.iter_mut().find(|rule|rule.value==value){rule.color=[color[0],color[1],color[2]];}})))
            }
        }
        ActionButton(icon=true,class="band-remove-rule",a11y:label=format!("Remove state {value} mapping"),on:click=move |_|workbench.remove_mapping_rule(id,value)) {"X"}
    }}
}
