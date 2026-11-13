use crate::state::Pane;
use zgui::prelude::*;
use zgui_ui::prelude::*;

const LINK_ICON: &str = "M6.5 9.5 L9.5 6.5 M5 8 L3.5 9.5 A2.1 2.1 0 0 0 6.5 12.5 L8 11 M11 8 L12.5 6.5 A2.1 2.1 0 0 0 9.5 3.5 L8 5";

/// Maps own their camera: they link the timeline and, through it, highlight the
/// X range of linked time plots.
#[component]
pub fn PlotLinksMenu(pane: Pane) -> impl IntoView {
    install_stylesheet("enlumi-plot-links", include_str!("plot_links.css"));
    let open = RwSignal::new_local(false);
    let map = Memo::new(move |_| {
        pane.trajectory
            .with(|s| s.as_ref().is_some_and(|s| s.map.is_some()))
    });
    let range = Memo::new(move |_| range_link(pane));
    view! {DropdownMenu(open=open) {
        DropdownMenuTrigger(size=ButtonSize::Xs,variant=ButtonVariant::Outline,class="plot-links-trigger",class:links-open=move ||open.get(),class:links-active=move ||{let links=pane.links.get();(!map.get()&&(links.x || links.y)) || links.pointer || pane.timeline_link.get() || range.get()},a11y:label="Linked plots") {
            vector(class="plot-links-icon",prop:viewBox="0 0 16 16",prop:d=LINK_ICON,a11y:hidden=true)
            if move ||map.get() {box(class="plot-links-dot",a11y:hidden=true) {}}
            text(class="plot-links-label") {{move ||links_label(pane, map.get())}}
        }
        DropdownMenuContent(class="plot-links-surface") {column(class="link-controls") {
            for item in move ||links_items(map.get()),key=|item|item.0 {
                MenuCheckboxItem(class="link-option",close_on_select=false,checked=Binding::controlled(Signal::derive_local(move ||{let links=pane.links.get();match item.0{0=>links.x,1=>links.y,2=>links.pointer,4=>pane.timeline_link.get(),5=>range.get(),_=>links.x && links.y && links.pointer && pane.timeline_link.get()}}),move |value:bool|{
                    if item.0==5 {pane.trajectory.update(|s|if let Some(map)=s.as_mut().and_then(|s|s.map.as_mut()){map.range_link=value;});return;}
                    if item.0==3 || item.0==4 {pane.timeline_link.set(value);}
                    if item.0!=4 {pane.links.update(|links|match item.0{0=>links.x=value,1=>links.y=value,2=>links.pointer=value,_=>{links.x=value;links.y=value;links.pointer=value;}});}
                })) {{item.1}}
            }
        }}
    }}
}

fn links_label(pane: Pane, map: bool) -> String {
    let links = pane.links.get();
    let timeline = pane.timeline_link.get();
    let selected = if map {
        vec![
            (links.pointer, "Pointer"),
            (timeline, "Time"),
            (range_link(pane), "Range"),
        ]
    } else {
        vec![
            (links.x, "X"),
            (links.y, "Y"),
            (links.pointer, "Pointer"),
            (timeline, "Time"),
        ]
    };
    if !map && selected.len() > 1 && selected.iter().all(|(on, _)| *on) {
        return "All".to_owned();
    }
    let on = selected
        .into_iter()
        .filter_map(|(on, label)| on.then_some(label))
        .collect::<Vec<_>>();
    if on.is_empty() {
        "Unlinked".to_owned()
    } else {
        on.join(" · ")
    }
}

fn links_items(map: bool) -> Vec<(u8, &'static str)> {
    if map {
        vec![(2, "Pointer"), (4, "Timeline"), (5, "Time range")]
    } else {
        vec![
            (3, "All"),
            (0, "Link X bounds"),
            (1, "Link Y bounds"),
            (2, "Link pointer"),
            (4, "Link timeline"),
        ]
    }
}

/// Whether a map dims its path outside the X range of linked time plots.
fn range_link(pane: Pane) -> bool {
    pane.trajectory
        .with(|s| s.as_ref().and_then(|s| s.map).is_some_and(|m| m.range_link))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_links_name_the_enabled_demo_links() {
        let window = zgui_testkit_view::Window::open();
        let pane = window.scope.with(|| Pane::new(1));
        pane.trajectory.set(Some(crate::trajectory::Spec {
            map: Some(Default::default()),
            ..Default::default()
        }));
        assert_eq!(links_label(pane, true), "Pointer · Time · Range");
        pane.links.update(|links| links.pointer = false);
        assert_eq!(links_label(pane, true), "Time · Range");
        assert_eq!(
            links_items(true),
            [(2, "Pointer"), (4, "Timeline"), (5, "Time range")]
        );
    }
}
