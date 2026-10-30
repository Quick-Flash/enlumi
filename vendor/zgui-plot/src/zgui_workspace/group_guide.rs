use super::{DockState, GroupGuide};
use crate::workspace::layout::DropZone;
use zgui::prelude::*;

#[component]
pub fn GroupDockGuide(dock: DockState, guide: GroupGuide, origin: (f64, f64)) -> impl IntoView {
    let guide = StoredValue::new_local(guide);
    let selected = move || {
        dock.group_target.with(|target| {
            target
                .as_ref()
                .is_some_and(|(members, _)| guide.with_value(|g| members == &g.members))
        })
    };
    view! {
        column(class="dock-group-set", class:hidden=move || {
            let (x,y)=dock.pointer.get();
            guide.with_value(|g|g.count>1 && (g.center.0-f64::from(x)).hypot(g.center.1-f64::from(y))>120.0 && !selected())
        }) {
            column(class="dock-group-scope",class:hidden=move || !selected(),
                style:left=Some(format!("{}px",guide.get_value().rect.x-origin.0)),style:top=Some(format!("{}px",guide.get_value().rect.y-origin.1)),
                style:width=Some(format!("{}px",guide.get_value().rect.width)),style:height=Some(format!("{}px",guide.get_value().rect.height))) {}
            text(class="dock-group-label",style:left=Some(format!("{}px",guide.get_value().center.0-origin.0-10.0)),style:top=Some(format!("{}px",guide.get_value().center.1-origin.1-10.0))) {{guide.get_value().count.to_string()}}
            for item in move || [(DropZone::Left,"left","\u{2190}"),(DropZone::Right,"right","\u{2192}"),(DropZone::Top,"above","\u{2191}"),(DropZone::Bottom,"below","\u{2193}")],key=|item|item.1 {
                text(class="dock-group-guide",class:dock-group-active=move || dock.group_target.with(|target|target.as_ref().is_some_and(|(members,zone)|*zone==item.0 && guide.with_value(|g|members==&g.members))),
                    a11y:label=format!("Dock {} region of {} panes",item.1,guide.get_value().count),
                    style:left=Some(format!("{}px",guide.get_value().button(item.0).x-origin.0)),style:top=Some(format!("{}px",guide.get_value().button(item.0).y-origin.1))) {{item.2}}

            }
        }
    }
}
