use crate::{
    geometry::{GeometryKind, LineOrientation},
    state::Workbench,
};
use zgui::prelude::*;
use zgui_ui::prelude::*;
#[component]
pub fn GeometryCoordinates(workbench: Workbench, id: u64) -> impl IntoView {
    let item = move || {
        workbench.geometry.items.track();
        workbench.geometry_item(id)
    };
    let constrained = Memo::new(move |_| {
        item().is_some_and(|item| {
            item.kind == GeometryKind::Line && item.orientation != LineOrientation::Free
        })
    });
    view! {column(class="geometry-coordinate-grid") {
        if move ||item().is_some_and(|item|item.kind==GeometryKind::Band) {
            row(class="geometry-coordinate-row") {
                for index in move ||[0usize,2],key=|index|*index {column {text(class="muted") {{if index==0{"Start (s)"}else{"End (s)"}}} Input(class="geometry-coordinate",value=Binding::controlled(Signal::derive_local(move ||workbench.geometry.coordinates.with(|values|values[index].clone())),move |value:String|workbench.geometry_coordinates(index,value)),a11y:label=if index==0{"Band start time"}else{"Band end time"})}}
            }
        }else{
        if move || constrained.get() {
            row(class="geometry-coordinate-row geometry-level-row") {
                text(class="geometry-point-name muted") {"Position"}
                for index in move || if item().is_some_and(|item|item.orientation==LineOrientation::Horizontal){vec![1]}else{vec![0]},key=|index|*index {
                    column {text(class="muted") {{if index==1 {"Y (horizontal)"}else{"X (vertical)"}}}
                        Input(class="geometry-coordinate geometry-level",value=Binding::controlled(Signal::derive_local(move ||workbench.geometry.coordinates.with(|v|v[index].clone())),move |value:String|workbench.geometry_coordinates(index,value)),a11y:label="Line position")
                    }
                }
            }
            if move ||item().is_some_and(|item|!item.extend) {
                row(class="geometry-coordinate-row") {
                    text(class="geometry-point-name muted") {"Ends"}
                    for index in move || if item().is_some_and(|item|item.orientation==LineOrientation::Horizontal){vec![0,2]}else{vec![1,3]},key=|index|*index {
                        column {text(class="muted") {{["X1","Y1","X2","Y2"][index]}} Input(class="geometry-coordinate",value=Binding::controlled(Signal::derive_local(move ||workbench.geometry.coordinates.with(|v|v[index].clone())),move |value:String|workbench.geometry_coordinates(index,value)),a11y:label=["Geometry X1","Geometry Y1","Geometry X2","Geometry Y2"][index])}
                    }
                }
            }
        } else {
            for pair in move || [0usize,2],key=|pair|*pair {
                row(class="geometry-coordinate-row") {
                    text(class="geometry-point-name muted") {{move || match (item().map(|v|v.kind),pair){(Some(GeometryKind::Rectangle),0)=>"Corner 1",(Some(GeometryKind::Rectangle),_)=>"Corner 2",(_,0)=>"Start",_=>"End"}}}
                    for index in move || pair..pair+2,key=|index|*index {
                        column {text(class="muted") {{if index%2==0 {"X"}else{"Y"}}} Input(class="geometry-coordinate",value=Binding::controlled(Signal::derive_local(move ||workbench.geometry.coordinates.with(|v|v[index].clone())),move |value:String|workbench.geometry_coordinates(index,value)),a11y:label=["Geometry X1","Geometry Y1","Geometry X2","Geometry Y2"][index])}
                    }
                }
            }
        }
        }
    }}
}
