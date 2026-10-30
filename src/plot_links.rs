use crate::state::Pane;
use zgui::prelude::*;
use zgui_ui::prelude::*;

#[component]
pub fn PlotLinksMenu(pane: Pane) -> impl IntoView {
    install_stylesheet("enlumi-plot-links", include_str!("plot_links.css"));
    let open = RwSignal::new_local(false);
    view! {DropdownMenu(open=open) {
        DropdownMenuTrigger(size=ButtonSize::Xs,variant=ButtonVariant::Outline,class="plot-links-trigger",class:links-open=move ||open.get(),class:links-active=move ||{let links=pane.links.get();links.x || links.y || links.pointer || pane.timeline_link.get()},a11y:label="Linked plots") {
            text(class="plot-links-label") {{move ||{
                let links=pane.links.get();let timeline=pane.timeline_link.get();
                if links.x && links.y && links.pointer && timeline {return "Links: all".to_owned();}
                let selected=[(links.x,"X"),(links.y,"Y"),(links.pointer,"pointer"),(timeline,"timeline")].into_iter().filter_map(|(enabled,label)|enabled.then_some(label)).collect::<Vec<_>>();
                format!("Links: {}",if selected.is_empty(){"off".into()}else{selected.join(", ")})
            }}}
            vector(class="plot-links-chevron",prop:viewBox="0 0 12 12",prop:d="M3 4 L6 7 L9 4",a11y:hidden=true)
        }
        DropdownMenuContent(class="plot-links-surface") {column(class="link-controls") {
            for item in move || [(3,"All"),(0,"Link X bounds"),(1,"Link Y bounds"),(2,"Link pointer"),(4,"Link timeline")],key=|item|item.0 {
                MenuCheckboxItem(class="link-option",close_on_select=false,checked=Binding::controlled(Signal::derive_local(move ||{let links=pane.links.get();match item.0{0=>links.x,1=>links.y,2=>links.pointer,4=>pane.timeline_link.get(),_=>links.x && links.y && links.pointer && pane.timeline_link.get()}}),move |value:bool|{
                    if item.0==3 || item.0==4 {pane.timeline_link.set(value);}
                    if item.0!=4 {pane.links.update(|links|match item.0{0=>links.x=value,1=>links.y=value,2=>links.pointer=value,_=>{links.x=value;links.y=value;links.pointer=value;}});}
                })) {{item.1}}
            }
        }}
    }}
}
