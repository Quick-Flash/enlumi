use crate::geometry_handle::GeometryHandleProps;
use crate::geometry_inspector::GeometryInspectorProps;
use crate::{
    action_button::ActionButtonProps,
    state::{Pane, Workbench},
};
use kurbo::Point;
use plot::{PlotGeometry, zgui_renderer::local_position};
use zgui::{
    canvas::{Brush, ShapeBuilder, zgui_color::Color},
    prelude::*,
};
fn flat(color: Color) -> Brush {
    Brush::Linear {
        start: (0., 0.).into(),
        end: (1., 0.).into(),
        stops: vec![(0., color), (1., color)],
        repeating: false,
    }
}
#[component]
pub fn GeometryOverlay(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let host = NodeRef::new();
    let bounds = host.observe_border_box();
    workbench.geometry.hosts.update_value(|hosts| {
        hosts.insert(pane.id, host);
    });
    on_cleanup_local(move || {
        workbench.geometry.hosts.update_value(|hosts| {
            if hosts.get(&pane.id) == Some(&host) {
                hosts.remove(&pane.id);
            }
        });
    });
    let geometry = Memo::new(move |_| {
        let size = bounds.get()?.size;
        let w = f64::from(size.width.0 / host.scale());
        let h = f64::from(size.height.0 / host.scale());
        (w > 0. && h > 0.)
            .then(|| PlotGeometry::with_bounds(0., 0., w, h, w, h))
            .flatten()
    });
    let editing = move || {
        visible.get()
            && workbench.geometry.editing.get()
            && (workbench.geometry.tool.get().is_some()
                || workbench.geometry.selected.get().is_some_and(|id| {
                    pane.geometry.with(|ids| {
                        ids.contains(&id)
                            && !pane.hidden_geometry.with(|hidden| hidden.contains(&id))
                    })
                }))
    };
    let handle = zgui::elements::canvas::CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Foreground);
    let drawing = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        drawing.draw(|canvas_scene| {
            canvas_scene.clear();
            if !visible.get() {
                return;
            }
            let Some(geometry) = geometry.get() else {
                return;
            };
            let ranges = pane.plot.ranges.get();
            let ids = pane.geometry.get();
            let hidden = pane.hidden_geometry.get();
            let items = workbench.ordered_geometry(pane);
            for item in items
                .iter()
                .filter(|item| ids.contains(&item.id) && !hidden.contains(&item.id))
            {
                let (fill, stroke) = item.paths(ranges, geometry);
                let [r, g, b] = item.color;
                let color = Color::srgb_u8(r, g, b, 255);
                if !fill.is_empty() {
                    canvas_scene.push(
                        ShapeBuilder::new(fill)
                            .fill(flat(color.with_alpha(item.opacity as f32)))
                            .build(),
                    );
                }
                if !stroke.is_empty() {
                    canvas_scene.push(
                        ShapeBuilder::new(stroke)
                            .stroke(flat(color), item.width)
                            .build(),
                    );
                }
            }
        })
    });
    let canvas = zgui::elements::canvas()
        .class("geometry-canvas")
        .scene(&handle);
    view! { column(class="geometry-overlay",node_ref=host,style:cursor=move ||workbench.geometry_cursor(pane.id),class:geometry-drop-target=move || workbench.geometry.target.get()==Some(pane.id),
        on:pointer_down=move |event| {if workbench.geometry.drag.get_untracked().is_some_and(|drag|drag.pane==pane.id) || workbench.geometry.editor_drag.get_untracked().is_some_and(|drag|drag.pane==pane.id){event.capture_pointer();event.prevent_default();event.stop_propagation();}},
        on:pointer_move=move |event| {workbench.move_geometry_editor(pane.id,event.position);if workbench.geometry.drag.get_untracked().is_some_and(|drag|drag.pane==pane.id){if let (Some((x,y)),Some(g))=(local_position(&host,event.position),geometry.get_untracked()){workbench.move_geometry(pane,Point::new(x,y),g);}event.stop_propagation();}},
        on:pointer_up=move |event| {if workbench.geometry.drag.get_untracked().is_some_and(|drag|drag.pane==pane.id){if let (Some((x,y)),Some(g))=(local_position(&host,event.position),geometry.get_untracked()){workbench.move_geometry(pane,Point::new(x,y),g);}workbench.end_geometry();}
            if workbench.geometry.editor_drag.get_untracked().is_some_and(|drag|drag.pane==pane.id){workbench.finish_geometry_editor(false);}event.release_pointer();},
        on:pointer_cancel=move |event| {if workbench.geometry.drag.get_untracked().is_some_and(|drag|drag.pane==pane.id){workbench.cancel_geometry_drag();}
            if workbench.geometry.editor_drag.get_untracked().is_some_and(|drag|drag.pane==pane.id){workbench.finish_geometry_editor(true);}event.release_pointer();}) {
        {canvas.into_view()}
        if move || editing() && (workbench.geometry.tool.get().is_some() || workbench.geometry.drag.get().is_some_and(|drag|drag.part==crate::geometry_state::GeometryPart::Create)) {
            control(class="geometry-interaction",tabindex=Focus::Sequential,a11y:label="Edit geometry: drag shape or handles; Escape finishes editing",
                on:pointer_down=move |event| {
                    if event.button != Some(zgui::vocab::PointerButton::Primary) { return; }
                    if let (Some((x,y)),Some(g)) = (local_position(&host,event.position),geometry.get_untracked()) { workbench.begin_geometry(pane,Point::new(x,y),g); }
                    event.prevent_default();
                }) {}
            row(class="geometry-edit-hint") {text {{move || if workbench.geometry.tool.get().is_some() {"Drag to draw"} else {"Drag shape or handles"}}} ActionButton(class="geometry-done",on:click=move |_| workbench.finish_geometry_editing()) {"Done"} }
        }
        if move || editing() && workbench.geometry.tool.get().is_none() && !workbench.geometry.drag.get().is_some_and(|drag|drag.part==crate::geometry_state::GeometryPart::Create) {
            for id in move || workbench.geometry.selected.get().into_iter().collect::<Vec<_>>(),key=|id|*id {
                for index in move || {workbench.geometry.items.track();if workbench.geometry_item(id).is_some_and(|item|item.states.is_some()){vec![]}else if workbench.geometry_item(id).is_some_and(|item|item.kind==crate::geometry::GeometryKind::Rectangle){vec![0,1,2,3,4]}else{vec![0,1,4]}},key=|index|*index {
                    GeometryHandle(workbench=workbench,pane=pane,id=id,index=index,host=host,geometry=geometry)
                }
            }
        }
        if move || visible.get() && workbench.active.get()==pane.id && workbench.geometry.editing.get() && workbench.geometry.tool.get().is_none() {
            for id in move || workbench.geometry.selected.get().filter(|id| pane.geometry.with(|ids|ids.contains(id)) && !pane.hidden_geometry.with(|ids|ids.contains(id))).into_iter().collect::<Vec<_>>(),key=|id| *id {
                GeometryInspector(workbench=workbench,id=id)
            }
        }
    } }
}
