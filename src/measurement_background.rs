use crate::state::{Pane, Workbench};
use kurbo::Shape;
use zgui::{
    canvas::{Brush, ShapeBuilder, zgui_color::Color},
    prelude::*,
};

#[component]
pub fn MeasurementBackground(
    workbench: Workbench,
    pane: Pane,
    visible: Signal<bool, LocalStorage>,
) -> impl IntoView {
    let host = NodeRef::new();
    let palette =
        zgui::reactive::use_local_context::<Signal<crate::theme::Palette, LocalStorage>>()
            .unwrap_or_else(|| Signal::derive_local(crate::theme::Palette::default));
    let bounds = host.observe_border_box();
    let region = Memo::new(move |_| {
        if !visible.get() {
            return None;
        }
        let measurement = workbench
            .measurement
            .selection
            .get()
            .filter(|selection| selection.pane == pane.id)?;
        let bounds = bounds.get()?;
        let width = f64::from(bounds.size.width.0 / host.scale());
        let height = f64::from(bounds.size.height.0 / host.scale());
        let geometry = plot::PlotGeometry::with_bounds(0., 0., width, height, width, height)?;
        let range = pane.plot.ranges.get().x;
        let x1 = geometry.x(measurement.start.x, range);
        let x2 = geometry.x(measurement.end.x, range);
        Some(kurbo::Rect::new(
            x1.min(x2),
            0.,
            x1.max(x2).max(x1.min(x2) + 1.),
            height,
        ))
    });
    let handle = zgui::elements::canvas::CanvasHandle::new();
    plot::zgui_svg::register_canvas(&handle, plot::zgui_svg::SnapshotLayer::Background);
    let drawing = handle.clone();
    plot::zgui_renderer::redraw_on_change(move || {
        let region = region.get();
        let [r, g, b, a] = palette.get().measurement_fill();
        drawing.draw(|scene| {
            scene.clear();
            if let Some(region) = region {
                scene.push(
                    ShapeBuilder::new(region.to_path(0.1))
                        .fill(Brush::Solid(Color::srgb_u8(r, g, b, a)))
                        .build(),
                );
            }
        });
    });
    view! { column(class="measurement-background",node_ref=host) {
        {zgui::elements::canvas().class("measurement-fill").scene(&handle)}
        if move || region.get().is_some() {
            box(class="measurement-region",style:left=move ||region.get().map(|r|format!("{}px",r.x0)),style:width=move ||region.get().map(|r|format!("{}px",r.width())))
        }
    } }
}
