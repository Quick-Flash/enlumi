use crate::{
    geometry_state::GeometryPart,
    state::{Pane, Workbench},
};
use kurbo::Point;
use plot::{PlotGeometry, zgui_renderer::local_position};
use zgui::prelude::*;
#[component]
pub fn GeometryHandle(
    workbench: Workbench,
    pane: Pane,
    id: u64,
    index: usize,
    host: NodeRef,
    geometry: Memo<Option<PlotGeometry>>,
) -> impl IntoView {
    let part = if index == 4 {
        GeometryPart::Move
    } else {
        GeometryPart::Handle(index)
    };
    let position = Memo::new(move |_| {
        workbench.geometry.items.track();
        let item = workbench.geometry_item(id)?;
        let g = geometry.get()?;
        let ranges = pane.plot.ranges.get();
        if index == 4 {
            item.move_handle(ranges, g)
        } else if item.kind == crate::geometry::GeometryKind::Band {
            Some(Point::new(
                g.x(item.coordinates[if index == 0 { 0 } else { 2 }], ranges.x),
                g.height * 0.5,
            ))
        } else {
            item.handles()
                .get(index)
                .map(|point| g.data_transform(ranges) * *point)
        }
    });
    let color = move || {
        workbench.geometry.items.track();
        workbench.geometry_item(id).map(|item| {
            format!(
                "#{:02x}{:02x}{:02x}",
                item.color[0], item.color[1], item.color[2]
            )
        })
    };
    let cursor = move || {
        workbench.geometry.items.track();
        let dragging = workbench.geometry.drag.get().is_some_and(|drag| {
            drag.pane == pane.id && drag.original.id == id && drag.part == part
        });
        workbench.geometry_item(id).map(|item| {
            crate::geometry_cursor::handle_cursor(
                &item,
                if index == 4 { None } else { Some(index) },
                dragging,
            )
            .to_string()
        })
    };
    view! {control(class="geometry-handle",class:geometry-move-handle=move || index==4,class:geometry-endpoint-handle=move || index!=4,tabindex=Focus::Sequential,
        style:left=move || position.get().map(|p|format!("{}px",p.x-9.)),style:top=move ||position.get().map(|p|format!("{}px",p.y-9.)),style:display=move || if position.get().is_some(){None}else{Some("none".into())},style:color=color,style:cursor=cursor,
        a11y:label=if index==4 {"Move geometry"}else{"Resize geometry endpoint or corner"},
        on:pointer_down=move |event| {
            if event.button!=Some(zgui::vocab::PointerButton::Primary){return;}
            if let (Some((x,y)),Some(g))=(local_position(&host,event.position),geometry.get_untracked()){workbench.begin_geometry_handle(pane,Point::new(x,y),g,part);}
            event.prevent_default();
        },
        on:key_down=move |event| {
            use zgui::vocab::{Key,NamedKey,Modifiers};
            let step=if event.modifiers.contains(Modifiers::SHIFT){10.}else{1.};
            let delta=match event.key {Key::Named(NamedKey::ArrowLeft)=>(-step,0.),Key::Named(NamedKey::ArrowRight)=>(step,0.),Key::Named(NamedKey::ArrowUp)=>(0.,-step),Key::Named(NamedKey::ArrowDown)=>(0.,step),_=>return};
            if let (Some(point),Some(g))=(position.get_untracked(),geometry.get_untracked()) && workbench.begin_geometry_handle(pane,point,g,part) {
                workbench.move_geometry(pane,point+kurbo::Vec2::from(delta),g);workbench.end_geometry();
            }
            event.prevent_default();event.stop_propagation();
        }) {
        if move || index==4 {vector(prop:viewBox="0 0 18 18",prop:d=move || PropValue::from(match cursor().as_deref(){Some("ew-resize")=>"M2 9 H16 M5 6 L2 9 L5 12 M13 6 L16 9 L13 12",Some("ns-resize")=>"M9 2 V16 M6 5 L9 2 L12 5 M6 13 L9 16 L12 13",_=>"M9 2 V16 M2 9 H16 M6 5 L9 2 L12 5 M6 13 L9 16 L12 13 M5 6 L2 9 L5 12 M13 6 L16 9 L13 12"}),a11y:hidden=true)} else {box(class="geometry-resize-mark") {}}
    }}
}
